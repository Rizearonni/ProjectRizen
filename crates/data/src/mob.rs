//! Mob definition loading from TOML files.
//!
//! Mob definitions specify stats, AI behavior, and loot for NPCs.

use serde::{Deserialize, Serialize};

use crate::zone::ValidationError;

/// Mob definition loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobDef {
    /// Unique mob ID (e.g., "mob.skeleton_scout").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Mob level (affects scaling, drop rates).
    #[serde(default = "default_level")]
    pub level: u32,
    /// Model/mesh identifier.
    #[serde(default = "default_model")]
    pub model: String,
    /// Combat and movement stats.
    #[serde(default)]
    pub stats: MobStats,
    /// Loot configuration.
    #[serde(default)]
    pub loot: LootConfig,
}

fn default_level() -> u32 { 1 }
fn default_model() -> String { "primitive.capsule".to_string() }

/// Mob combat and movement stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobStats {
    /// Maximum health points.
    #[serde(default = "default_hp")]
    pub hp: u32,
    /// Movement speed (units per second).
    #[serde(default = "default_move_speed")]
    pub move_speed: f32,
    /// Attack range (units).
    #[serde(default = "default_attack_range")]
    pub attack_range: f32,
    /// Damage per attack.
    #[serde(default = "default_attack_damage")]
    pub attack_damage: u32,
    /// Time between attacks (seconds).
    #[serde(default = "default_attack_cooldown")]
    pub attack_cooldown: f32,
    /// Range at which mob aggros on players.
    #[serde(default = "default_aggro_range")]
    pub aggro_range: f32,
    /// Range at which mob gives up and returns home.
    #[serde(default = "default_leash_range")]
    pub leash_range: f32,
}

fn default_hp() -> u32 { 50 }
fn default_move_speed() -> f32 { 3.0 }
fn default_attack_range() -> f32 { 2.0 }
fn default_attack_damage() -> u32 { 5 }
fn default_attack_cooldown() -> f32 { 2.0 }
fn default_aggro_range() -> f32 { 10.0 }
fn default_leash_range() -> f32 { 20.0 }

impl Default for MobStats {
    fn default() -> Self {
        Self {
            hp: default_hp(),
            move_speed: default_move_speed(),
            attack_range: default_attack_range(),
            attack_damage: default_attack_damage(),
            attack_cooldown: default_attack_cooldown(),
            aggro_range: default_aggro_range(),
            leash_range: default_leash_range(),
        }
    }
}

/// Loot configuration for a mob.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LootConfig {
    /// Reference to loot table ID.
    #[serde(default)]
    pub table: Option<String>,
    /// Direct drop chance (0.0-1.0) if no table.
    #[serde(default)]
    pub drop_chance: f32,
}

impl MobDef {
    /// Parse a mob definition from TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Validate the mob definition.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_empty() {
            return Err(ValidationError::EmptyId("mob"));
        }
        if !self.id.starts_with("mob.") {
            return Err(ValidationError::InvalidIdPrefix {
                kind: "mob",
                id: self.id.clone(),
                expected: "mob.",
            });
        }
        if self.name.is_empty() {
            return Err(ValidationError::EmptyName("mob", self.id.clone()));
        }
        if self.stats.hp == 0 {
            return Err(ValidationError::InvalidValue {
                field: "stats.hp",
                reason: "must be at least 1",
            });
        }
        if self.stats.move_speed < 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "stats.move_speed",
                reason: "cannot be negative",
            });
        }
        if self.stats.attack_cooldown <= 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "stats.attack_cooldown",
                reason: "must be positive",
            });
        }
        if self.stats.leash_range < self.stats.aggro_range {
            return Err(ValidationError::InvalidValue {
                field: "stats.leash_range",
                reason: "must be >= aggro_range",
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MOB_TOML: &str = r#"
id = "mob.skeleton_scout"
name = "Skeleton Scout"
level = 2
model = "primitive.capsule"

[stats]
hp = 45
move_speed = 3.5
attack_range = 2.0
attack_damage = 6
attack_cooldown = 1.8
aggro_range = 12.0
leash_range = 18.0

[loot]
table = "loot.low_ossuary"
"#;

    #[test]
    fn parse_mob_toml() {
        let mob = MobDef::from_toml(SAMPLE_MOB_TOML).expect("Failed to parse");
        assert_eq!(mob.id, "mob.skeleton_scout");
        assert_eq!(mob.name, "Skeleton Scout");
        assert_eq!(mob.level, 2);
        assert_eq!(mob.stats.hp, 45);
        assert_eq!(mob.stats.aggro_range, 12.0);
        assert_eq!(mob.loot.table, Some("loot.low_ossuary".to_string()));
    }

    #[test]
    fn mob_validation_pass() {
        let mob = MobDef::from_toml(SAMPLE_MOB_TOML).unwrap();
        assert!(mob.validate().is_ok());
    }

    #[test]
    fn mob_validation_fail_leash_less_than_aggro() {
        let toml = r#"
id = "mob.test"
name = "Test"

[stats]
aggro_range = 20.0
leash_range = 10.0
"#;
        let mob = MobDef::from_toml(toml).unwrap();
        assert!(matches!(mob.validate(), Err(ValidationError::InvalidValue { .. })));
    }

    #[test]
    fn mob_defaults() {
        let toml = r#"
id = "mob.simple"
name = "Simple Mob"
"#;
        let mob = MobDef::from_toml(toml).unwrap();
        assert_eq!(mob.level, 1);
        assert_eq!(mob.model, "primitive.capsule");
        assert_eq!(mob.stats.hp, 50);
    }
}
