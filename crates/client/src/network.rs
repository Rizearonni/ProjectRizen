//! Network client for WebSocket communication with the zone server.

use std::sync::Arc;

use common::{
    decode_message, encode_message, InputMove, Message, ZoneHello,
    PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, warn};

use crate::game_state::{ConnectionStatus, GameState, PendingInput};

/// Commands sent to the network task.
pub enum NetCommand {
    Connect,
    #[allow(dead_code)]
    Disconnect,
    SendInput(PendingInput),
}

/// Start the network task. Returns a command sender.
pub fn start_network_task(
    game_state: Arc<RwLock<GameState>>,
) -> mpsc::Sender<NetCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);

    // Spawn the network task on a tokio runtime
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        rt.block_on(network_task(game_state, cmd_rx));
    });

    cmd_tx
}

/// Main network task loop.
async fn network_task(
    game_state: Arc<RwLock<GameState>>,
    mut cmd_rx: mpsc::Receiver<NetCommand>,
) {
    let mut ws_tx: Option<futures_util::stream::SplitSink<_, WsMessage>> = None;
    let mut ws_rx: Option<futures_util::stream::SplitStream<_>> = None;

    loop {
        tokio::select! {
            // Handle commands from main thread
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    NetCommand::Connect => {
                        let url = {
                            let state = game_state.read().await;
                            state.server_url.clone()
                        };

                        info!("Connecting to {}", url);
                        {
                            let mut state = game_state.write().await;
                            state.connection_status = ConnectionStatus::Connecting;
                        }

                        match connect_and_handshake(&url, &game_state).await {
                            Ok((tx, rx)) => {
                                ws_tx = Some(tx);
                                ws_rx = Some(rx);
                                info!("Connected successfully");
                            }
                            Err(e) => {
                                error!("Connection failed: {}", e);
                                let mut state = game_state.write().await;
                                state.connection_status = ConnectionStatus::Error(e.to_string());
                            }
                        }
                    }

                    NetCommand::Disconnect => {
                        info!("Disconnecting");
                        ws_tx = None;
                        ws_rx = None;
                        let mut state = game_state.write().await;
                        state.connection_status = ConnectionStatus::Disconnected;
                        state.local_entity_id = None;
                        state.entities.clear();
                    }

                    NetCommand::SendInput(input) => {
                        if let Some(ref mut tx) = ws_tx {
                            let msg = InputMove {
                                move_x: input.move_x,
                                move_y: input.move_y,
                                yaw: input.yaw,
                                client_tick: input.client_tick,
                            };

                            if let Ok(data) = encode_message(msg, None) {
                                if let Err(e) = tx.send(WsMessage::Binary(data.into())).await {
                                    warn!("Failed to send input: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            // Handle incoming messages
            msg = async {
                if let Some(ref mut rx) = ws_rx {
                    rx.next().await
                } else {
                    std::future::pending().await
                }
            } => {
                match msg {
                    Some(Ok(WsMessage::Binary(data))) => {
                        if let Ok((message, _)) = decode_message(&data) {
                            handle_server_message(&game_state, message).await;
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => {
                        info!("Server closed connection");
                        ws_tx = None;
                        ws_rx = None;
                        let mut state = game_state.write().await;
                        state.connection_status = ConnectionStatus::Disconnected;
                        state.local_entity_id = None;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        ws_tx = None;
                        ws_rx = None;
                        let mut state = game_state.write().await;
                        state.connection_status = ConnectionStatus::Error(e.to_string());
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Connect to server and perform handshake.
async fn connect_and_handshake(
    url: &str,
    game_state: &Arc<RwLock<GameState>>,
) -> Result<(
    futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>,
    futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
), Box<dyn std::error::Error + Send + Sync>> {
    // Connect
    let (ws_stream, _) = connect_async(url).await?;
    let (mut tx, mut rx) = ws_stream.split();

    // Get character ID
    let character_id = {
        let state = game_state.read().await;
        state.character_id
    };

    // Send ZoneHello
    let hello = ZoneHello {
        character_id,
        protocol_version: PROTOCOL_VERSION,
    };
    let data = encode_message(hello, None)?;
    tx.send(WsMessage::Binary(data.into())).await?;
    debug!("Sent ZoneHello");

    // Wait for ZoneWelcome
    let zone_name = loop {
        if let Some(Ok(WsMessage::Binary(data))) = rx.next().await {
            if let Ok((Message::ZoneWelcome(welcome), _)) = decode_message(&data) {
                debug!("Received ZoneWelcome: {}", welcome.zone_name);
                break welcome.zone_name;
            }
        } else {
            return Err("Failed to receive ZoneWelcome".into());
        }
    };

    // Wait for ZoneEnterWorldOk
    let (entity_id, transform) = loop {
        if let Some(Ok(WsMessage::Binary(data))) = rx.next().await {
            if let Ok((Message::ZoneEnterWorldOk(enter), _)) = decode_message(&data) {
                debug!("Received ZoneEnterWorldOk: {:?}", enter.entity_id);
                break (enter.entity_id, enter.transform);
            }
        } else {
            return Err("Failed to receive ZoneEnterWorldOk".into());
        }
    };

    // Update game state
    {
        let mut state = game_state.write().await;
        state.connection_status = ConnectionStatus::Connected;
        state.zone_name = zone_name;
        state.local_entity_id = Some(entity_id);
        state.entities.insert(entity_id, transform);
    }

    Ok((tx, rx))
}

/// Handle a message from the server.
async fn handle_server_message(game_state: &Arc<RwLock<GameState>>, message: Message) {
    match message {
        Message::WorldSnapshot(snapshot) => {
            let mut state = game_state.write().await;
            state.apply_snapshot(snapshot.server_tick, snapshot.entities);
        }
        Message::EntityDespawn(despawn) => {
            let mut state = game_state.write().await;
            state.entities.remove(&despawn.entity_id);
        }
        _ => {
            debug!("Unhandled message: {:?}", message);
        }
    }
}
