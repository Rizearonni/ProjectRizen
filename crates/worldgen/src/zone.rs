//! Zone parameters for terrain generation.
//!
//! These parameters define how a zone's terrain is generated.
//! Sent from server to client in `ZoneWelcome` message.

use serde::{Deserialize, Serialize};

/// Noise generation parameters for terrain heightmap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseParams {
    /// Base frequency of the noise (lower = larger features).
    pub base_freq: f64,
    /// Number of noise octaves (more = more detail).
    pub octaves: u32,
    /// Frequency multiplier between octaves.
    pub lacunarity: f64,
    /// Amplitude multiplier between octaves (persistence).
    pub gain: f64,
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            base_freq: 0.002,
            octaves: 5,
            lacunarity: 2.0,
            gain: 0.5,
        }
    }
}

/// Zone terrain generation parameters.
///
/// This struct contains all information needed to generate terrain
/// for a zone. Both server and client use identical parameters
/// to ensure deterministic agreement on terrain shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneParams {
    /// Zone seed for deterministic generation.
    pub seed: u64,
    /// Size of each chunk in world units (meters).
    pub chunk_size: f64,
    /// Number of vertices per side of chunk mesh (typically chunk_size + 1).
    pub verts_per_side: u32,
    /// Maximum terrain height.
    pub height_scale: f64,
    /// Noise parameters for heightmap generation.
    pub noise: NoiseParams,
}

impl Default for ZoneParams {
    fn default() -> Self {
        Self {
            seed: 0,
            chunk_size: 64.0,
            verts_per_side: 65,
            height_scale: 48.0,
            noise: NoiseParams::default(),
        }
    }
}

impl ZoneParams {
    /// Create zone params with a specific seed.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            seed,
            ..Default::default()
        }
    }

    /// Get the spacing between vertices in world units.
    pub fn vertex_spacing(&self) -> f64 {
        self.chunk_size / (self.verts_per_side - 1) as f64
    }
}

/// Biome classification parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeParams {
    /// Maximum normalized height for ash/low biome.
    pub ash_height_max: f64,
    /// Minimum slope for rocky terrain.
    pub rock_slope_min: f64,
}

impl Default for BiomeParams {
    fn default() -> Self {
        Self {
            ash_height_max: 0.35,
            rock_slope_min: 0.60,
        }
    }
}

/// Feature density parameters for procedural placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureParams {
    /// Density of bone features (0-1).
    pub bones_density: f64,
    /// Density of ruin features (0-1).
    pub ruins_density: f64,
    /// Density of lava crack features (0-1).
    pub lava_crack_density: f64,
}

impl Default for FeatureParams {
    fn default() -> Self {
        Self {
            bones_density: 0.08,
            ruins_density: 0.02,
            lava_crack_density: 0.01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_valid() {
        let params = ZoneParams::default();
        assert!(params.chunk_size > 0.0);
        assert!(params.verts_per_side > 1);
        assert!(params.height_scale > 0.0);
    }

    #[test]
    fn vertex_spacing_correct() {
        let params = ZoneParams {
            chunk_size: 64.0,
            verts_per_side: 65,
            ..Default::default()
        };
        let spacing = params.vertex_spacing();
        assert!((spacing - 1.0).abs() < 0.001, "Expected 1.0m spacing, got {}", spacing);
    }

    #[test]
    fn params_serialize_roundtrip() {
        let params = ZoneParams::with_seed(12345);
        let json = serde_json::to_string(&params).unwrap();
        let restored: ZoneParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params.seed, restored.seed);
        assert_eq!(params.chunk_size, restored.chunk_size);
    }
}
