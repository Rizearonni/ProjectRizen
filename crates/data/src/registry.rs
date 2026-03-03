//! Data registry for loading and accessing game definitions.
//!
//! The registry loads all TOML definition files from a data directory
//! and provides type-safe access to zones, mobs, abilities, etc.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tracing::{debug, info, warn};

use crate::ability::AbilityDef;
use crate::mob::MobDef;
use crate::zone::{ValidationError, ZoneDef};

/// Error type for registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error in {file}: {error}")]
    TomlParse { file: String, error: String },

    #[error("validation error in {file}: {error}")]
    Validation { file: String, error: ValidationError },

    #[error("duplicate ID: {id}")]
    DuplicateId { id: String },

    #[error("data directory not found: {path}")]
    DataDirNotFound { path: String },
}

/// Central registry of all game data definitions.
#[derive(Debug, Default)]
pub struct DataRegistry {
    /// Zone definitions by ID.
    pub zones: HashMap<String, ZoneDef>,
    /// Mob definitions by ID.
    pub mobs: HashMap<String, MobDef>,
    /// Ability definitions by ID.
    pub abilities: HashMap<String, AbilityDef>,
}

impl DataRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all data from a directory.
    ///
    /// Expected structure:
    /// ```text
    /// data_path/
    ///   zones/
    ///     ossuary.toml
    ///   mobs/
    ///     skeleton_scout.toml
    ///   abilities/
    ///     memory_dash.toml
    /// ```
    pub fn load_from_dir<P: AsRef<Path>>(data_path: P) -> Result<Self, RegistryError> {
        let data_path = data_path.as_ref();

        if !data_path.exists() {
            return Err(RegistryError::DataDirNotFound {
                path: data_path.display().to_string(),
            });
        }

        let mut registry = Self::new();

        // Load zones
        let zones_path = data_path.join("zones");
        if zones_path.exists() {
            registry.load_zones(&zones_path)?;
        } else {
            debug!("No zones directory found at {:?}", zones_path);
        }

        // Load mobs
        let mobs_path = data_path.join("mobs");
        if mobs_path.exists() {
            registry.load_mobs(&mobs_path)?;
        } else {
            debug!("No mobs directory found at {:?}", mobs_path);
        }

        // Load abilities
        let abilities_path = data_path.join("abilities");
        if abilities_path.exists() {
            registry.load_abilities(&abilities_path)?;
        } else {
            debug!("No abilities directory found at {:?}", abilities_path);
        }

        info!(
            "Loaded {} zones, {} mobs, {} abilities",
            registry.zones.len(),
            registry.mobs.len(),
            registry.abilities.len()
        );

        Ok(registry)
    }

    /// Load zone definitions from a directory.
    fn load_zones(&mut self, path: &Path) -> Result<(), RegistryError> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.extension().map_or(false, |e| e == "toml") {
                self.load_zone_file(&file_path)?;
            }
        }
        Ok(())
    }

    /// Load a single zone file.
    fn load_zone_file(&mut self, path: &Path) -> Result<(), RegistryError> {
        let file_name = path.display().to_string();
        let content = fs::read_to_string(path)?;

        let zone = ZoneDef::from_toml(&content).map_err(|e| RegistryError::TomlParse {
            file: file_name.clone(),
            error: e.to_string(),
        })?;

        zone.validate().map_err(|e| RegistryError::Validation {
            file: file_name.clone(),
            error: e,
        })?;

        if self.zones.contains_key(&zone.id) {
            return Err(RegistryError::DuplicateId { id: zone.id.clone() });
        }

        debug!("Loaded zone: {}", zone.id);
        self.zones.insert(zone.id.clone(), zone);
        Ok(())
    }

    /// Load mob definitions from a directory.
    fn load_mobs(&mut self, path: &Path) -> Result<(), RegistryError> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.extension().map_or(false, |e| e == "toml") {
                self.load_mob_file(&file_path)?;
            }
        }
        Ok(())
    }

    /// Load a single mob file.
    fn load_mob_file(&mut self, path: &Path) -> Result<(), RegistryError> {
        let file_name = path.display().to_string();
        let content = fs::read_to_string(path)?;

        let mob = MobDef::from_toml(&content).map_err(|e| RegistryError::TomlParse {
            file: file_name.clone(),
            error: e.to_string(),
        })?;

        mob.validate().map_err(|e| RegistryError::Validation {
            file: file_name.clone(),
            error: e,
        })?;

        if self.mobs.contains_key(&mob.id) {
            return Err(RegistryError::DuplicateId { id: mob.id.clone() });
        }

        debug!("Loaded mob: {}", mob.id);
        self.mobs.insert(mob.id.clone(), mob);
        Ok(())
    }

    /// Load ability definitions from a directory.
    fn load_abilities(&mut self, path: &Path) -> Result<(), RegistryError> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.extension().map_or(false, |e| e == "toml") {
                self.load_ability_file(&file_path)?;
            }
        }
        Ok(())
    }

    /// Load a single ability file.
    fn load_ability_file(&mut self, path: &Path) -> Result<(), RegistryError> {
        let file_name = path.display().to_string();
        let content = fs::read_to_string(path)?;

        let ability = AbilityDef::from_toml(&content).map_err(|e| RegistryError::TomlParse {
            file: file_name.clone(),
            error: e.to_string(),
        })?;

        ability.validate().map_err(|e| RegistryError::Validation {
            file: file_name.clone(),
            error: e,
        })?;

        if self.abilities.contains_key(&ability.id) {
            return Err(RegistryError::DuplicateId {
                id: ability.id.clone(),
            });
        }

        debug!("Loaded ability: {}", ability.id);
        self.abilities.insert(ability.id.clone(), ability);
        Ok(())
    }

    /// Get a zone by ID.
    pub fn get_zone(&self, id: &str) -> Option<&ZoneDef> {
        self.zones.get(id)
    }

    /// Get a mob by ID.
    pub fn get_mob(&self, id: &str) -> Option<&MobDef> {
        self.mobs.get(id)
    }

    /// Get an ability by ID.
    pub fn get_ability(&self, id: &str) -> Option<&AbilityDef> {
        self.abilities.get(id)
    }

    /// Validate all cross-references between definitions.
    ///
    /// Call this after loading to verify:
    /// - Spawn regions reference valid mob IDs
    /// - Loot tables reference valid items
    /// - Abilities reference valid buffs/debuffs
    pub fn validate_references(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check spawn region mob references
        for zone in self.zones.values() {
            for region in &zone.spawns.region {
                if !self.mobs.contains_key(&region.mob_id) {
                    errors.push(ValidationError::UnknownReference {
                        kind: "mob",
                        id: region.mob_id.clone(),
                    });
                    warn!(
                        "Zone '{}' spawn region '{}' references unknown mob '{}'",
                        zone.id, region.name, region.mob_id
                    );
                }
            }
        }

        // Check mob loot table references (placeholder - loot tables not yet implemented)
        for mob in self.mobs.values() {
            if let Some(table) = &mob.loot.table {
                // TODO: Validate against loot table registry when implemented
                debug!("Mob '{}' references loot table '{}'", mob.id, table);
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_data_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create subdirectories
        fs::create_dir(dir.path().join("zones")).unwrap();
        fs::create_dir(dir.path().join("mobs")).unwrap();
        fs::create_dir(dir.path().join("abilities")).unwrap();

        // Create zone file
        let mut zone_file = fs::File::create(dir.path().join("zones/test_zone.toml")).unwrap();
        writeln!(
            zone_file,
            r#"
id = "zone.test"
name = "Test Zone"
seed = 12345
"#
        )
        .unwrap();

        // Create mob file
        let mut mob_file = fs::File::create(dir.path().join("mobs/test_mob.toml")).unwrap();
        writeln!(
            mob_file,
            r#"
id = "mob.test"
name = "Test Mob"
"#
        )
        .unwrap();

        // Create ability file
        let mut ability_file =
            fs::File::create(dir.path().join("abilities/test_ability.toml")).unwrap();
        writeln!(
            ability_file,
            r#"
id = "ability.test"
name = "Test Ability"
"#
        )
        .unwrap();

        dir
    }

    #[test]
    fn load_from_dir() {
        let dir = create_test_data_dir();
        let registry = DataRegistry::load_from_dir(dir.path()).expect("Failed to load");

        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.mobs.len(), 1);
        assert_eq!(registry.abilities.len(), 1);

        assert!(registry.get_zone("zone.test").is_some());
        assert!(registry.get_mob("mob.test").is_some());
        assert!(registry.get_ability("ability.test").is_some());
    }

    #[test]
    fn detect_duplicate_id() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("zones")).unwrap();

        // Create two zones with same ID
        let mut file1 = fs::File::create(dir.path().join("zones/zone1.toml")).unwrap();
        writeln!(file1, "id = \"zone.dupe\"\nname = \"Zone 1\"\nseed = 1").unwrap();

        let mut file2 = fs::File::create(dir.path().join("zones/zone2.toml")).unwrap();
        writeln!(file2, "id = \"zone.dupe\"\nname = \"Zone 2\"\nseed = 2").unwrap();

        let result = DataRegistry::load_from_dir(dir.path());
        assert!(matches!(result, Err(RegistryError::DuplicateId { .. })));
    }

    #[test]
    fn validate_references_missing_mob() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("zones")).unwrap();

        let mut zone_file = fs::File::create(dir.path().join("zones/zone.toml")).unwrap();
        writeln!(
            zone_file,
            r#"
id = "zone.test"
name = "Test"
seed = 1

[[spawns.region]]
name = "Spawn"
min = [0, 0]
max = [10, 10]
mob_id = "mob.nonexistent"
cap = 5
respawn_seconds = 10
"#
        )
        .unwrap();

        let registry = DataRegistry::load_from_dir(dir.path()).unwrap();
        let errors = registry.validate_references();

        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            ValidationError::UnknownReference { kind: "mob", .. }
        ));
    }
}
