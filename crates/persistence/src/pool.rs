//! Database connection pool management.
//!
//! Provides connection pool creation and configuration.

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tracing::info;

use crate::error::Result;

/// Database configuration.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL.
    pub url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
    /// Minimum number of connections to keep open.
    pub min_connections: u32,
    /// Connection timeout in seconds.
    pub connect_timeout_secs: u64,
    /// Idle connection timeout in seconds.
    pub idle_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/rizen".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_secs: 10,
            idle_timeout_secs: 300,
        }
    }
}

impl DatabaseConfig {
    /// Create config from environment variables.
    ///
    /// Looks for:
    /// - DATABASE_URL (required)
    /// - DATABASE_MAX_CONNECTIONS (optional, default 10)
    /// - DATABASE_MIN_CONNECTIONS (optional, default 1)
    /// - DATABASE_CONNECT_TIMEOUT (optional, default 10)
    /// - DATABASE_IDLE_TIMEOUT (optional, default 300)
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rizen".to_string());

        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let min_connections = std::env::var("DATABASE_MIN_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let connect_timeout_secs = std::env::var("DATABASE_CONNECT_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let idle_timeout_secs = std::env::var("DATABASE_IDLE_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);

        Ok(Self {
            url,
            max_connections,
            min_connections,
            connect_timeout_secs,
            idle_timeout_secs,
        })
    }

    /// Create config for testing with default SQLite.
    pub fn for_testing() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/rizen_test".to_string(),
            max_connections: 2,
            min_connections: 1,
            connect_timeout_secs: 5,
            idle_timeout_secs: 60,
        }
    }
}

/// Create a database connection pool.
pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool> {
    info!(
        "Connecting to database (max_connections={}, min_connections={})",
        config.max_connections, config.min_connections
    );

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(Duration::from_secs(config.connect_timeout_secs))
        .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
        .connect(&config.url)
        .await?;

    info!("Database connection pool created");
    Ok(pool)
}

/// Create a database connection pool from environment variables.
pub async fn create_pool_from_env() -> Result<PgPool> {
    let config = DatabaseConfig::from_env()?;
    create_pool(&config).await
}

/// Run pending migrations.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    info!("Running database migrations...");
    
    // For now, we'll use a simple approach - check if tables exist
    // In production, use sqlx-cli: `sqlx migrate run`
    let tables_exist = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'accounts'",
    )
    .fetch_one(pool)
    .await?;

    if tables_exist == 0 {
        info!("Running initial schema migration...");
        
        // Run the initial schema
        sqlx::query(include_str!("../migrations/001_initial_schema.sql"))
            .execute(pool)
            .await?;
        
        info!("Initial schema migration complete");
    } else {
        info!("Database schema already exists");
    }

    Ok(())
}
