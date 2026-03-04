//! Renderer with 3D terrain and entity rendering.
//!
//! Uses wgpu for hardware-accelerated rendering with:
//! - Procedural terrain chunks from worldgen
//! - Entity cubes for players
//! - Simple height-based terrain coloring

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};
use wgpu::SurfaceError;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::camera::{Camera, CameraInput};
use crate::game_state::GameState;
use crate::input::{handle_key_event, has_movement};
use crate::network::{start_network_task, NetCommand};
use crate::shaders::{ENTITY_SHADER, TERRAIN_SHADER};
use crate::terrain::{create_cube_mesh, CubeVertex, TerrainCache, TerrainVertex};
use crate::Config;

/// How often to update the window title (seconds).
const TITLE_UPDATE_INTERVAL: f64 = 1.0;

/// Chunk streaming radius.
const CHUNK_RADIUS: i32 = 3;

/// Camera uniform buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniforms {
    view_proj: [[f32; 4]; 4],
}

/// Entity uniform buffer (per-entity).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct EntityUniforms {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

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

    // Depth buffer
    depth_texture: Option<wgpu::TextureView>,

    // Camera
    camera: Camera,
    camera_input: CameraInput,
    camera_uniform_buffer: Option<wgpu::Buffer>,
    camera_bind_group: Option<wgpu::BindGroup>,

    // Terrain rendering
    terrain_pipeline: Option<wgpu::RenderPipeline>,
    terrain_cache: TerrainCache,

    // Entity rendering
    entity_pipeline: Option<wgpu::RenderPipeline>,
    cube_vertex_buffer: Option<wgpu::Buffer>,
    cube_index_buffer: Option<wgpu::Buffer>,
    cube_index_count: u32,
    entity_uniform_buffer: Option<wgpu::Buffer>,
    entity_bind_group_layout: Option<wgpu::BindGroupLayout>,

    // Egui state (kept for future UI)
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,

    // Timing
    last_frame_time: Instant,
    last_input_time: Instant,
    last_title_update: Instant,
    client_tick: u32,

    // UI state
    #[allow(dead_code)]
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

        let now = Instant::now();

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
            depth_texture: None,
            camera: Camera::default(),
            camera_input: CameraInput::default(),
            camera_uniform_buffer: None,
            camera_bind_group: None,
            terrain_pipeline: None,
            terrain_cache: TerrainCache::new(),
            entity_pipeline: None,
            cube_vertex_buffer: None,
            cube_index_buffer: None,
            cube_index_count: 0,
            entity_uniform_buffer: None,
            entity_bind_group_layout: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            egui_renderer: None,
            last_frame_time: now,
            last_input_time: now,
            last_title_update: now,
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

        // Create depth texture
        let depth_texture = self.create_depth_texture(&device, &surface_config);

        // Create camera uniform buffer and bind group
        let (camera_uniform_buffer, camera_bind_group_layout, camera_bind_group) =
            self.create_camera_resources(&device);

        // Create terrain pipeline
        let terrain_pipeline =
            self.create_terrain_pipeline(&device, surface_format, &camera_bind_group_layout);

        // Create entity pipeline
        let (entity_pipeline, entity_bind_group_layout) =
            self.create_entity_pipeline(&device, surface_format, &camera_bind_group_layout);

        // Create cube mesh
        let (cube_vb, cube_ib, cube_idx_count) = create_cube_mesh(&device);

        // Create entity uniform buffer (large enough for many entities)
        let entity_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Entity Uniform Buffer"),
            size: std::mem::size_of::<EntityUniforms>() as u64 * 100, // Room for 100 entities
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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

        // Store everything
        self.window = Some(window);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface_config = Some(surface_config);
        self.depth_texture = Some(depth_texture);
        self.camera_uniform_buffer = Some(camera_uniform_buffer);
        self.camera_bind_group = Some(camera_bind_group);
        self.terrain_pipeline = Some(terrain_pipeline);
        self.entity_pipeline = Some(entity_pipeline);
        self.entity_bind_group_layout = Some(entity_bind_group_layout);
        self.cube_vertex_buffer = Some(cube_vb);
        self.cube_index_buffer = Some(cube_ib);
        self.cube_index_count = cube_idx_count;
        self.entity_uniform_buffer = Some(entity_uniform_buffer);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
    }

    /// Create depth texture.
    fn create_depth_texture(
        &self,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Create camera uniform buffer and bind group.
    fn create_camera_resources(
        &self,
        device: &wgpu::Device,
    ) -> (wgpu::Buffer, wgpu::BindGroupLayout, wgpu::BindGroup) {
        let camera_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });

        (camera_uniform_buffer, bind_group_layout, bind_group)
    }

    /// Create terrain render pipeline.
    fn create_terrain_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(TERRAIN_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terrain Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[TerrainVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// Create entity render pipeline.
    fn create_entity_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Entity Shader"),
            source: wgpu::ShaderSource::Wgsl(ENTITY_SHADER.into()),
        });

        // Entity-specific bind group layout
        let entity_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Entity Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true, // Dynamic offset for each entity
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<EntityUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Entity Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &entity_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Entity Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[CubeVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        (pipeline, entity_bind_group_layout)
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

            // Recreate depth texture (separate borrow scope)
            if let (Some(device), Some(config)) = (&self.device, &self.surface_config) {
                self.depth_texture = Some(self.create_depth_texture(device, config));
            }
        }
    }

    /// Render a frame.
    fn render(&mut self) -> Result<(), SurfaceError> {
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let _window = self.window.as_ref().unwrap();
        let config = self.surface_config.as_ref().unwrap();

        // Calculate delta time
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Apply camera input
        self.camera_input.apply(&mut self.camera, dt);

        // Update camera uniforms
        let aspect = config.width as f32 / config.height as f32;
        let camera_uniforms = CameraUniforms {
            view_proj: self.camera.view_projection_matrix(aspect),
        };
        queue.write_buffer(
            self.camera_uniform_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&[camera_uniforms]),
        );

        // Update terrain chunks around camera
        self.terrain_cache.update_chunks(
            device,
            self.camera.position[0] as f64,
            self.camera.position[2] as f64,
            CHUNK_RADIUS,
        );

        // Get entity data from game state
        let entities: Vec<_> = {
            let state = pollster::block_on(self.game_state.read());
            state
                .entities
                .iter()
                .map(|(id, transform)| {
                    let is_local = state.local_entity_id == Some(*id);
                    (*id, *transform, is_local)
                })
                .collect()
        };

        // Update entity uniforms
        let entity_size = std::mem::size_of::<EntityUniforms>();
        let aligned_size = align_to(entity_size, 256); // wgpu requires 256-byte alignment for dynamic offsets
        let mut entity_data = vec![0u8; aligned_size * entities.len().max(1)];

        for (i, (_id, transform, is_local)) in entities.iter().enumerate() {
            let uniforms = EntityUniforms {
                model: translation_matrix(transform.pos.x, transform.pos.y + 1.0, transform.pos.z),
                color: if *is_local {
                    [0.2, 0.8, 0.3, 1.0] // Green for local player
                } else {
                    [0.8, 0.3, 0.2, 1.0] // Red for remote players
                },
            };
            let offset = i * aligned_size;
            let bytes = bytemuck::bytes_of(&uniforms);
            entity_data[offset..offset + entity_size].copy_from_slice(bytes);
        }

        if !entity_data.is_empty() {
            queue.write_buffer(
                self.entity_uniform_buffer.as_ref().unwrap(),
                0,
                &entity_data,
            );
        }

        // Create entity bind group with dynamic offsets
        let entity_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Entity Bind Group"),
            layout: self.entity_bind_group_layout.as_ref().unwrap(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: self.entity_uniform_buffer.as_ref().unwrap(),
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<EntityUniforms>() as u64),
                }),
            }],
        });

        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Main 3D render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3D Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.4,
                            g: 0.5,
                            b: 0.6,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.depth_texture.as_ref().unwrap(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render terrain
            render_pass.set_pipeline(self.terrain_pipeline.as_ref().unwrap());
            render_pass.set_bind_group(0, self.camera_bind_group.as_ref().unwrap(), &[]);

            // Set shared index buffer once
            if let Some((index_buffer, index_count)) = self.terrain_cache.index_buffer() {
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                for chunk_mesh in self.terrain_cache.meshes() {
                    render_pass.set_vertex_buffer(0, chunk_mesh.vertex_buffer.slice(..));
                    render_pass.draw_indexed(0..index_count, 0, 0..1);
                }
            }

            // Render entities
            if !entities.is_empty() {
                render_pass.set_pipeline(self.entity_pipeline.as_ref().unwrap());
                render_pass.set_bind_group(0, self.camera_bind_group.as_ref().unwrap(), &[]);
                render_pass.set_vertex_buffer(0, self.cube_vertex_buffer.as_ref().unwrap().slice(..));
                render_pass.set_index_buffer(
                    self.cube_index_buffer.as_ref().unwrap().slice(..),
                    wgpu::IndexFormat::Uint32,
                );

                let aligned_size = align_to(std::mem::size_of::<EntityUniforms>(), 256);
                for (i, _) in entities.iter().enumerate() {
                    let offset = (i * aligned_size) as u32;
                    render_pass.set_bind_group(1, &entity_bind_group, &[offset]);
                    render_pass.draw_indexed(0..self.cube_index_count, 0, 0..1);
                }
            }
        }

        // Submit
        queue.submit(std::iter::once(encoder.finish()));
        output.present();

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
                let chunks = self.terrain_cache.chunk_count();
                let title = format!(
                    "{} — chunks: {}",
                    hud_stats.to_title(),
                    chunks
                );
                window.set_title(&title);
            }
        }
    }

    /// Handle camera input from keyboard.
    fn handle_camera_input(&mut self, event: &winit::event::KeyEvent) {
        let pressed = event.state == ElementState::Pressed;

        if let PhysicalKey::Code(code) = event.physical_key {
            match code {
                KeyCode::KeyW => self.camera_input.forward = pressed,
                KeyCode::KeyS => self.camera_input.backward = pressed,
                KeyCode::KeyA => self.camera_input.left = pressed,
                KeyCode::KeyD => self.camera_input.right = pressed,
                KeyCode::Space => self.camera_input.up = pressed,
                KeyCode::ShiftLeft | KeyCode::ShiftRight => self.camera_input.down = pressed,
                KeyCode::KeyQ => self.camera_input.yaw_left = pressed,
                KeyCode::KeyE => self.camera_input.yaw_right = pressed,
                KeyCode::KeyR => self.camera_input.pitch_up = pressed,
                KeyCode::KeyF => self.camera_input.pitch_down = pressed,
                _ => {}
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
        // Let egui handle events first (for future UI)
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
                // Handle camera input
                self.handle_camera_input(&event);

                // Also update network move input for server sync
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

// Helper functions

/// Create a translation matrix.
fn translation_matrix(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [x, y, z, 1.0],
    ]
}

/// Align value to next multiple of alignment.
fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) / alignment * alignment
}
