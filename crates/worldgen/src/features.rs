//! Feature placement for procedural terrain decoration.
//!
//! Features (bones, rocks, ruins, etc.) are placed using deterministic
//! hashing of coordinates to ensure server/client agreement.

use serde::{Deserialize, Serialize};

use crate::chunk::ChunkCoord;
use crate::noise::{hash_coords, hash_to_float, sample_height};
use crate::ZoneParams;

/// Types of features that can be placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureKind {
    Bones,
    Rock,
    Pillar,
    Ruins,
    LavaCrack,
}

impl FeatureKind {
    /// Get salt value for hashing (ensures different distributions).
    fn salt(&self) -> u32 {
        match self {
            FeatureKind::Bones => 1,
            FeatureKind::Rock => 2,
            FeatureKind::Pillar => 3,
            FeatureKind::Ruins => 4,
            FeatureKind::LavaCrack => 5,
        }
    }
}

/// A placed feature instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// Type of feature.
    pub kind: FeatureKind,
    /// World position.
    pub position: [f32; 3],
    /// Y-axis rotation (radians).
    pub rotation: f32,
    /// Uniform scale factor.
    pub scale: f32,
}

/// Feature placement parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturePlacementParams {
    /// Grid cell size for feature placement (larger = sparser).
    pub cell_size: f64,
    /// Probability of placing a feature in each cell.
    pub density: f64,
    /// Minimum scale factor.
    pub scale_min: f32,
    /// Maximum scale factor.
    pub scale_max: f32,
}

impl Default for FeaturePlacementParams {
    fn default() -> Self {
        Self {
            cell_size: 8.0,
            density: 0.1,
            scale_min: 0.8,
            scale_max: 1.2,
        }
    }
}

/// Generate features for a chunk using deterministic placement.
///
/// Uses a grid-based approach with hashed coordinates to ensure
/// features are placed identically on server and client.
pub fn generate_chunk_features(
    zone_params: &ZoneParams,
    coord: ChunkCoord,
    kind: FeatureKind,
    placement: &FeaturePlacementParams,
) -> Vec<Feature> {
    let mut features = Vec::new();
    
    let (min_x, min_z) = coord.world_min(zone_params.chunk_size);
    let (max_x, max_z) = coord.world_max(zone_params.chunk_size);
    
    // Determine grid cells that overlap this chunk
    let cell_min_x = (min_x / placement.cell_size).floor() as i32;
    let cell_min_z = (min_z / placement.cell_size).floor() as i32;
    let cell_max_x = (max_x / placement.cell_size).ceil() as i32;
    let cell_max_z = (max_z / placement.cell_size).ceil() as i32;
    
    for cell_z in cell_min_z..cell_max_z {
        for cell_x in cell_min_x..cell_max_x {
            // Hash to determine if this cell has a feature
            let presence_hash = hash_coords(zone_params.seed, cell_x, cell_z, kind.salt());
            let presence = hash_to_float(presence_hash);
            
            if presence >= placement.density {
                continue;
            }
            
            // Hash for position within cell
            let pos_hash_x = hash_coords(zone_params.seed, cell_x, cell_z, kind.salt() + 100);
            let pos_hash_z = hash_coords(zone_params.seed, cell_x, cell_z, kind.salt() + 200);
            
            let local_x = hash_to_float(pos_hash_x);
            let local_z = hash_to_float(pos_hash_z);
            
            let world_x = (cell_x as f64 + local_x) * placement.cell_size;
            let world_z = (cell_z as f64 + local_z) * placement.cell_size;
            
            // Check if position is actually within this chunk
            if world_x < min_x || world_x >= max_x || world_z < min_z || world_z >= max_z {
                continue;
            }
            
            // Get terrain height at position
            let world_y = sample_height(zone_params, world_x, world_z);
            
            // Hash for rotation and scale
            let rot_hash = hash_coords(zone_params.seed, cell_x, cell_z, kind.salt() + 300);
            let scale_hash = hash_coords(zone_params.seed, cell_x, cell_z, kind.salt() + 400);
            
            let rotation = (hash_to_float(rot_hash) * std::f64::consts::TAU) as f32;
            let scale_t = hash_to_float(scale_hash) as f32;
            let scale = placement.scale_min + scale_t * (placement.scale_max - placement.scale_min);
            
            features.push(Feature {
                kind,
                position: [world_x as f32, world_y as f32, world_z as f32],
                rotation,
                scale,
            });
        }
    }
    
    features
}

/// Generate all features for a chunk with default placement parameters.
pub fn generate_all_chunk_features(
    zone_params: &ZoneParams,
    coord: ChunkCoord,
) -> Vec<Feature> {
    let mut features = Vec::new();
    
    // Bones - common decoration
    features.extend(generate_chunk_features(
        zone_params,
        coord,
        FeatureKind::Bones,
        &FeaturePlacementParams {
            cell_size: 6.0,
            density: 0.08,
            scale_min: 0.5,
            scale_max: 1.5,
        },
    ));
    
    // Rocks - scattered
    features.extend(generate_chunk_features(
        zone_params,
        coord,
        FeatureKind::Rock,
        &FeaturePlacementParams {
            cell_size: 8.0,
            density: 0.05,
            scale_min: 0.8,
            scale_max: 2.0,
        },
    ));
    
    // Pillars - rare
    features.extend(generate_chunk_features(
        zone_params,
        coord,
        FeatureKind::Pillar,
        &FeaturePlacementParams {
            cell_size: 16.0,
            density: 0.02,
            scale_min: 0.9,
            scale_max: 1.3,
        },
    ));
    
    // Ruins - very rare
    features.extend(generate_chunk_features(
        zone_params,
        coord,
        FeatureKind::Ruins,
        &FeaturePlacementParams {
            cell_size: 32.0,
            density: 0.02,
            scale_min: 1.0,
            scale_max: 1.2,
        },
    ));
    
    features
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
    fn feature_generation_deterministic() {
        let params = test_params();
        let coord = ChunkCoord::new(0, 0);
        let placement = FeaturePlacementParams::default();
        
        let features1 = generate_chunk_features(&params, coord, FeatureKind::Bones, &placement);
        let features2 = generate_chunk_features(&params, coord, FeatureKind::Bones, &placement);
        
        assert_eq!(features1.len(), features2.len());
        for (f1, f2) in features1.iter().zip(features2.iter()) {
            assert_eq!(f1.position, f2.position);
            assert_eq!(f1.rotation, f2.rotation);
            assert_eq!(f1.scale, f2.scale);
        }
    }

    #[test]
    fn features_within_chunk_bounds() {
        let params = test_params();
        let coord = ChunkCoord::new(1, 2);
        let (min_x, min_z) = coord.world_min(params.chunk_size);
        let (max_x, max_z) = coord.world_max(params.chunk_size);
        
        let features = generate_all_chunk_features(&params, coord);
        
        for feature in &features {
            let [x, _, z] = feature.position;
            assert!(
                x >= min_x as f32 && x < max_x as f32,
                "Feature x={} outside chunk bounds [{}, {})",
                x, min_x, max_x
            );
            assert!(
                z >= min_z as f32 && z < max_z as f32,
                "Feature z={} outside chunk bounds [{}, {})",
                z, min_z, max_z
            );
        }
    }

    #[test]
    fn different_kinds_have_different_placements() {
        let params = test_params();
        let coord = ChunkCoord::new(0, 0);
        let placement = FeaturePlacementParams {
            density: 1.0, // Always place for comparison
            ..Default::default()
        };
        
        let bones = generate_chunk_features(&params, coord, FeatureKind::Bones, &placement);
        let rocks = generate_chunk_features(&params, coord, FeatureKind::Rock, &placement);
        
        // Should have different positions due to different salts
        if !bones.is_empty() && !rocks.is_empty() {
            assert_ne!(
                bones[0].position, rocks[0].position,
                "Different feature kinds should have different positions"
            );
        }
    }

    #[test]
    fn feature_scale_in_range() {
        let params = test_params();
        let coord = ChunkCoord::new(0, 0);
        let placement = FeaturePlacementParams {
            scale_min: 0.5,
            scale_max: 2.0,
            density: 0.5,
            ..Default::default()
        };
        
        let features = generate_chunk_features(&params, coord, FeatureKind::Bones, &placement);
        
        for feature in &features {
            assert!(
                feature.scale >= 0.5 && feature.scale <= 2.0,
                "Scale {} out of range",
                feature.scale
            );
        }
    }
}
