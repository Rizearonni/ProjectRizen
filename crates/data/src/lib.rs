//! Data loading library for game content definitions.
//!
//! This crate provides TOML-based loading and validation for:
//! - Zone definitions (terrain, spawns, biomes)
//! - Mob definitions (stats, AI, loot)
//! - Ability definitions (cooldowns, effects, costs)
//!
//! # Usage
//!
//! ```ignore
//! use data::DataRegistry;
//!
//! // Load all data from a directory
//! let registry = DataRegistry::load_from_dir("./data")?;
//!
//! // Access definitions by ID
//! let zone = registry.get_zone("zone.ossuary").unwrap();
//! let mob = registry.get_mob("mob.skeleton_scout").unwrap();
//!
//! // Validate cross-references
//! let errors = registry.validate_references();
//! ```
//!
//! # Data Directory Structure
//!
//! ```text
//! data/
//!   zones/
//!     ossuary.toml
//!   mobs/
//!     skeleton_scout.toml
//!   abilities/
//!     memory_dash.toml
//! ```

pub mod ability;
pub mod mob;
pub mod registry;
pub mod zone;

// Re-export main types
pub use ability::{AbilityDef, AbilityEffects, EffectType, ResourceCost};
pub use mob::{LootConfig, MobDef, MobStats};
pub use registry::{DataRegistry, RegistryError};
pub use zone::{SpawnRegion, SpawnsDef, TerrainDef, ValidationError, ZoneDef};
