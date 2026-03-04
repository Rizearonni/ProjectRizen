//! Zone Server - entry point.
//!
//! Hosts WebSocket endpoint at `/ws/zone` for game clients.
//! Runs a tick loop at 20Hz and broadcasts world snapshots at 10Hz.

mod world;
mod zone;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Router};
use tokio::sync::RwLock;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use crate::world::World;
use crate::zone::ws_handler;

/// Parse DATABASE_URL to extract host and database name for logging (no password).
fn parse_db_url_for_logging(url: &str) -> (String, String) {
    // URL format: postgres://user:pass@host:port/database
    // We want to extract host and database without exposing password
    let without_scheme = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))
        .unwrap_or(url);
    
    // Find @ to skip user:pass
    let after_auth = without_scheme
        .find('@')
        .map(|i| &without_scheme[i + 1..])
        .unwrap_or(without_scheme);
    
    // Split host:port/database
    let (host_port, database) = after_auth
        .find('/')
        .map(|i| (&after_auth[..i], &after_auth[i + 1..]))
        .unwrap_or((after_auth, "unknown"));
    
    // Remove query params from database name
    let database = database
        .find('?')
        .map(|i| &database[..i])
        .unwrap_or(database);
    
    (host_port.to_string(), database.to_string())
}

/// Initialize persistence layer if DATABASE_URL is set.
async fn init_persistence() -> Result<Option<persistence::PgPool>> {
    let db_url = std::env::var("DATABASE_URL");
    
    match db_url {
        Ok(url) => {
            let (host, database) = parse_db_url_for_logging(&url);
            info!("Persistence: enabled (host={}, database={})", host, database);
            
            // Create connection pool
            let config = persistence::DatabaseConfig::from_env()
                .map_err(|e| anyhow::anyhow!("Failed to load database config: {}", e))?;
            
            let pool = persistence::create_pool(&config)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create database pool: {}", e))?;
            
            // Run migrations
            info!("Running migrations...");
            match persistence::run_migrations(&pool).await {
                Ok(()) => {
                    info!("Migrations complete");
                    Ok(Some(pool))
                }
                Err(e) => {
                    error!("Migrations failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(_) => {
            warn!("Persistence: disabled (no DATABASE_URL)");
            Ok(None)
        }
    }
}

/// Server configuration.
pub struct Config {
    pub bind_addr: SocketAddr,
    pub tick_rate: u32,
    pub snapshot_rate: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:3000".parse().unwrap(),
            tick_rate: 20,      // 20 ticks per second
            snapshot_rate: 10,  // 10 snapshots per second (every other tick)
        }
    }
}

/// Shared server state.
pub struct AppState {
    pub world: RwLock<World>,
    pub config: Config,
    /// Database pool (None if persistence disabled).
    pub db_pool: Option<persistence::PgPool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = Config::default();
    let bind_addr = config.bind_addr;

    info!("Starting zone server...");
    info!("Tick rate: {} Hz, Snapshot rate: {} Hz", config.tick_rate, config.snapshot_rate);

    // Initialize persistence (database connection + migrations)
    let pool = init_persistence().await?;

    // Create shared state
    let state = Arc::new(AppState {
        world: RwLock::new(World::new()),
        config,
        db_pool: pool,
    });

    // Spawn the tick loop
    let tick_state = Arc::clone(&state);
    tokio::spawn(async move {
        zone::tick_loop(tick_state).await;
    });

    // Build router
    let app = Router::new()
        .route("/ws/zone", get(ws_handler))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!("Zone server listening on {}", bind_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
