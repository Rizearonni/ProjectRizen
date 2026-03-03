//! Database models for persistence.
//!
//! These structs map directly to PostgreSQL tables and are used
//! with sqlx for type-safe queries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Account record.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Account {
    pub account_id: Uuid,
    pub username: String,
    /// NULL for dev auth, bcrypt hash for production.
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Session record for authenticated connections.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Session {
    pub session_token: String,
    pub account_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// Character record (persistent identity).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Character {
    pub character_id: Uuid,
    pub account_id: Uuid,
    pub name: String,
    pub appearance_seed: i64,
    pub level: i32,
    pub memory_fragments: i32,
    pub created_at: DateTime<Utc>,
}

/// Character world state (position, zone).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterState {
    pub character_id: Uuid,
    pub zone_id: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub yaw: f32,
    pub updated_at: DateTime<Utc>,
}

/// Inventory slot record.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct InventorySlot {
    pub character_id: Uuid,
    pub slot_index: i32,
    pub item_id: String,
    pub quantity: i32,
}

/// Memory tree unlock record.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryUnlock {
    pub character_id: Uuid,
    pub node_id: String,
    pub unlocked_at: DateTime<Utc>,
}

/// Summary of a character for login/selection screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub character_id: Uuid,
    pub name: String,
    pub level: i32,
    pub zone_id: Option<String>,
}

impl From<(Character, Option<CharacterState>)> for CharacterSummary {
    fn from((char, state): (Character, Option<CharacterState>)) -> Self {
        Self {
            character_id: char.character_id,
            name: char.name,
            level: char.level,
            zone_id: state.map(|s| s.zone_id),
        }
    }
}

/// New account creation data.
#[derive(Debug, Clone)]
pub struct NewAccount {
    pub username: String,
    pub password_hash: Option<String>,
}

/// New character creation data.
#[derive(Debug, Clone)]
pub struct NewCharacter {
    pub account_id: Uuid,
    pub name: String,
    pub appearance_seed: i64,
}

/// Character state update data.
#[derive(Debug, Clone)]
pub struct CharacterStateUpdate {
    pub zone_id: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub yaw: f32,
}

impl CharacterStateUpdate {
    pub fn new(zone_id: impl Into<String>, x: f32, y: f32, z: f32, yaw: f32) -> Self {
        Self {
            zone_id: zone_id.into(),
            pos_x: x,
            pos_y: y,
            pos_z: z,
            yaw,
        }
    }
}
