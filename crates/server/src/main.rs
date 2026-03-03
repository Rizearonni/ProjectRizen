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
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::world::World;
use crate::zone::ws_handler;

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

    // Create shared state
    let state = Arc::new(AppState {
        world: RwLock::new(World::new()),
        config,
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
