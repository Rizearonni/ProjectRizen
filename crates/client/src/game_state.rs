//! Client-side game state.
//!
//! Stores connection status, entity positions, and local player state.

use std::collections::HashMap;

use common::{CharacterId, EntityId, EntitySnapshot, Transform};

/// Connection status with the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl ConnectionStatus {
    /// Get display string for window title.
    pub fn as_str(&self) -> &str {
        match self {
            ConnectionStatus::Disconnected => "Disconnected",
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Error(_) => "Error",
        }
    }
}

/// Stats for window title HUD display.
#[derive(Debug, Clone)]
pub struct HudStats {
    pub status: String,
    pub entity_count: usize,
    pub server_tick: u64,
    pub ping_ms: u32,
    pub input_seq: u32,
}

impl HudStats {
    pub fn new() -> Self {
        Self {
            status: "Disconnected".to_string(),
            entity_count: 0,
            server_tick: 0,
            ping_ms: 0,
            input_seq: 0,
        }
    }

    /// Format as window title string.
    pub fn to_title(&self) -> String {
        format!(
            "Project Rizen — {} — ents: {} — tick: {} — ping: {}ms — seq: {}",
            self.status, self.entity_count, self.server_tick, self.ping_ms, self.input_seq
        )
    }
}

impl Default for HudStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Client-side game state.
#[derive(Debug)]
pub struct GameState {
    /// Server URL to connect to.
    pub server_url: String,
    /// Current connection status.
    pub connection_status: ConnectionStatus,
    /// Local player's entity ID (assigned by server).
    pub local_entity_id: Option<EntityId>,
    /// Local player's character ID.
    pub character_id: CharacterId,
    /// Zone information.
    pub zone_name: String,
    /// All known entities and their transforms.
    pub entities: HashMap<EntityId, Transform>,
    /// Latest server tick received.
    pub server_tick: u64,
    /// Pending input to send.
    pub pending_input: Option<PendingInput>,
    /// Current movement input state.
    pub move_input: MoveInput,
    /// Debug: ping placeholder (ms).
    pub ping_ms: u32,
}

/// Pending input to be sent to server.
#[derive(Debug, Clone, Copy)]
pub struct PendingInput {
    pub move_x: f32,
    pub move_y: f32,
    pub yaw: f32,
    pub client_tick: u32,
}

/// Current movement input state from keyboard/mouse.
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub yaw: f32,
}

impl MoveInput {
    /// Convert WASD state to normalized move vector.
    pub fn to_move_vec(&self) -> (f32, f32) {
        let mut x: f32 = 0.0;
        let mut y: f32 = 0.0;

        if self.forward {
            y += 1.0;
        }
        if self.backward {
            y -= 1.0;
        }
        if self.left {
            x -= 1.0;
        }
        if self.right {
            x += 1.0;
        }

        // Normalize diagonal movement
        let len = (x * x + y * y).sqrt();
        if len > 0.0 {
            (x / len, y / len)
        } else {
            (0.0, 0.0)
        }
    }
}

impl GameState {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            connection_status: ConnectionStatus::Disconnected,
            local_entity_id: None,
            character_id: CharacterId::new(),
            zone_name: String::new(),
            entities: HashMap::new(),
            server_tick: 0,
            pending_input: None,
            move_input: MoveInput::default(),
            ping_ms: 0,
        }
    }

    /// Update entities from a world snapshot.
    pub fn apply_snapshot(&mut self, tick: u64, entities: Vec<EntitySnapshot>) {
        self.server_tick = tick;
        self.entities.clear();
        for ent in entities {
            self.entities.insert(ent.id, ent.transform);
        }
    }

    /// Get entity count.
    #[allow(dead_code)]
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.connection_status == ConnectionStatus::Connected
    }

    /// Queue input to send based on current move state.
    pub fn queue_input(&mut self, client_tick: u32) {
        let (move_x, move_y) = self.move_input.to_move_vec();
        self.pending_input = Some(PendingInput {
            move_x,
            move_y,
            yaw: self.move_input.yaw,
            client_tick,
        });
    }

    /// Take pending input (clears it).
    pub fn take_pending_input(&mut self) -> Option<PendingInput> {
        self.pending_input.take()
    }

    /// Build HudStats from current state.
    pub fn build_hud_stats(&self, input_seq: u32) -> HudStats {
        HudStats {
            status: self.connection_status.as_str().to_string(),
            entity_count: self.entities.len(),
            server_tick: self.server_tick,
            ping_ms: self.ping_ms,
            input_seq,
        }
    }
}
