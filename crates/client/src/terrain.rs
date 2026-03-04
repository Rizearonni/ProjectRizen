//! Terrain chunk mesh generation and caching.
//!
//! Uses worldgen crate to generate heightmaps and builds GPU-ready meshes.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use worldgen::{ChunkCoord, ChunkHeightmap, ZoneParams, generate_chunk_vertices, generate_chunk_indices};

/// Terrain vertex with position and normal.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl TerrainVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TerrainVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// GPU buffers for a single terrain chunk (vertex buffer only, shares index buffer).
pub struct ChunkMesh {
    pub vertex_buffer: wgpu::Buffer,
}

/// Cache of terrain chunks with GPU buffers.
pub struct TerrainCache {
    /// Zone parameters for terrain generation.
    params: ZoneParams,
    /// Cached chunk meshes.
    chunks: HashMap<(i32, i32), ChunkMesh>,
    /// Shared index buffer (all chunks have same topology).
    shared_indices: Option<(wgpu::Buffer, u32)>,
}

impl TerrainCache {
    /// Create a new terrain cache with default zone parameters.
    pub fn new() -> Self {
        Self {
            params: ZoneParams::with_seed(12345), // Default seed for dev
            chunks: HashMap::new(),
            shared_indices: None,
        }
    }

    /// Get zone parameters.
    #[allow(dead_code)]
    pub fn params(&self) -> &ZoneParams {
        &self.params
    }

    /// Ensure chunks within radius of position are loaded.
    pub fn update_chunks(&mut self, device: &wgpu::Device, world_x: f64, world_z: f64, radius: i32) {
        let center = ChunkCoord::from_world(world_x, world_z, self.params.chunk_size);
        
        // Build shared index buffer if not yet created
        if self.shared_indices.is_none() {
            let indices = generate_chunk_indices(self.params.verts_per_side);
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Terrain Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.shared_indices = Some((index_buffer, indices.len() as u32));
        }

        // Generate missing chunks in radius
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let cx = center.cx + dx;
                let cz = center.cz + dz;
                let key = (cx, cz);
                
                if !self.chunks.contains_key(&key) {
                    if let Some(mesh) = self.generate_chunk_mesh(device, cx, cz) {
                        self.chunks.insert(key, mesh);
                    }
                }
            }
        }

        // Optionally remove chunks outside larger radius (prevent memory growth)
        let max_dist = (radius + 2) * (radius + 2);
        self.chunks.retain(|(cx, cz), _| {
            let dx = cx - center.cx;
            let dz = cz - center.cz;
            dx * dx + dz * dz <= max_dist
        });
    }

    /// Generate mesh for a single chunk.
    fn generate_chunk_mesh(&self, device: &wgpu::Device, cx: i32, cz: i32) -> Option<ChunkMesh> {
        let coord = ChunkCoord::new(cx, cz);
        let heightmap = ChunkHeightmap::generate(&self.params, coord);
        
        // Get raw positions from worldgen
        let positions = generate_chunk_vertices(&heightmap, self.params.chunk_size);
        
        // Build vertices with normals
        let vertices = build_vertices_with_normals(&positions, &heightmap);
        
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Chunk ({}, {}) Vertex Buffer", cx, cz)),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Some(ChunkMesh {
            vertex_buffer,
        })
    }

    /// Get all loaded chunk meshes for rendering.
    pub fn meshes(&self) -> impl Iterator<Item = &ChunkMesh> {
        self.chunks.values()
    }

    /// Get shared index buffer and count for rendering.
    pub fn index_buffer(&self) -> Option<(&wgpu::Buffer, u32)> {
        self.shared_indices
            .as_ref()
            .map(|(buf, count)| (buf, *count))
    }

    /// Get chunk count.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

/// Build vertices with computed normals from raw positions.
fn build_vertices_with_normals(positions: &[f32], heightmap: &ChunkHeightmap) -> Vec<TerrainVertex> {
    let verts_per_side = heightmap.verts_per_side as usize;
    let vertex_count = verts_per_side * verts_per_side;
    let mut vertices = Vec::with_capacity(vertex_count);

    for vz in 0..verts_per_side {
        for vx in 0..verts_per_side {
            let idx = vz * verts_per_side + vx;
            let base = idx * 3;
            
            let x = positions[base];
            let y = positions[base + 1];
            let z = positions[base + 2];

            // Compute normal from neighboring heights
            let normal = compute_vertex_normal(heightmap, vx as u32, vz as u32);
            
            vertices.push(TerrainVertex {
                position: [x, y, z],
                normal,
            });
        }
    }

    vertices
}

/// Compute normal at a vertex using central differences.
fn compute_vertex_normal(heightmap: &ChunkHeightmap, vx: u32, vz: u32) -> [f32; 3] {
    let spacing = heightmap.vertex_spacing as f32;
    let max_v = heightmap.verts_per_side - 1;

    // Sample neighboring heights
    let h_center = heightmap.get(vx, vz);
    let h_left = if vx > 0 { heightmap.get(vx - 1, vz) } else { h_center };
    let h_right = if vx < max_v { heightmap.get(vx + 1, vz) } else { h_center };
    let h_down = if vz > 0 { heightmap.get(vx, vz - 1) } else { h_center };
    let h_up = if vz < max_v { heightmap.get(vx, vz + 1) } else { h_center };

    // Central differences
    let dx = (h_right - h_left) / (2.0 * spacing);
    let dz = (h_up - h_down) / (2.0 * spacing);

    // Normal is cross product of tangent vectors
    // tangent_x = (1, dx, 0), tangent_z = (0, dz, 1)
    // normal = tangent_x × tangent_z = (dx, 1, dz) after normalization... wait, let me recalculate
    // Actually: tangent in X direction = (spacing, h_right - h_center, 0) simplified to (1, dx, 0)
    // tangent in Z direction = (0, h_up - h_center, spacing) simplified to (0, dz, 1)
    // cross product: (1, dx, 0) × (0, dz, 1) = (dx*1 - 0*dz, 0*0 - 1*1, 1*dz - dx*0) = (dx, -1, dz)
    // We want upward normal, so negate: (-dx, 1, -dz)
    let nx = -dx;
    let ny = 1.0;
    let nz = -dz;

    // Normalize
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 0.0 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Unit cube vertex data for entity rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CubeVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl CubeVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Create unit cube mesh (centered at origin, size 1x1x1).
pub fn create_cube_mesh(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let half = 0.5;

    // 8 corners of the cube (but we need 24 vertices for proper normals)
    let vertices: [CubeVertex; 24] = [
        // Front face (+Z)
        CubeVertex { position: [-half, -half,  half], normal: [0.0, 0.0, 1.0] },
        CubeVertex { position: [ half, -half,  half], normal: [0.0, 0.0, 1.0] },
        CubeVertex { position: [ half,  half,  half], normal: [0.0, 0.0, 1.0] },
        CubeVertex { position: [-half,  half,  half], normal: [0.0, 0.0, 1.0] },
        // Back face (-Z)
        CubeVertex { position: [ half, -half, -half], normal: [0.0, 0.0, -1.0] },
        CubeVertex { position: [-half, -half, -half], normal: [0.0, 0.0, -1.0] },
        CubeVertex { position: [-half,  half, -half], normal: [0.0, 0.0, -1.0] },
        CubeVertex { position: [ half,  half, -half], normal: [0.0, 0.0, -1.0] },
        // Top face (+Y)
        CubeVertex { position: [-half,  half,  half], normal: [0.0, 1.0, 0.0] },
        CubeVertex { position: [ half,  half,  half], normal: [0.0, 1.0, 0.0] },
        CubeVertex { position: [ half,  half, -half], normal: [0.0, 1.0, 0.0] },
        CubeVertex { position: [-half,  half, -half], normal: [0.0, 1.0, 0.0] },
        // Bottom face (-Y)
        CubeVertex { position: [-half, -half, -half], normal: [0.0, -1.0, 0.0] },
        CubeVertex { position: [ half, -half, -half], normal: [0.0, -1.0, 0.0] },
        CubeVertex { position: [ half, -half,  half], normal: [0.0, -1.0, 0.0] },
        CubeVertex { position: [-half, -half,  half], normal: [0.0, -1.0, 0.0] },
        // Right face (+X)
        CubeVertex { position: [ half, -half,  half], normal: [1.0, 0.0, 0.0] },
        CubeVertex { position: [ half, -half, -half], normal: [1.0, 0.0, 0.0] },
        CubeVertex { position: [ half,  half, -half], normal: [1.0, 0.0, 0.0] },
        CubeVertex { position: [ half,  half,  half], normal: [1.0, 0.0, 0.0] },
        // Left face (-X)
        CubeVertex { position: [-half, -half, -half], normal: [-1.0, 0.0, 0.0] },
        CubeVertex { position: [-half, -half,  half], normal: [-1.0, 0.0, 0.0] },
        CubeVertex { position: [-half,  half,  half], normal: [-1.0, 0.0, 0.0] },
        CubeVertex { position: [-half,  half, -half], normal: [-1.0, 0.0, 0.0] },
    ];

    let indices: [u32; 36] = [
        // Front
        0, 1, 2, 2, 3, 0,
        // Back
        4, 5, 6, 6, 7, 4,
        // Top
        8, 9, 10, 10, 11, 8,
        // Bottom
        12, 13, 14, 14, 15, 12,
        // Right
        16, 17, 18, 18, 19, 16,
        // Left
        20, 21, 22, 22, 23, 20,
    ];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Cube Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Cube Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    (vertex_buffer, index_buffer, indices.len() as u32)
}
