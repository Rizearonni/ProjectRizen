//! Zone WebSocket handler and tick loop.
//!
//! Handles client connections, protocol messages, and server tick broadcasting.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use common::{
    decode_message, encode_message, CharacterId, EntityId, InputMove, Message, Transform,
    ZoneEnterWorldOk, ZoneWelcome, PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use persistence::{AccountRepo, CharacterRepo, CharacterStateUpdate, NewCharacter, PgPool};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::AppState;

/// Channel capacity for client message queue.
const CLIENT_MSG_CAPACITY: usize = 64;

/// Broadcast channel capacity for snapshots.
const BROADCAST_CAPACITY: usize = 16;

/// Shared broadcast sender for world snapshots.
static SNAPSHOT_TX: std::sync::OnceLock<broadcast::Sender<Vec<u8>>> = std::sync::OnceLock::new();

fn get_snapshot_tx() -> &'static broadcast::Sender<Vec<u8>> {
    SNAPSHOT_TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        tx
    })
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_connection(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    info!("New WebSocket connection");

    // Wait for ZoneHello
    let character_id_from_client = match wait_for_hello(&mut ws_rx).await {
        Some(id) => id,
        None => {
            warn!("Client disconnected before sending ZoneHello");
            return;
        }
    };

    info!(?character_id_from_client, "Received ZoneHello");

    // Load or create character data from DB (if persistence enabled)
    let (db_character_id, spawn_transform, zone_id) = match &state.db_pool {
        Some(pool) => {
            match load_or_create_dev_character(pool).await {
                Ok((char_id, transform, zone)) => (Some(char_id), Some(transform), Some(zone)),
                Err(e) => {
                    error!("Failed to load/create character: {}", e);
                    // Continue without persistence
                    (None, None, None)
                }
            }
        }
        None => (None, None, None),
    };

    // Send ZoneWelcome
    let welcome = ZoneWelcome {
        zone_id: zone_id.clone().unwrap_or_else(|| "ossuary".to_string()),
        zone_name: "The Ossuary".to_string(),
        tick_rate: state.config.tick_rate,
    };
    if send_message(&mut ws_tx, welcome).await.is_err() {
        return;
    }

    // Spawn player entity
    let (entity_id, transform) = {
        let mut world = state.world.write().await;
        world.spawn_player(spawn_transform, db_character_id, zone_id)
    };

    info!(?entity_id, ?db_character_id, "Player spawned");

    // Send ZoneEnterWorldOk
    let enter_ok = ZoneEnterWorldOk {
        entity_id,
        transform,
    };
    if send_message(&mut ws_tx, enter_ok).await.is_err() {
        cleanup_player(&state, entity_id).await;
        return;
    }

    // Subscribe to snapshot broadcasts
    let mut snapshot_rx = get_snapshot_tx().subscribe();

    // Channel for forwarding incoming messages
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(CLIENT_MSG_CAPACITY);

    // Spawn task to read from WebSocket
    let read_handle = tokio::spawn(async move {
        while let Some(Ok(ws_msg)) = ws_rx.next().await {
            if let WsMessage::Binary(data) = ws_msg {
                match decode_message(&data) {
                    Ok((msg, _)) => {
                        if msg_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to decode message: {}", e);
                    }
                }
            } else if matches!(ws_msg, WsMessage::Close(_)) {
                break;
            }
        }
    });

    // Main client loop
    loop {
        tokio::select! {
            // Handle incoming client messages
            Some(msg) = msg_rx.recv() => {
                handle_client_message(&state, entity_id, msg).await;
            }

            // Forward snapshot broadcasts to client
            Ok(snapshot_data) = snapshot_rx.recv() => {
                if ws_tx.send(WsMessage::Binary(snapshot_data.into())).await.is_err() {
                    break;
                }
            }

            // Read task finished (client disconnected)
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if read_handle.is_finished() {
                    break;
                }
            }
        }
    }

    info!(?entity_id, "Client disconnected");
    
    // Save character state before cleanup (if persistence enabled)
    if let Some(pool) = &state.db_pool {
        save_character_state_on_disconnect(&state, entity_id, pool.clone()).await;
    }
    
    cleanup_player(&state, entity_id).await;
}

/// Wait for the initial ZoneHello message.
async fn wait_for_hello(
    ws_rx: &mut futures_util::stream::SplitStream<WebSocket>,
) -> Option<CharacterId> {
    // Timeout after 10 seconds
    let timeout = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(ws_msg)) = ws_rx.next().await {
            if let WsMessage::Binary(data) = ws_msg {
                if let Ok((Message::ZoneHello(hello), _)) = decode_message(&data) {
                    if hello.protocol_version == PROTOCOL_VERSION {
                        return Some(hello.character_id);
                    } else {
                        warn!(
                            "Protocol version mismatch: expected {}, got {}",
                            PROTOCOL_VERSION, hello.protocol_version
                        );
                        return None;
                    }
                }
            }
        }
        None
    });

    match timeout.await {
        Ok(result) => result,
        Err(_) => {
            warn!("Timeout waiting for ZoneHello");
            None
        }
    }
}

/// Send a protocol message over WebSocket.
async fn send_message<T: Into<Message>>(
    ws_tx: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    msg: T,
) -> Result<(), ()> {
    let data = encode_message(msg, None).map_err(|e| {
        error!("Failed to encode message: {}", e);
    })?;

    ws_tx
        .send(WsMessage::Binary(data.into()))
        .await
        .map_err(|e| {
            error!("Failed to send message: {}", e);
        })
}

/// Handle an incoming client message.
async fn handle_client_message(state: &AppState, entity_id: EntityId, msg: Message) {
    match msg {
        Message::InputMove(InputMove {
            move_x,
            move_y,
            yaw,
            client_tick: _,
        }) => {
            let mut world = state.world.write().await;
            world.queue_input(entity_id, move_x, move_y, yaw);
        }
        _ => {
            debug!(?msg, "Unhandled message type");
        }
    }
}

/// Remove player entity on disconnect.
async fn cleanup_player(state: &AppState, entity_id: EntityId) {
    let mut world = state.world.write().await;
    if world.despawn_player(entity_id) {
        info!(?entity_id, "Player despawned");
    }
}

/// Server tick loop.
///
/// Runs at configured tick rate, processes world updates,
/// and broadcasts snapshots at configured snapshot rate.
pub async fn tick_loop(state: Arc<AppState>) {
    let tick_duration = Duration::from_secs_f64(1.0 / state.config.tick_rate as f64);
    let snapshot_interval = state.config.tick_rate / state.config.snapshot_rate;

    let mut tick_timer = interval(tick_duration);
    tick_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut tick_count: u32 = 0;
    let mut last_tick = Instant::now();

    info!("Tick loop started");

    loop {
        tick_timer.tick().await;

        let now = Instant::now();
        let delta_time = (now - last_tick).as_secs_f32();
        last_tick = now;

        // Process world tick
        {
            let mut world = state.world.write().await;
            world.tick(delta_time);
        }

        tick_count += 1;

        // Broadcast snapshot at configured rate
        if tick_count % snapshot_interval == 0 {
            let snapshot = {
                let world = state.world.read().await;
                world.snapshot()
            };

            if let Ok(data) = encode_message(snapshot, None) {
                // Ignore send errors (no subscribers)
                let _ = get_snapshot_tx().send(data);
            }

            debug!(tick = tick_count, "Snapshot broadcast");
        }
    }
}

/// Default zone for new characters.
const DEFAULT_ZONE_ID: &str = "zone.ossuary";
/// Dev auth username.
const DEV_USERNAME: &str = "dev";
/// Dev character name.
const DEV_CHARACTER_NAME: &str = "DevChar";
/// Dev character appearance seed.
const DEV_APPEARANCE_SEED: i64 = 12345;

/// Load or create dev account, character, and state.
///
/// Returns (character_id, spawn_transform, zone_id).
async fn load_or_create_dev_character(
    pool: &PgPool,
) -> Result<(Uuid, Transform, String), Box<dyn std::error::Error + Send + Sync>> {
    let account_repo = AccountRepo::new(pool);
    let char_repo = CharacterRepo::new(pool);

    // Get or create dev account
    let account = account_repo.get_or_create_dev(DEV_USERNAME).await?;
    debug!(account_id = %account.account_id, "Using dev account");

    // Get or create dev character for this account
    let characters = char_repo.get_by_account(account.account_id).await?;
    let character = if let Some(char) = characters.first() {
        debug!(character_id = %char.character_id, name = %char.name, "Loaded existing character");
        char.clone()
    } else {
        let new_char = NewCharacter {
            account_id: account.account_id,
            name: DEV_CHARACTER_NAME.to_string(),
            appearance_seed: DEV_APPEARANCE_SEED,
        };
        let char = char_repo.create(new_char).await?;
        info!(character_id = %char.character_id, name = %char.name, "Created new dev character");
        char
    };

    // Load or create character state
    let state = char_repo.get_state(character.character_id).await?;
    let (transform, zone_id) = match state {
        Some(s) => {
            let t = Transform {
                pos: common::Vec3::new(s.pos_x, s.pos_y, s.pos_z),
                yaw: s.yaw,
            };
            info!(
                character_id = %character.character_id,
                zone = %s.zone_id,
                pos = ?t.pos,
                "Loaded character state from DB"
            );
            (t, s.zone_id)
        }
        None => {
            // Create default state
            let default_state = CharacterStateUpdate::new(DEFAULT_ZONE_ID, 0.0, 0.0, 0.0, 0.0);
            char_repo.save_state(character.character_id, default_state).await?;
            info!(
                character_id = %character.character_id,
                zone = DEFAULT_ZONE_ID,
                "Created default character state"
            );
            (Transform::at_origin(), DEFAULT_ZONE_ID.to_string())
        }
    };

    Ok((character.character_id, transform, zone_id))
}

/// Save character state on disconnect (spawns async task).
async fn save_character_state_on_disconnect(state: &AppState, entity_id: EntityId, pool: PgPool) {
    // Get player data before despawn
    let player_data = {
        let world = state.world.read().await;
        world.get_player(entity_id).map(|p| (p.character_id, p.transform, p.zone_id.clone()))
    };

    if let Some((Some(character_id), transform, zone_id)) = player_data {
        // Spawn async task to save (don't block disconnect)
        tokio::spawn(async move {
            let char_repo = CharacterRepo::new(&pool);
            let update = CharacterStateUpdate::new(
                zone_id.clone(),
                transform.pos.x,
                transform.pos.y,
                transform.pos.z,
                transform.yaw,
            );

            match char_repo.save_state(character_id, update).await {
                Ok(()) => {
                    info!(
                        character_id = %character_id,
                        zone = %zone_id,
                        pos = ?transform.pos,
                        yaw = transform.yaw,
                        "Saved character_state on disconnect"
                    );
                }
                Err(e) => {
                    error!(
                        character_id = %character_id,
                        error = %e,
                        "Failed to save character_state"
                    );
                }
            }
        });
    }
}
