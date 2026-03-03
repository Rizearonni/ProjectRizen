//! Zone definition loading from TOML files.
//!
//! Zone definitions specify terrain parameters, biomes, features,
//! and spawn regions for procedural generation.

use serde::{Deserialize, Serialize};
use worldgen::{BiomeParams, FeatureParams, NoiseParams, ZoneParams};

/// Zone definition loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneDef {
    /// Unique zone ID (e.g., "zone.ossuary").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Zone seed for deterministic generation.
    pub seed: u64,
    /// Terrain generation parameters.
    #[serde(default)]
    pub terrain: TerrainDef,
    /// Biome classification parameters.
    #[serde(default)]
    pub biomes: BiomeParams,
    /// Feature density parameters.
    #[serde(default)]
    pub features: FeatureParams,
    /// Spawn regions.
    #[serde(default)]
    pub spawns: SpawnsDef,
}

/// Terrain generation definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainDef {
    /// Chunk size in meters.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: f64,
    /// Vertices per side of chunk mesh.
    #[serde(default = "default_verts_per_side")]
    pub verts_per_side: u32,
    /// Maximum terrain height.
    #[serde(default = "default_height_scale")]
    pub height_scale: f64,
    /// Noise parameters.
    #[serde(default)]
    pub noise: NoiseParams,
}

fn default_chunk_size() -> f64 { 64.0 }
fn default_verts_per_side() -> u32 { 65 }
fn default_height_scale() -> f64 { 48.0 }

impl Default for TerrainDef {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
            verts_per_side: default_verts_per_side(),
            height_scale: default_height_scale(),
            noise: NoiseParams::default(),
        }
    }
}

impl TerrainDef {
    /// Convert to worldgen ZoneParams with a seed.
    pub fn to_zone_params(&self, seed: u64) -> ZoneParams {
        ZoneParams {
            seed,
            chunk_size: self.chunk_size,
            verts_per_side: self.verts_per_side,
            height_scale: self.height_scale,
            noise: self.noise.clone(),
        }
    }
}

/// Spawns definition containing spawn regions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpawnsDef {
    /// List of spawn regions.
    #[serde(default)]
    pub region: Vec<SpawnRegion>,
}

/// A spawn region for mobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRegion {
    /// Region name (for debugging/tools).
    pub name: String,
    /// Minimum corner [x, z].
    pub min: [f32; 2],
    /// Maximum corner [x, z].
    pub max: [f32; 2],
    /// Mob definition ID to spawn.
    pub mob_id: String,
    /// Maximum number of mobs in this region.
    pub cap: u32,
    /// Respawn time in seconds.
    pub respawn_seconds: f32,
}

impl ZoneDef {
    /// Parse a zone definition from TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Convert to worldgen ZoneParams.
    pub fn to_zone_params(&self) -> ZoneParams {
        self.terrain.to_zone_params(self.seed)
    }

    /// Validate the zone definition.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_empty() {
            return Err(ValidationError::EmptyId("zone"));
        }
        if !self.id.starts_with("zone.") {
            return Err(ValidationError::InvalidIdPrefix {
                kind: "zone",
                id: self.id.clone(),
                expected: "zone.",
            });
        }
        if self.name.is_empty() {
            return Err(ValidationError::EmptyName("zone", self.id.clone()));
        }
        if self.terrain.chunk_size <= 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "terrain.chunk_size",
                reason: "must be positive",
            });
        }
        if self.terrain.verts_per_side < 2 {
            return Err(ValidationError::InvalidValue {
                field: "terrain.verts_per_side",
                reason: "must be at least 2",
            });
        }

        // Validate spawn regions
        for region in &self.spawns.region {
            if region.mob_id.is_empty() {
                return Err(ValidationError::EmptyId("spawn region mob_id"));
            }
            if region.cap == 0 {
                return Err(ValidationError::InvalidValue {
                    field: "spawn region cap",
                    reason: "must be at least 1",
                });
            }
        }

        Ok(())
    }
}

/// Validation error for data definitions.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("{0} ID cannot be empty")]
    EmptyId(&'static str),

    #[error("{kind} ID '{id}' must start with '{expected}'")]
    InvalidIdPrefix {
        kind: &'static str,
        id: String,
        expected: &'static str,
    },

    #[error("{0} '{1}' has empty name")]
    EmptyName(&'static str, String),

    #[error("invalid value for {field}: {reason}")]
    InvalidValue {
        field: &'static str,
        reason: &'static str,
    },

    #[error("reference to unknown {kind}: {id}")]
    UnknownReference { kind: &'static str, id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ZONE_TOML: &str = r#"
id = "zone.ossuary"
name = "The Ossuary"
seed = 1844674407370955161

[terrain]
chunk_size = 64
verts_per_side = 65
height_scale = 48.0

[terrain.noise]
base_freq = 0.002
octaves = 5
lacunarity = 2.0
gain = 0.5

[biomes]
ash_height_max = 0.35
rock_slope_min = 0.60

[features]
bones_density = 0.08
ruins_density = 0.02
lava_crack_density = 0.01

[[spawns.region]]
name = "North Yard"
min = [-256, -256]
max = [256, 256]
mob_id = "mob.skeleton_scout"
cap = 12
respawn_seconds = 20
"#;

    #[test]
    fn parse_zone_toml() {
        let zone = ZoneDef::from_toml(SAMPLE_ZONE_TOML).expect("Failed to parse");
        assert_eq!(zone.id, "zone.ossuary");
        assert_eq!(zone.name, "The Ossuary");
        assert_eq!(zone.seed, 1844674407370955161);
        assert_eq!(zone.terrain.chunk_size, 64.0);
        assert_eq!(zone.spawns.region.len(), 1);
        assert_eq!(zone.spawns.region[0].mob_id, "mob.skeleton_scout");
    }

    #[test]
    fn zone_validation_pass() {
        let zone = ZoneDef::from_toml(SAMPLE_ZONE_TOML).unwrap();
        assert!(zone.validate().is_ok());
    }

    #[test]
    fn zone_validation_fail_empty_id() {
        let toml = r#"
id = ""
name = "Test"
seed = 123
"#;
        let zone = ZoneDef::from_toml(toml).unwrap();
        assert!(matches!(zone.validate(), Err(ValidationError::EmptyId(_))));
    }

    #[test]
    fn zone_validation_fail_bad_prefix() {
        let toml = r#"
id = "bad.ossuary"
name = "Test"
seed = 123
"#;
        let zone = ZoneDef::from_toml(toml).unwrap();
        assert!(matches!(zone.validate(), Err(ValidationError::InvalidIdPrefix { .. })));
    }

    #[test]
    fn zone_to_params() {
        let zone = ZoneDef::from_toml(SAMPLE_ZONE_TOML).unwrap();
        let params = zone.to_zone_params();
        assert_eq!(params.seed, zone.seed);
        assert_eq!(params.chunk_size, zone.terrain.chunk_size);
    }
}
