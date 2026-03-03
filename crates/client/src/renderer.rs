//! Renderer with winit + wgpu + egui integration.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};
use wgpu::SurfaceError;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::game_state::GameState;
use crate::input::{handle_key_event, has_movement};
use crate::network::{start_network_task, NetCommand};
use crate::ui::draw_ui;
use crate::Config;

/// How often to update the window title (seconds).
const TITLE_UPDATE_INTERVAL: f64 = 1.0;

/// Main renderer holding all graphics state.
pub struct Renderer {
    // Core state
    game_state: Arc<RwLock<GameState>>,
    net_cmd_tx: mpsc::Sender<NetCommand>,
    config: Config,

    // Window and graphics (initialized on resume)
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_config: Option<wgpu::SurfaceConfiguration>,

    // Egui state
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,

    // Timing
    last_input_time: Instant,
    last_title_update: Instant,
    client_tick: u32,

    // UI state
    server_url_input: String,
}

impl Renderer {
    pub async fn new(
        _event_loop: &EventLoop<()>,
        config: &Config,
        game_state: Arc<RwLock<GameState>>,
    ) -> Result<Self> {
        // Start network task
        let net_cmd_tx = start_network_task(Arc::clone(&game_state));

        let server_url = {
            let state = game_state.read().await;
            state.server_url.clone()
        };

        Ok(Self {
            game_state,
            net_cmd_tx,
            config: Config {
                window_title: config.window_title.clone(),
                window_width: config.window_width,
                window_height: config.window_height,
                server_url: config.server_url.clone(),
                input_rate: config.input_rate,
            },
            window: None,
            surface: None,
            device: None,
            queue: None,
            surface_config: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            egui_renderer: None,
            last_input_time: Instant::now(),
            last_title_update: Instant::now(),
            client_tick: 0,
            server_url_input: server_url,
        })
    }

    /// Initialize graphics when window is available.
    fn init_graphics(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        info!("Using adapter: {:?}", adapter.get_info().name);

        // Create device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Initialize egui
        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        self.window = Some(window);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface_config = Some(surface_config);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
    }

    /// Handle window resize.
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            if let (Some(surface), Some(device), Some(config)) =
                (&self.surface, &self.device, &mut self.surface_config)
            {
                config.width = new_size.width;
                config.height = new_size.height;
                surface.configure(device, config);
            }
        }
    }

    /// Render a frame.
    fn render(&mut self) -> Result<(), SurfaceError> {
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let window = self.window.as_ref().unwrap();

        let output = surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Begin egui frame
        let raw_input = self.egui_state.as_mut().unwrap().take_egui_input(window);
        self.egui_ctx.begin_pass(raw_input);

        // Draw UI (read game state)
        let mut connect_requested = false;
        let mut disconnect_requested = false;

        {
            let game_state = pollster::block_on(self.game_state.read());
            draw_ui(
                &self.egui_ctx,
                &game_state,
                &mut connect_requested,
                &mut disconnect_requested,
                &mut self.server_url_input,
            );
        }

        // Handle connect/disconnect requests
        if connect_requested {
            // Update server URL in game state
            {
                let mut state = pollster::block_on(self.game_state.write());
                state.server_url = self.server_url_input.clone();
            }
            let _ = self.net_cmd_tx.try_send(NetCommand::Connect);
        }
        if disconnect_requested {
            let _ = self.net_cmd_tx.try_send(NetCommand::Disconnect);
        }

        // End egui frame
        let full_output = self.egui_ctx.end_pass();

        // Handle platform output
        self.egui_state.as_mut().unwrap().handle_platform_output(window, full_output.platform_output);

        // Prepare egui primitives
        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        
        // Screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                self.surface_config.as_ref().unwrap().width,
                self.surface_config.as_ref().unwrap().height,
            ],
            pixels_per_point: window.scale_factor() as f32,
        };

        // Update egui textures
        let textures_delta = full_output.textures_delta;
        for (id, delta) in &textures_delta.set {
            self.egui_renderer.as_mut().unwrap().update_texture(device, queue, *id, delta);
        }

        // Build encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Update egui buffers
        self.egui_renderer.as_mut().unwrap().update_buffers(
            device, 
            queue, 
            &mut encoder, 
            &clipped_primitives, 
            &screen_descriptor
        );

        // Clear screen render pass (egui render integration pending)
        // TODO: Fix egui-wgpu lifetime issues with RenderPass<'static> requirement
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // egui rendering temporarily disabled due to lifetime issues
            // with egui-wgpu 0.30's Renderer::render() requiring 'static RenderPass
        }

        // Submit
        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Free textures
        for id in &textures_delta.free {
            self.egui_renderer.as_mut().unwrap().free_texture(id);
        }

        Ok(())
    }

    /// Send input at configured rate.
    fn maybe_send_input(&mut self) {
        let input_interval = Duration::from_secs_f64(1.0 / self.config.input_rate as f64);
        let now = Instant::now();

        if now.duration_since(self.last_input_time) >= input_interval {
            self.last_input_time = now;
            self.client_tick = self.client_tick.wrapping_add(1);

            // Queue and send input if connected and has movement
            let input = {
                let mut state = pollster::block_on(self.game_state.write());
                if state.is_connected() && has_movement(&state.move_input) {
                    state.queue_input(self.client_tick);
                    state.take_pending_input()
                } else {
                    None
                }
            };

            if let Some(input) = input {
                let _ = self.net_cmd_tx.try_send(NetCommand::SendInput(input));
            }
        }
    }

    /// Update window title with HUD stats (once per second).
    fn maybe_update_title(&mut self) {
        let title_interval = Duration::from_secs_f64(TITLE_UPDATE_INTERVAL);
        let now = Instant::now();

        if now.duration_since(self.last_title_update) >= title_interval {
            self.last_title_update = now;

            if let Some(window) = &self.window {
                let hud_stats = {
                    let state = pollster::block_on(self.game_state.read());
                    state.build_hud_stats(self.client_tick)
                };
                window.set_title(&hud_stats.to_title());
            }
        }
    }
}

impl ApplicationHandler for Renderer {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Poll | StartCause::ResumeTimeReached { .. }) {
            self.maybe_send_input();
            self.maybe_update_title();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attrs = Window::default_attributes()
                .with_title(&self.config.window_title)
                .with_inner_size(PhysicalSize::new(
                    self.config.window_width,
                    self.config.window_height,
                ));

            let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
            self.init_graphics(window);
            info!("Window created and graphics initialized");

            // Auto-connect to server
            info!("Auto-connecting to server...");
            let _ = self.net_cmd_tx.try_send(NetCommand::Connect);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Let egui handle events first
        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let response = egui_state.on_window_event(window, &event);
                if response.consumed {
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested, exiting");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                self.resize(size);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Update move input
                let mut state = pollster::block_on(self.game_state.write());
                handle_key_event(&mut state.move_input, &event);
            }

            WindowEvent::RedrawRequested => {
                match self.render() {
                    Ok(_) => {}
                    Err(SurfaceError::Lost) => {
                        if let Some(config) = &self.surface_config {
                            self.resize(PhysicalSize::new(config.width, config.height));
                        }
                    }
                    Err(SurfaceError::OutOfMemory) => {
                        error!("Out of memory!");
                        event_loop.exit();
                    }
                    Err(e) => {
                        debug!("Surface error: {:?}", e);
                    }
                }

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.maybe_send_input();
        self.maybe_update_title();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
