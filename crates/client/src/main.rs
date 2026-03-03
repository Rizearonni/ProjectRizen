//! Game Client - entry point.
//!
//! Opens a window with wgpu + egui, connects to zone server via WebSocket,
//! handles input, and displays entity positions.

mod game_state;
mod input;
mod network;
mod renderer;
mod ui;

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use winit::event_loop::EventLoop;

use crate::game_state::GameState;
use crate::renderer::Renderer;

/// Client configuration.
pub struct Config {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub server_url: String,
    pub input_rate: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_title: "Project Rizen".to_string(),
            window_width: 1280,
            window_height: 720,
            server_url: "ws://127.0.0.1:3000/ws/zone".to_string(),
            input_rate: 20, // 20 Hz input send rate
        }
    }
}

fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting game client...");

    let config = Config::default();

    // Create event loop
    let event_loop = EventLoop::new()?;

    // Create shared game state
    let game_state = Arc::new(RwLock::new(GameState::new(config.server_url.clone())));

    // Create and run renderer (owns the window and main loop)
    let mut renderer = pollster::block_on(Renderer::new(&event_loop, &config, game_state))?;

    info!("Client initialized, starting main loop");

    // Run the event loop
    event_loop.run_app(&mut renderer)?;

    Ok(())
}
