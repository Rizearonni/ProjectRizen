//! Persistence layer for ProjectRizen.
//!
//! Provides PostgreSQL database access for accounts, characters, and game state.
//!
//! # Features
//!
//! - Account management (create, authenticate, sessions)
//! - Character CRUD operations
//! - Character state persistence (position, zone)
//! - Inventory management
//! - Memory unlock tracking
//!
//! # Usage
//!
//! ```ignore
//! use persistence::{DatabaseConfig, create_pool, AccountRepo, CharacterRepo};
//!
//! let config = DatabaseConfig::from_env()?;
//! let pool = create_pool(&config).await?;
//!
//! let account_repo = AccountRepo::new(&pool);
//! let account = account_repo.get_or_create_dev("test_user").await?;
//!
//! let char_repo = CharacterRepo::new(&pool);
//! let characters = char_repo.get_by_account(account.account_id).await?;
//! ```

pub mod account;
pub mod character;
pub mod error;
pub mod models;
pub mod pool;

pub use account::AccountRepo;
pub use character::{CharacterRepo, MAX_CHARACTERS_PER_ACCOUNT};
pub use error::{PersistenceError, Result};
pub use models::*;
pub use pool::{create_pool, create_pool_from_env, run_migrations, DatabaseConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 1);
        assert!(config.url.contains("rizen"));
    }

    #[test]
    fn test_database_config_testing() {
        let config = DatabaseConfig::for_testing();
        assert_eq!(config.max_connections, 2);
        assert!(config.url.contains("rizen_test"));
    }

    #[test]
    fn test_new_account_struct() {
        let new_account = NewAccount {
            username: "test_user".to_string(),
            password_hash: Some("hash123".to_string()),
        };
        assert_eq!(new_account.username, "test_user");
    }

    #[test]
    fn test_new_character_struct() {
        let account_id = Uuid::new_v4();
        let new_char = NewCharacter {
            account_id,
            name: "TestHero".to_string(),
            appearance_seed: 42,
        };
        assert_eq!(new_char.name, "TestHero");
        assert_eq!(new_char.appearance_seed, 42);
    }

    #[test]
    fn test_character_state_update() {
        let update = CharacterStateUpdate {
            zone_id: "ossuary".to_string(),
            pos_x: 10.0,
            pos_y: 5.0,
            pos_z: 0.0,
            yaw: 1.57,
        };
        assert_eq!(update.zone_id, "ossuary");
        assert_eq!(update.pos_x, 10.0);
    }

    #[test]
    fn test_persistence_error_display() {
        let err = PersistenceError::AccountNotFound("unknown_user".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("account not found"));
    }

    #[test]
    fn test_persistence_error_character_not_found() {
        let id = Uuid::new_v4();
        let err = PersistenceError::CharacterNotFound(id);
        let msg = format!("{}", err);
        assert!(msg.contains("character not found"));
    }

    #[test]
    fn test_persistence_error_name_exists() {
        let err = PersistenceError::CharacterNameExists("TakenName".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("TakenName"));
    }

    #[test]
    fn test_persistence_error_max_characters() {
        let err = PersistenceError::MaxCharactersReached(5);
        let msg = format!("{}", err);
        assert!(msg.contains("5"));
    }

    #[test]
    fn test_max_characters_constant() {
        assert_eq!(MAX_CHARACTERS_PER_ACCOUNT, 5);
    }

    #[test]
    fn test_character_summary_struct() {
        let summary = CharacterSummary {
            character_id: Uuid::new_v4(),
            name: "Hero".to_string(),
            level: 10,
            zone_id: Some("ossuary".to_string()),
        };
        assert_eq!(summary.level, 10);
        assert_eq!(summary.zone_id.unwrap(), "ossuary");
    }
}
