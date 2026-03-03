//! Deterministic noise functions for terrain generation.
//!
//! All functions are pure and produce identical output for identical inputs,
//! ensuring server/client agreement on terrain height.

use noise::{NoiseFn, Perlin};

use crate::ZoneParams;

/// Compute terrain height at world coordinates.
///
/// This is the core determinism contract:
/// `height(x, z) = f(zone_seed, zone_params, world_x, world_z)`
///
/// Both server and client must use this exact function.
pub fn sample_height(params: &ZoneParams, world_x: f64, world_z: f64) -> f64 {
    let perlin = Perlin::new(params.seed as u32);
    
    let noise_params = &params.noise;
    let mut amplitude = 1.0;
    let mut frequency = noise_params.base_freq;
    let mut height = 0.0;
    let mut max_amplitude = 0.0;

    // Fractional Brownian Motion (fBm) - sum of octaves
    for _ in 0..noise_params.octaves {
        let sample_x = world_x * frequency;
        let sample_z = world_z * frequency;
        
        // Perlin returns [-1, 1], normalize to [0, 1]
        let noise_value = (perlin.get([sample_x, sample_z]) + 1.0) * 0.5;
        
        height += noise_value * amplitude;
        max_amplitude += amplitude;
        
        amplitude *= noise_params.gain;
        frequency *= noise_params.lacunarity;
    }

    // Normalize and scale
    let normalized = height / max_amplitude;
    normalized * params.height_scale
}

/// Compute terrain normal at world coordinates using central differences.
pub fn sample_normal(params: &ZoneParams, world_x: f64, world_z: f64) -> [f64; 3] {
    let delta = 0.5; // Sample distance for derivative
    
    let h_left = sample_height(params, world_x - delta, world_z);
    let h_right = sample_height(params, world_x + delta, world_z);
    let h_back = sample_height(params, world_x, world_z - delta);
    let h_front = sample_height(params, world_x, world_z + delta);
    
    // Gradient
    let dx = (h_right - h_left) / (2.0 * delta);
    let dz = (h_front - h_back) / (2.0 * delta);
    
    // Normal = normalize(cross(tangent_x, tangent_z))
    // tangent_x = (1, dx, 0), tangent_z = (0, dz, 1)
    // cross = (dx, 1, dz) (unnormalized, pointing up)
    // Actually: cross((1,dx,0), (0,dz,1)) = (dx*1 - 0*dz, 0*0 - 1*1, 1*dz - dx*0) = (dx, -1, dz)
    // We want upward normal, so negate: (-dx, 1, -dz)
    let nx = -dx;
    let ny = 1.0;
    let nz = -dz;
    
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    [nx / len, ny / len, nz / len]
}

/// Compute terrain slope (0.0 = flat, 1.0 = vertical).
pub fn sample_slope(params: &ZoneParams, world_x: f64, world_z: f64) -> f64 {
    let normal = sample_normal(params, world_x, world_z);
    // Slope = 1.0 - dot(normal, up)
    // up = (0, 1, 0)
    let dot_up = normal[1]; // ny
    1.0 - dot_up.max(0.0)
}

/// Hash function for deterministic coordinate-based randomness.
/// 
/// Used for feature placement to ensure repeatability.
pub fn hash_coords(seed: u64, x: i32, z: i32, salt: u32) -> u64 {
    // Simple mixing function
    let mut h = seed;
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= (x as u64).wrapping_mul(0x9e3779b97f4a7c15);
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= (z as u64).wrapping_mul(0x9e3779b97f4a7c15);
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= (salt as u64).wrapping_mul(0x9e3779b97f4a7c15);
    h = h.wrapping_mul(0x517cc1b727220a95);
    h
}

/// Convert hash to float in [0, 1).
pub fn hash_to_float(hash: u64) -> f64 {
    (hash >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoiseParams, ZoneParams};

    fn test_params() -> ZoneParams {
        ZoneParams {
            seed: 12345,
            chunk_size: 64.0,
            verts_per_side: 65,
            height_scale: 48.0,
            noise: NoiseParams {
                base_freq: 0.002,
                octaves: 5,
                lacunarity: 2.0,
                gain: 0.5,
            },
        }
    }

    #[test]
    fn height_is_deterministic() {
        let params = test_params();
        let h1 = sample_height(&params, 100.0, 200.0);
        let h2 = sample_height(&params, 100.0, 200.0);
        assert_eq!(h1, h2, "Same coords must produce same height");
    }

    #[test]
    fn different_coords_differ() {
        let params = test_params();
        let h1 = sample_height(&params, 0.0, 0.0);
        let h2 = sample_height(&params, 100.0, 100.0);
        assert!((h1 - h2).abs() > 0.001, "Different coords should differ");
    }

    #[test]
    fn height_in_range() {
        let params = test_params();
        for x in (-100..100).step_by(10) {
            for z in (-100..100).step_by(10) {
                let h = sample_height(&params, x as f64, z as f64);
                assert!(h >= 0.0 && h <= params.height_scale, "Height {} out of range", h);
            }
        }
    }

    #[test]
    fn normal_is_unit_length() {
        let params = test_params();
        let n = sample_normal(&params, 50.0, 50.0);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 0.001, "Normal should be unit length, got {}", len);
    }

    #[test]
    fn slope_in_range() {
        let params = test_params();
        let s = sample_slope(&params, 50.0, 50.0);
        assert!(s >= 0.0 && s <= 1.0, "Slope should be in [0,1], got {}", s);
    }

    #[test]
    fn hash_is_deterministic() {
        let h1 = hash_coords(12345, 10, 20, 1);
        let h2 = hash_coords(12345, 10, 20, 1);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_varies_with_salt() {
        let h1 = hash_coords(12345, 10, 20, 1);
        let h2 = hash_coords(12345, 10, 20, 2);
        assert_ne!(h1, h2);
    }
}
