//! Error types for persistence operations.

use thiserror::Error;

/// Persistence error type.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("account not found: {0}")]
    AccountNotFound(String),

    #[error("character not found: {0}")]
    CharacterNotFound(uuid::Uuid),

    #[error("session not found or expired")]
    SessionNotFound,

    #[error("username already exists: {0}")]
    UsernameExists(String),

    #[error("character name already exists: {0}")]
    CharacterNameExists(String),

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("account has maximum characters ({0})")]
    MaxCharactersReached(u32),

    #[error("connection pool not initialized")]
    PoolNotInitialized,
}

/// Result type alias for persistence operations.
pub type Result<T> = std::result::Result<T, PersistenceError>;
