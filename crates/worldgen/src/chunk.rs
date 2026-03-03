//! Chunk generation and coordinate systems.
//!
//! Chunks are the fundamental unit of terrain streaming.
//! Each chunk has integer coordinates (cx, cz) and covers a
//! square region of `chunk_size` world units.

use serde::{Deserialize, Serialize};

use crate::noise::sample_height;
use crate::ZoneParams;

/// Chunk coordinate (integer grid position).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cz: i32,
}

impl ChunkCoord {
    pub fn new(cx: i32, cz: i32) -> Self {
        Self { cx, cz }
    }

    /// Get chunk coord from world position.
    pub fn from_world(world_x: f64, world_z: f64, chunk_size: f64) -> Self {
        Self {
            cx: (world_x / chunk_size).floor() as i32,
            cz: (world_z / chunk_size).floor() as i32,
        }
    }

    /// Get world-space minimum corner of this chunk.
    pub fn world_min(&self, chunk_size: f64) -> (f64, f64) {
        (self.cx as f64 * chunk_size, self.cz as f64 * chunk_size)
    }

    /// Get world-space maximum corner of this chunk.
    pub fn world_max(&self, chunk_size: f64) -> (f64, f64) {
        ((self.cx + 1) as f64 * chunk_size, (self.cz + 1) as f64 * chunk_size)
    }

    /// Get world-space center of this chunk.
    pub fn world_center(&self, chunk_size: f64) -> (f64, f64) {
        let (min_x, min_z) = self.world_min(chunk_size);
        (min_x + chunk_size * 0.5, min_z + chunk_size * 0.5)
    }

    /// Iterator over neighboring chunks (including self).
    pub fn neighbors_inclusive(&self) -> impl Iterator<Item = ChunkCoord> {
        let cx = self.cx;
        let cz = self.cz;
        (-1..=1).flat_map(move |dx| {
            (-1..=1).map(move |dz| ChunkCoord::new(cx + dx, cz + dz))
        })
    }

    /// Iterator over neighboring chunks (excluding self).
    pub fn neighbors(&self) -> impl Iterator<Item = ChunkCoord> {
        let cx = self.cx;
        let cz = self.cz;
        (-1..=1).flat_map(move |dx| {
            (-1..=1).filter_map(move |dz| {
                if dx == 0 && dz == 0 {
                    None
                } else {
                    Some(ChunkCoord::new(cx + dx, cz + dz))
                }
            })
        })
    }
}

/// Generated chunk heightmap data.
#[derive(Debug, Clone)]
pub struct ChunkHeightmap {
    /// Chunk coordinates.
    pub coord: ChunkCoord,
    /// Height values in row-major order (verts_per_side x verts_per_side).
    /// Index = z * verts_per_side + x
    pub heights: Vec<f32>,
    /// Number of vertices per side.
    pub verts_per_side: u32,
    /// Spacing between vertices in world units.
    pub vertex_spacing: f64,
}

impl ChunkHeightmap {
    /// Generate heightmap for a chunk.
    pub fn generate(params: &ZoneParams, coord: ChunkCoord) -> Self {
        let verts = params.verts_per_side;
        let spacing = params.vertex_spacing();
        let (min_x, min_z) = coord.world_min(params.chunk_size);
        
        let mut heights = Vec::with_capacity((verts * verts) as usize);
        
        for vz in 0..verts {
            for vx in 0..verts {
                let world_x = min_x + vx as f64 * spacing;
                let world_z = min_z + vz as f64 * spacing;
                let h = sample_height(params, world_x, world_z);
                heights.push(h as f32);
            }
        }
        
        Self {
            coord,
            heights,
            verts_per_side: verts,
            vertex_spacing: spacing,
        }
    }

    /// Get height at vertex coordinates (0-based).
    pub fn get(&self, vx: u32, vz: u32) -> f32 {
        let idx = (vz * self.verts_per_side + vx) as usize;
        self.heights.get(idx).copied().unwrap_or(0.0)
    }

    /// Get interpolated height at local chunk coordinates [0, chunk_size].
    pub fn sample_local(&self, local_x: f64, local_z: f64, _chunk_size: f64) -> f32 {
        // Convert to vertex coordinates
        let vx_f = local_x / self.vertex_spacing;
        let vz_f = local_z / self.vertex_spacing;
        
        // Clamp to valid range
        let max_v = (self.verts_per_side - 1) as f64;
        let vx_f = vx_f.clamp(0.0, max_v);
        let vz_f = vz_f.clamp(0.0, max_v);
        
        // Bilinear interpolation
        let vx0 = vx_f.floor() as u32;
        let vz0 = vz_f.floor() as u32;
        let vx1 = (vx0 + 1).min(self.verts_per_side - 1);
        let vz1 = (vz0 + 1).min(self.verts_per_side - 1);
        
        let fx = vx_f.fract() as f32;
        let fz = vz_f.fract() as f32;
        
        let h00 = self.get(vx0, vz0);
        let h10 = self.get(vx1, vz0);
        let h01 = self.get(vx0, vz1);
        let h11 = self.get(vx1, vz1);
        
        let h0 = h00 * (1.0 - fx) + h10 * fx;
        let h1 = h01 * (1.0 - fx) + h11 * fx;
        
        h0 * (1.0 - fz) + h1 * fz
    }

    /// Get minimum height in this chunk.
    pub fn min_height(&self) -> f32 {
        self.heights.iter().copied().fold(f32::INFINITY, f32::min)
    }

    /// Get maximum height in this chunk.
    pub fn max_height(&self) -> f32 {
        self.heights.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    }
}

/// Generate vertex positions for a chunk mesh.
///
/// Returns positions as flat array: [x0, y0, z0, x1, y1, z1, ...]
pub fn generate_chunk_vertices(heightmap: &ChunkHeightmap, chunk_size: f64) -> Vec<f32> {
    let verts = heightmap.verts_per_side;
    let (min_x, min_z) = heightmap.coord.world_min(chunk_size);
    let spacing = heightmap.vertex_spacing;
    
    let mut positions = Vec::with_capacity((verts * verts * 3) as usize);
    
    for vz in 0..verts {
        for vx in 0..verts {
            let x = min_x + vx as f64 * spacing;
            let y = heightmap.get(vx, vz) as f64;
            let z = min_z + vz as f64 * spacing;
            
            positions.push(x as f32);
            positions.push(y as f32);
            positions.push(z as f32);
        }
    }
    
    positions
}

/// Generate triangle indices for a chunk mesh.
///
/// Uses counter-clockwise winding for front faces (standard).
pub fn generate_chunk_indices(verts_per_side: u32) -> Vec<u32> {
    let quads = verts_per_side - 1;
    let mut indices = Vec::with_capacity((quads * quads * 6) as usize);
    
    for qz in 0..quads {
        for qx in 0..quads {
            let i00 = qz * verts_per_side + qx;
            let i10 = i00 + 1;
            let i01 = i00 + verts_per_side;
            let i11 = i01 + 1;
            
            // First triangle (lower-left)
            indices.push(i00);
            indices.push(i01);
            indices.push(i10);
            
            // Second triangle (upper-right)
            indices.push(i10);
            indices.push(i01);
            indices.push(i11);
        }
    }
    
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoiseParams;

    fn test_params() -> ZoneParams {
        ZoneParams {
            seed: 12345,
            chunk_size: 64.0,
            verts_per_side: 65,
            height_scale: 48.0,
            noise: NoiseParams::default(),
        }
    }

    #[test]
    fn chunk_coord_from_world() {
        let coord = ChunkCoord::from_world(100.0, 150.0, 64.0);
        assert_eq!(coord.cx, 1);
        assert_eq!(coord.cz, 2);
    }

    #[test]
    fn chunk_coord_from_world_negative() {
        let coord = ChunkCoord::from_world(-10.0, -200.0, 64.0);
        assert_eq!(coord.cx, -1);
        assert_eq!(coord.cz, -4);
    }

    #[test]
    fn chunk_world_bounds() {
        let coord = ChunkCoord::new(1, 2);
        let (min_x, min_z) = coord.world_min(64.0);
        let (max_x, max_z) = coord.world_max(64.0);
        
        assert_eq!(min_x, 64.0);
        assert_eq!(min_z, 128.0);
        assert_eq!(max_x, 128.0);
        assert_eq!(max_z, 192.0);
    }

    #[test]
    fn heightmap_generation() {
        let params = test_params();
        let coord = ChunkCoord::new(0, 0);
        let hm = ChunkHeightmap::generate(&params, coord);
        
        assert_eq!(hm.heights.len(), (65 * 65) as usize);
        assert!(hm.min_height() >= 0.0);
        assert!(hm.max_height() <= params.height_scale as f32);
    }

    #[test]
    fn heightmap_edges_match() {
        // Adjacent chunks should have matching edge heights (seamless)
        let params = test_params();
        
        let chunk_a = ChunkHeightmap::generate(&params, ChunkCoord::new(0, 0));
        let chunk_b = ChunkHeightmap::generate(&params, ChunkCoord::new(1, 0));
        
        // Right edge of A should match left edge of B
        let verts = params.verts_per_side;
        for vz in 0..verts {
            let h_a = chunk_a.get(verts - 1, vz);
            let h_b = chunk_b.get(0, vz);
            assert!(
                (h_a - h_b).abs() < 0.001,
                "Edge mismatch at vz={}: {} vs {}",
                vz, h_a, h_b
            );
        }
    }

    #[test]
    fn vertex_count_correct() {
        let params = test_params();
        let coord = ChunkCoord::new(0, 0);
        let hm = ChunkHeightmap::generate(&params, coord);
        let verts = generate_chunk_vertices(&hm, params.chunk_size);
        
        // 65x65 vertices, 3 floats each
        assert_eq!(verts.len(), 65 * 65 * 3);
    }

    #[test]
    fn index_count_correct() {
        let indices = generate_chunk_indices(65);
        // 64x64 quads, 2 triangles each, 3 indices per triangle
        assert_eq!(indices.len(), 64 * 64 * 6);
    }

    #[test]
    fn neighbors_count() {
        let coord = ChunkCoord::new(5, 5);
        assert_eq!(coord.neighbors_inclusive().count(), 9);
        assert_eq!(coord.neighbors().count(), 8);
    }
}
