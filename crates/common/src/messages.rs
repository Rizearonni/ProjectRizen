//! Protocol message definitions.
//!
//! Each message has an explicit discriminant (u16) for stable wire format.
//! Messages are grouped by category: auth, zone, input, snapshot, chat.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, CharacterId, EntityId};
use crate::math::Transform;

// ============================================================================
// Message type discriminants (stable across builds)
// ============================================================================

/// Discriminant constants for message types.
/// These must remain stable once assigned.
pub mod msg_type {
    // Auth messages: 1-99
    pub const AUTH_HELLO: u16 = 1;
    pub const AUTH_LOGIN_DEV: u16 = 2;
    pub const AUTH_LOGIN_OK: u16 = 3;
    pub const AUTH_CREATE_CHARACTER: u16 = 4;
    pub const AUTH_CREATE_CHARACTER_OK: u16 = 5;

    // Zone messages: 100-199
    pub const ZONE_HELLO: u16 = 100;
    pub const ZONE_WELCOME: u16 = 101;
    pub const ZONE_ENTER_WORLD_OK: u16 = 102;

    // Input messages: 200-299
    pub const INPUT_MOVE: u16 = 200;

    // Snapshot messages: 300-399
    pub const WORLD_SNAPSHOT: u16 = 300;
    pub const ENTITY_DESPAWN: u16 = 301;

    // Chat messages: 400-499
    pub const CHAT_SEND: u16 = 400;
    pub const CHAT_BROADCAST: u16 = 401;
}

// ============================================================================
// Auth messages
// ============================================================================

/// Initial auth server hello (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthHello {
    pub server_name: String,
    pub protocol_version: u16,
}

/// Dev login request (client -> server). Bypasses real auth for development.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthLoginDev {
    pub username: String,
}

/// Successful login response (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthLoginOk {
    pub account_id: AccountId,
    pub characters: Vec<CharacterSummary>,
}

/// Summary of a character for selection screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterSummary {
    pub id: CharacterId,
    pub name: String,
    pub level: u32,
}

/// Create a new character (client -> server).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthCreateCharacter {
    pub name: String,
    pub appearance_seed: u64,
}

/// Character creation success (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthCreateCharacterOk {
    pub character: CharacterSummary,
}

// ============================================================================
// Zone messages
// ============================================================================

/// Client requests to enter zone (client -> server).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneHello {
    pub character_id: CharacterId,
    pub protocol_version: u16,
}

/// Server acknowledges zone connection (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneWelcome {
    pub zone_id: String,
    pub zone_name: String,
    pub tick_rate: u32,
}

/// Player has entered the world successfully (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneEnterWorldOk {
    pub entity_id: EntityId,
    pub transform: Transform,
}

// ============================================================================
// Input messages
// ============================================================================

/// Movement input from client (client -> server).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputMove {
    /// Movement vector on XZ plane (-1 to 1 each axis).
    pub move_x: f32,
    pub move_y: f32,
    /// Yaw rotation (absolute, radians).
    pub yaw: f32,
    /// Client tick for reconciliation.
    pub client_tick: u32,
}

// ============================================================================
// Snapshot messages
// ============================================================================

/// Snapshot of entity states (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldSnapshot {
    pub server_tick: u64,
    pub entities: Vec<EntitySnapshot>,
}

/// Single entity state in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub transform: Transform,
}

/// Entity has been removed from the world (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityDespawn {
    pub entity_id: EntityId,
}

// ============================================================================
// Chat messages
// ============================================================================

/// Chat message from client (client -> server).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatSend {
    pub channel: ChatChannel,
    pub message: String,
}

/// Chat message to broadcast (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatBroadcast {
    pub channel: ChatChannel,
    pub sender_name: String,
    pub sender_id: EntityId,
    pub message: String,
}

/// Chat channel types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChatChannel {
    Say,
    Shout,
    Party,
    Guild,
    System,
}

// ============================================================================
// Message enum (for envelope payload)
// ============================================================================

/// Union of all protocol messages.
/// Used internally for envelope encoding/decoding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Message {
    // Auth
    AuthHello(AuthHello),
    AuthLoginDev(AuthLoginDev),
    AuthLoginOk(AuthLoginOk),
    AuthCreateCharacter(AuthCreateCharacter),
    AuthCreateCharacterOk(AuthCreateCharacterOk),
    // Zone
    ZoneHello(ZoneHello),
    ZoneWelcome(ZoneWelcome),
    ZoneEnterWorldOk(ZoneEnterWorldOk),
    // Input
    InputMove(InputMove),
    // Snapshot
    WorldSnapshot(WorldSnapshot),
    EntityDespawn(EntityDespawn),
    // Chat
    ChatSend(ChatSend),
    ChatBroadcast(ChatBroadcast),
}

impl Message {
    /// Get the discriminant (msg_type) for this message.
    pub fn msg_type(&self) -> u16 {
        match self {
            Message::AuthHello(_) => msg_type::AUTH_HELLO,
            Message::AuthLoginDev(_) => msg_type::AUTH_LOGIN_DEV,
            Message::AuthLoginOk(_) => msg_type::AUTH_LOGIN_OK,
            Message::AuthCreateCharacter(_) => msg_type::AUTH_CREATE_CHARACTER,
            Message::AuthCreateCharacterOk(_) => msg_type::AUTH_CREATE_CHARACTER_OK,
            Message::ZoneHello(_) => msg_type::ZONE_HELLO,
            Message::ZoneWelcome(_) => msg_type::ZONE_WELCOME,
            Message::ZoneEnterWorldOk(_) => msg_type::ZONE_ENTER_WORLD_OK,
            Message::InputMove(_) => msg_type::INPUT_MOVE,
            Message::WorldSnapshot(_) => msg_type::WORLD_SNAPSHOT,
            Message::EntityDespawn(_) => msg_type::ENTITY_DESPAWN,
            Message::ChatSend(_) => msg_type::CHAT_SEND,
            Message::ChatBroadcast(_) => msg_type::CHAT_BROADCAST,
        }
    }
}

// Conversion traits for ergonomic message creation
impl From<AuthHello> for Message {
    fn from(m: AuthHello) -> Self {
        Message::AuthHello(m)
    }
}
impl From<AuthLoginDev> for Message {
    fn from(m: AuthLoginDev) -> Self {
        Message::AuthLoginDev(m)
    }
}
impl From<AuthLoginOk> for Message {
    fn from(m: AuthLoginOk) -> Self {
        Message::AuthLoginOk(m)
    }
}
impl From<AuthCreateCharacter> for Message {
    fn from(m: AuthCreateCharacter) -> Self {
        Message::AuthCreateCharacter(m)
    }
}
impl From<AuthCreateCharacterOk> for Message {
    fn from(m: AuthCreateCharacterOk) -> Self {
        Message::AuthCreateCharacterOk(m)
    }
}
impl From<ZoneHello> for Message {
    fn from(m: ZoneHello) -> Self {
        Message::ZoneHello(m)
    }
}
impl From<ZoneWelcome> for Message {
    fn from(m: ZoneWelcome) -> Self {
        Message::ZoneWelcome(m)
    }
}
impl From<ZoneEnterWorldOk> for Message {
    fn from(m: ZoneEnterWorldOk) -> Self {
        Message::ZoneEnterWorldOk(m)
    }
}
impl From<InputMove> for Message {
    fn from(m: InputMove) -> Self {
        Message::InputMove(m)
    }
}
impl From<WorldSnapshot> for Message {
    fn from(m: WorldSnapshot) -> Self {
        Message::WorldSnapshot(m)
    }
}
impl From<EntityDespawn> for Message {
    fn from(m: EntityDespawn) -> Self {
        Message::EntityDespawn(m)
    }
}
impl From<ChatSend> for Message {
    fn from(m: ChatSend) -> Self {
        Message::ChatSend(m)
    }
}
impl From<ChatBroadcast> for Message {
    fn from(m: ChatBroadcast) -> Self {
        Message::ChatBroadcast(m)
    }
}
