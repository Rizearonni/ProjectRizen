//! Ability definition loading from TOML files.
//!
//! Ability definitions specify cooldowns, effects, and requirements.

use serde::{Deserialize, Serialize};

use crate::zone::ValidationError;

/// Ability definition loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityDef {
    /// Unique ability ID (e.g., "ability.memory_dash").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Ability cooldown in seconds.
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: f32,
    /// Global cooldown trigger in seconds.
    #[serde(default = "default_gcd")]
    pub gcd_seconds: f32,
    /// Resource cost (mana, energy, etc.).
    #[serde(default)]
    pub cost: ResourceCost,
    /// Ability effects.
    #[serde(default)]
    pub effects: AbilityEffects,
    /// Optional requirements to use ability.
    #[serde(default)]
    pub requires: AbilityRequirements,
}

fn default_cooldown() -> f32 { 0.0 }
fn default_gcd() -> f32 { 1.0 }

/// Resource cost for using an ability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceCost {
    /// Mana cost.
    #[serde(default)]
    pub mana: u32,
    /// Energy cost.
    #[serde(default)]
    pub energy: u32,
    /// Health cost (life tap style).
    #[serde(default)]
    pub health: u32,
}

/// Ability effects configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AbilityEffects {
    /// Effect type.
    #[serde(rename = "type", default)]
    pub effect_type: EffectType,
    /// Damage amount (for damage effects).
    #[serde(default)]
    pub damage: u32,
    /// Heal amount (for heal effects).
    #[serde(default)]
    pub heal: u32,
    /// Distance (for dash/teleport effects).
    #[serde(default)]
    pub distance: f32,
    /// Duration in seconds (for buffs/debuffs).
    #[serde(default)]
    pub duration: f32,
    /// Area of effect radius.
    #[serde(default)]
    pub radius: f32,
}

/// Types of ability effects.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectType {
    #[default]
    None,
    Damage,
    Heal,
    Dash,
    Teleport,
    Buff,
    Debuff,
    Projectile,
    AreaDamage,
    AreaHeal,
}

/// Requirements for using an ability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AbilityRequirements {
    /// Minimum level required.
    #[serde(default)]
    pub level: u32,
    /// Required memory unlock ID.
    #[serde(default)]
    pub memory_unlock: Option<String>,
    /// Required weapon type.
    #[serde(default)]
    pub weapon_type: Option<String>,
}

impl AbilityDef {
    /// Parse an ability definition from TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Validate the ability definition.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_empty() {
            return Err(ValidationError::EmptyId("ability"));
        }
        if !self.id.starts_with("ability.") {
            return Err(ValidationError::InvalidIdPrefix {
                kind: "ability",
                id: self.id.clone(),
                expected: "ability.",
            });
        }
        if self.name.is_empty() {
            return Err(ValidationError::EmptyName("ability", self.id.clone()));
        }
        if self.cooldown_seconds < 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "cooldown_seconds",
                reason: "cannot be negative",
            });
        }
        if self.gcd_seconds < 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "gcd_seconds",
                reason: "cannot be negative",
            });
        }

        // Validate effect-specific requirements
        match self.effects.effect_type {
            EffectType::Dash | EffectType::Teleport => {
                if self.effects.distance <= 0.0 {
                    return Err(ValidationError::InvalidValue {
                        field: "effects.distance",
                        reason: "must be positive for dash/teleport",
                    });
                }
            }
            EffectType::Damage | EffectType::AreaDamage => {
                if self.effects.damage == 0 {
                    return Err(ValidationError::InvalidValue {
                        field: "effects.damage",
                        reason: "must be set for damage abilities",
                    });
                }
            }
            EffectType::Heal | EffectType::AreaHeal => {
                if self.effects.heal == 0 {
                    return Err(ValidationError::InvalidValue {
                        field: "effects.heal",
                        reason: "must be set for heal abilities",
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Check if this ability triggers GCD.
    pub fn triggers_gcd(&self) -> bool {
        self.gcd_seconds > 0.0
    }

    /// Check if this ability has a cooldown.
    pub fn has_cooldown(&self) -> bool {
        self.cooldown_seconds > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ABILITY_TOML: &str = r#"
id = "ability.memory_dash"
name = "Memory Dash"
cooldown_seconds = 8
gcd_seconds = 1

[effects]
type = "dash"
distance = 8.0
"#;

    const SAMPLE_DAMAGE_TOML: &str = r#"
id = "ability.shadow_bolt"
name = "Shadow Bolt"
cooldown_seconds = 0
gcd_seconds = 1.5

[cost]
mana = 25

[effects]
type = "projectile"
damage = 30
"#;

    #[test]
    fn parse_ability_toml() {
        let ability = AbilityDef::from_toml(SAMPLE_ABILITY_TOML).expect("Failed to parse");
        assert_eq!(ability.id, "ability.memory_dash");
        assert_eq!(ability.name, "Memory Dash");
        assert_eq!(ability.cooldown_seconds, 8.0);
        assert_eq!(ability.effects.effect_type, EffectType::Dash);
        assert_eq!(ability.effects.distance, 8.0);
    }

    #[test]
    fn parse_damage_ability() {
        let ability = AbilityDef::from_toml(SAMPLE_DAMAGE_TOML).expect("Failed to parse");
        assert_eq!(ability.cost.mana, 25);
        assert_eq!(ability.effects.damage, 30);
    }

    #[test]
    fn ability_validation_pass() {
        let ability = AbilityDef::from_toml(SAMPLE_ABILITY_TOML).unwrap();
        assert!(ability.validate().is_ok());
    }

    #[test]
    fn ability_validation_fail_dash_no_distance() {
        let toml = r#"
id = "ability.bad_dash"
name = "Bad Dash"

[effects]
type = "dash"
distance = 0
"#;
        let ability = AbilityDef::from_toml(toml).unwrap();
        assert!(matches!(ability.validate(), Err(ValidationError::InvalidValue { .. })));
    }

    #[test]
    fn ability_triggers_gcd() {
        let ability = AbilityDef::from_toml(SAMPLE_ABILITY_TOML).unwrap();
        assert!(ability.triggers_gcd());
        assert!(ability.has_cooldown());
    }
}
