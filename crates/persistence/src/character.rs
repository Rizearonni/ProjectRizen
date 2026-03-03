//! Character repository.
//!
//! Handles character CRUD, state persistence, inventory, and memory unlocks.

use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::{PersistenceError, Result};
use crate::models::{
    Character, CharacterState, CharacterStateUpdate, CharacterSummary,
    InventorySlot, MemoryUnlock, NewCharacter,
};

/// Maximum characters per account.
pub const MAX_CHARACTERS_PER_ACCOUNT: u32 = 5;

/// Character repository for database operations.
pub struct CharacterRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> CharacterRepo<'a> {
    /// Create a new character repository.
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new character.
    pub async fn create(&self, new_char: NewCharacter) -> Result<Character> {
        // Check character count for account
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM characters WHERE account_id = $1",
        )
        .bind(new_char.account_id)
        .fetch_one(self.pool)
        .await?;

        if count >= MAX_CHARACTERS_PER_ACCOUNT as i64 {
            return Err(PersistenceError::MaxCharactersReached(MAX_CHARACTERS_PER_ACCOUNT));
        }

        // Check if name already exists
        let name_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM characters WHERE LOWER(name) = LOWER($1)",
        )
        .bind(&new_char.name)
        .fetch_one(self.pool)
        .await?;

        if name_exists > 0 {
            return Err(PersistenceError::CharacterNameExists(new_char.name));
        }

        let character_id = Uuid::new_v4();
        let now = Utc::now();

        let character = sqlx::query_as::<_, Character>(
            r#"
            INSERT INTO characters (character_id, account_id, name, appearance_seed, level, memory_fragments, created_at)
            VALUES ($1, $2, $3, $4, 1, 0, $5)
            RETURNING character_id, account_id, name, appearance_seed, level, memory_fragments, created_at
            "#,
        )
        .bind(character_id)
        .bind(new_char.account_id)
        .bind(&new_char.name)
        .bind(new_char.appearance_seed)
        .bind(now)
        .fetch_one(self.pool)
        .await?;

        info!("Created character: {} ({})", character.name, character.character_id);
        Ok(character)
    }

    /// Get character by ID.
    pub async fn get_by_id(&self, character_id: Uuid) -> Result<Character> {
        sqlx::query_as::<_, Character>(
            "SELECT character_id, account_id, name, appearance_seed, level, memory_fragments, created_at FROM characters WHERE character_id = $1",
        )
        .bind(character_id)
        .fetch_optional(self.pool)
        .await?
        .ok_or(PersistenceError::CharacterNotFound(character_id))
    }

    /// Get all characters for an account.
    pub async fn get_by_account(&self, account_id: Uuid) -> Result<Vec<Character>> {
        let characters = sqlx::query_as::<_, Character>(
            "SELECT character_id, account_id, name, appearance_seed, level, memory_fragments, created_at FROM characters WHERE account_id = $1 ORDER BY created_at",
        )
        .bind(account_id)
        .fetch_all(self.pool)
        .await?;

        Ok(characters)
    }

    /// Get character summaries for account (for character select screen).
    pub async fn get_summaries_by_account(&self, account_id: Uuid) -> Result<Vec<CharacterSummary>> {
        let rows = sqlx::query_as::<_, (Uuid, String, i32, Option<String>)>(
            r#"
            SELECT c.character_id, c.name, c.level, cs.zone_id
            FROM characters c
            LEFT JOIN character_state cs ON c.character_id = cs.character_id
            WHERE c.account_id = $1
            ORDER BY c.created_at
            "#,
        )
        .bind(account_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, level, zone_id)| CharacterSummary {
                character_id: id,
                name,
                level,
                zone_id,
            })
            .collect())
    }

    /// Delete a character.
    pub async fn delete(&self, character_id: Uuid, account_id: Uuid) -> Result<()> {
        // Verify ownership
        let char = self.get_by_id(character_id).await?;
        if char.account_id != account_id {
            return Err(PersistenceError::CharacterNotFound(character_id));
        }

        // Delete in order: inventory, memory_unlocks, character_state, character
        sqlx::query("DELETE FROM inventory_slots WHERE character_id = $1")
            .bind(character_id)
            .execute(self.pool)
            .await?;

        sqlx::query("DELETE FROM memory_unlocks WHERE character_id = $1")
            .bind(character_id)
            .execute(self.pool)
            .await?;

        sqlx::query("DELETE FROM character_state WHERE character_id = $1")
            .bind(character_id)
            .execute(self.pool)
            .await?;

        sqlx::query("DELETE FROM characters WHERE character_id = $1")
            .bind(character_id)
            .execute(self.pool)
            .await?;

        info!("Deleted character: {}", character_id);
        Ok(())
    }

    /// Update character level.
    pub async fn update_level(&self, character_id: Uuid, level: i32) -> Result<()> {
        sqlx::query("UPDATE characters SET level = $1 WHERE character_id = $2")
            .bind(level)
            .bind(character_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Update memory fragments count.
    pub async fn update_memory_fragments(&self, character_id: Uuid, fragments: i32) -> Result<()> {
        sqlx::query("UPDATE characters SET memory_fragments = $1 WHERE character_id = $2")
            .bind(fragments)
            .bind(character_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    // --- Character State ---

    /// Get character world state.
    pub async fn get_state(&self, character_id: Uuid) -> Result<Option<CharacterState>> {
        let state = sqlx::query_as::<_, CharacterState>(
            "SELECT character_id, zone_id, pos_x, pos_y, pos_z, yaw, updated_at FROM character_state WHERE character_id = $1",
        )
        .bind(character_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(state)
    }

    /// Save character world state (upsert).
    pub async fn save_state(&self, character_id: Uuid, update: CharacterStateUpdate) -> Result<()> {
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO character_state (character_id, zone_id, pos_x, pos_y, pos_z, yaw, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (character_id) DO UPDATE SET
                zone_id = EXCLUDED.zone_id,
                pos_x = EXCLUDED.pos_x,
                pos_y = EXCLUDED.pos_y,
                pos_z = EXCLUDED.pos_z,
                yaw = EXCLUDED.yaw,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(character_id)
        .bind(&update.zone_id)
        .bind(update.pos_x)
        .bind(update.pos_y)
        .bind(update.pos_z)
        .bind(update.yaw)
        .bind(now)
        .execute(self.pool)
        .await?;

        debug!("Saved state for character {}", character_id);
        Ok(())
    }

    // --- Inventory ---

    /// Get all inventory slots for a character.
    pub async fn get_inventory(&self, character_id: Uuid) -> Result<Vec<InventorySlot>> {
        let slots = sqlx::query_as::<_, InventorySlot>(
            "SELECT character_id, slot_index, item_id, quantity FROM inventory_slots WHERE character_id = $1 ORDER BY slot_index",
        )
        .bind(character_id)
        .fetch_all(self.pool)
        .await?;

        Ok(slots)
    }

    /// Set an inventory slot (upsert).
    pub async fn set_inventory_slot(
        &self,
        character_id: Uuid,
        slot_index: i32,
        item_id: &str,
        quantity: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO inventory_slots (character_id, slot_index, item_id, quantity)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (character_id, slot_index) DO UPDATE SET
                item_id = EXCLUDED.item_id,
                quantity = EXCLUDED.quantity
            "#,
        )
        .bind(character_id)
        .bind(slot_index)
        .bind(item_id)
        .bind(quantity)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Clear an inventory slot.
    pub async fn clear_inventory_slot(&self, character_id: Uuid, slot_index: i32) -> Result<()> {
        sqlx::query("DELETE FROM inventory_slots WHERE character_id = $1 AND slot_index = $2")
            .bind(character_id)
            .bind(slot_index)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    // --- Memory Unlocks ---

    /// Get all memory unlocks for a character.
    pub async fn get_memory_unlocks(&self, character_id: Uuid) -> Result<Vec<MemoryUnlock>> {
        let unlocks = sqlx::query_as::<_, MemoryUnlock>(
            "SELECT character_id, node_id, unlocked_at FROM memory_unlocks WHERE character_id = $1 ORDER BY unlocked_at",
        )
        .bind(character_id)
        .fetch_all(self.pool)
        .await?;

        Ok(unlocks)
    }

    /// Add a memory unlock.
    pub async fn add_memory_unlock(&self, character_id: Uuid, node_id: &str) -> Result<()> {
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO memory_unlocks (character_id, node_id, unlocked_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (character_id, node_id) DO NOTHING
            "#,
        )
        .bind(character_id)
        .bind(node_id)
        .bind(now)
        .execute(self.pool)
        .await?;

        debug!("Added memory unlock {} for character {}", node_id, character_id);
        Ok(())
    }

    /// Check if a memory node is unlocked.
    pub async fn has_memory_unlock(&self, character_id: Uuid, node_id: &str) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM memory_unlocks WHERE character_id = $1 AND node_id = $2",
        )
        .bind(character_id)
        .bind(node_id)
        .fetch_one(self.pool)
        .await?;

        Ok(count > 0)
    }
}
