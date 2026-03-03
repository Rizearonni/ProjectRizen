//! Math type re-exports from glam.
//!
//! We re-export glam types to provide a stable interface
//! and allow future customization if needed.

pub use glam::Vec2;
pub use glam::Vec3;

/// Transform component for positioned entities.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Transform {
    pub pos: Vec3,
    pub yaw: f32,
}

impl Transform {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Self { pos, yaw }
    }

    pub fn at_origin() -> Self {
        Self {
            pos: Vec3::ZERO,
            yaw: 0.0,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::at_origin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_default_at_origin() {
        let t = Transform::default();
        assert_eq!(t.pos, Vec3::ZERO);
        assert_eq!(t.yaw, 0.0);
    }
}
