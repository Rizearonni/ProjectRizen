//! World state management.
//!
//! Tracks all entities and their transforms. Provides snapshot generation.

use std::collections::HashMap;

use common::{EntityId, EntitySnapshot, Transform, WorldSnapshot};
use uuid::Uuid;

/// Counter for generating unique entity IDs.
#[derive(Debug, Default)]
pub struct EntityIdGen {
    next: u32,
}

impl EntityIdGen {
    pub fn next(&mut self) -> EntityId {
        let id = EntityId::new(self.next);
        self.next += 1;
        id
    }
}

/// Player entity with pending input.
#[derive(Debug, Clone)]
pub struct PlayerEntity {
    pub entity_id: EntityId,
    pub transform: Transform,
    /// Pending movement input (applied on tick).
    pub pending_move: Option<PendingMove>,
    /// Character ID for persistence (None if persistence disabled).
    pub character_id: Option<Uuid>,
    /// Current zone ID for persistence.
    pub zone_id: String,
}

/// Pending movement input from client.
#[derive(Debug, Clone, Copy)]
pub struct PendingMove {
    pub move_x: f32,
    pub move_y: f32,
    pub yaw: f32,
}

/// The game world state.
#[derive(Debug)]
pub struct World {
    /// Entity ID generator.
    id_gen: EntityIdGen,
    /// All player entities, keyed by entity ID.
    pub players: HashMap<EntityId, PlayerEntity>,
    /// Current server tick.
    pub server_tick: u64,
}

impl World {
    pub fn new() -> Self {
        Self {
            id_gen: EntityIdGen::default(),
            players: HashMap::new(),
            server_tick: 0,
        }
    }

    /// Spawn a new player entity with optional initial state.
    pub fn spawn_player(
        &mut self,
        transform: Option<Transform>,
        character_id: Option<Uuid>,
        zone_id: Option<String>,
    ) -> (EntityId, Transform) {
        let entity_id = self.id_gen.next();
        let transform = transform.unwrap_or_else(Transform::at_origin);
        let zone_id = zone_id.unwrap_or_else(|| "zone.ossuary".to_string());
        
        let player = PlayerEntity {
            entity_id,
            transform,
            pending_move: None,
            character_id,
            zone_id,
        };
        
        self.players.insert(entity_id, player);
        (entity_id, transform)
    }

    /// Get player by entity ID.
    pub fn get_player(&self, entity_id: EntityId) -> Option<&PlayerEntity> {
        self.players.get(&entity_id)
    }

    /// Remove a player entity.
    pub fn despawn_player(&mut self, entity_id: EntityId) -> bool {
        self.players.remove(&entity_id).is_some()
    }

    /// Queue movement input for a player.
    pub fn queue_input(&mut self, entity_id: EntityId, move_x: f32, move_y: f32, yaw: f32) {
        if let Some(player) = self.players.get_mut(&entity_id) {
            player.pending_move = Some(PendingMove { move_x, move_y, yaw });
        }
    }

    /// Process one server tick: apply pending inputs and advance tick counter.
    pub fn tick(&mut self, delta_time: f32) {
        const MOVE_SPEED: f32 = 5.0; // units per second

        for player in self.players.values_mut() {
            if let Some(input) = player.pending_move.take() {
                // Apply movement (simple, no collision)
                let move_vec = common::Vec3::new(
                    input.move_x * MOVE_SPEED * delta_time,
                    0.0,
                    input.move_y * MOVE_SPEED * delta_time,
                );
                player.transform.pos += move_vec;
                player.transform.yaw = input.yaw;
            }
        }

        self.server_tick += 1;
    }

    /// Generate a world snapshot for broadcasting.
    pub fn snapshot(&self) -> WorldSnapshot {
        let entities = self.players
            .values()
            .map(|p| EntitySnapshot {
                id: p.entity_id,
                transform: p.transform,
            })
            .collect();

        WorldSnapshot {
            server_tick: self.server_tick,
            entities,
        }
    }

    /// Get current player count.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_despawn_player() {
        let mut world = World::new();
        
        let (id1, _) = world.spawn_player(None, None, None);
        let (id2, _) = world.spawn_player(None, None, None);
        
        assert_eq!(world.player_count(), 2);
        assert_ne!(id1, id2);
        
        world.despawn_player(id1);
        assert_eq!(world.player_count(), 1);
    }

    #[test]
    fn movement_applied_on_tick() {
        let mut world = World::new();
        let (id, _) = world.spawn_player(None, None, None);
        
        world.queue_input(id, 1.0, 0.0, 0.0);
        world.tick(1.0); // 1 second tick
        
        let player = world.players.get(&id).unwrap();
        assert!(player.transform.pos.x > 0.0);
    }

    #[test]
    fn snapshot_contains_all_players() {
        let mut world = World::new();
        world.spawn_player(None, None, None);
        world.spawn_player(None, None, None);
        world.spawn_player(None, None, None);
        
        let snapshot = world.snapshot();
        assert_eq!(snapshot.entities.len(), 3);
    }
}
