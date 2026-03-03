//! Worldgen library for deterministic procedural terrain generation.
//!
//! This crate provides pure functions for generating terrain heightmaps,
//! normals, and features. Both server and client use identical code to
//! ensure agreement on terrain shape.
//!
//! # Core Contract
//!
//! All generation is deterministic:
//! ```text
//! height(x, z) = f(zone_seed, zone_params, world_x, world_z)
//! ```
//!
//! # Modules
//!
//! - [`zone`]: Zone parameters (`ZoneParams`, `NoiseParams`)
//! - [`noise`]: Height sampling and coordinate hashing
//! - [`chunk`]: Chunk coordinates and heightmap generation
//! - [`features`]: Deterministic feature placement
//!
//! # Example
//!
//! ```
//! use worldgen::{ZoneParams, ChunkCoord, ChunkHeightmap, sample_height};
//!
//! // Create zone with specific seed
//! let params = ZoneParams::with_seed(12345);
//!
//! // Sample height at world coordinate
//! let h = sample_height(&params, 100.0, 200.0);
//!
//! // Generate chunk heightmap
//! let coord = ChunkCoord::new(0, 0);
//! let heightmap = ChunkHeightmap::generate(&params, coord);
//! ```

pub mod chunk;
pub mod features;
pub mod noise;
pub mod zone;

// Re-export main types
pub use chunk::{
    generate_chunk_indices, generate_chunk_vertices, ChunkCoord, ChunkHeightmap,
};
pub use features::{
    generate_all_chunk_features, generate_chunk_features, Feature, FeatureKind,
    FeaturePlacementParams,
};
pub use noise::{hash_coords, hash_to_float, sample_height, sample_normal, sample_slope};
pub use zone::{BiomeParams, FeatureParams, NoiseParams, ZoneParams};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_height_and_chunk_agree() {
        let params = ZoneParams::with_seed(999);
        let coord = ChunkCoord::new(0, 0);
        let heightmap = ChunkHeightmap::generate(&params, coord);

        // Sample a point directly and via chunk
        let world_x = 32.0;
        let world_z = 32.0;
        
        let direct_h = sample_height(&params, world_x, world_z) as f32;
        
        // Get via bilinear sampling from chunk
        let (min_x, min_z) = coord.world_min(params.chunk_size);
        let local_x = world_x - min_x;
        let local_z = world_z - min_z;
        let chunk_h = heightmap.sample_local(local_x, local_z, params.chunk_size);
        
        // Should be very close (bilinear interpolation may introduce tiny error)
        assert!(
            (direct_h - chunk_h).abs() < 0.01,
            "Direct sample {} vs chunk sample {} differ too much",
            direct_h, chunk_h
        );
    }

    #[test]
    fn integration_adjacent_chunks_seamless() {
        let params = ZoneParams::with_seed(42);
        
        // Generate a 2x2 grid of chunks
        let chunks: Vec<ChunkHeightmap> = vec![
            ChunkHeightmap::generate(&params, ChunkCoord::new(0, 0)),
            ChunkHeightmap::generate(&params, ChunkCoord::new(1, 0)),
            ChunkHeightmap::generate(&params, ChunkCoord::new(0, 1)),
            ChunkHeightmap::generate(&params, ChunkCoord::new(1, 1)),
        ];
        
        // Check all edges match
        let verts = params.verts_per_side;
        
        // Chunk 0 right edge == Chunk 1 left edge
        for vz in 0..verts {
            let h0 = chunks[0].get(verts - 1, vz);
            let h1 = chunks[1].get(0, vz);
            assert!((h0 - h1).abs() < 0.001, "Horizontal seam mismatch");
        }
        
        // Chunk 0 top edge == Chunk 2 bottom edge
        for vx in 0..verts {
            let h0 = chunks[0].get(vx, verts - 1);
            let h2 = chunks[2].get(vx, 0);
            assert!((h0 - h2).abs() < 0.001, "Vertical seam mismatch");
        }
    }

    #[test]
    fn integration_features_on_terrain() {
        let params = ZoneParams::with_seed(777);
        let coord = ChunkCoord::new(0, 0);
        
        let features = generate_all_chunk_features(&params, coord);
        
        // Each feature's Y should match terrain height at that XZ
        for feature in &features {
            let expected_y = sample_height(&params, feature.position[0] as f64, feature.position[2] as f64) as f32;
            assert!(
                (feature.position[1] - expected_y).abs() < 0.01,
                "Feature Y {} doesn't match terrain {}",
                feature.position[1], expected_y
            );
        }
    }
}
