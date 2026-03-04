//! 3D camera system.
//!
//! Simple FPS-style camera with position, yaw, pitch.
//! Produces view and projection matrices for rendering.

/// First-person camera.
#[derive(Debug, Clone)]
pub struct Camera {
    /// World position.
    pub position: [f32; 3],
    /// Yaw angle in radians (rotation around Y axis).
    pub yaw: f32,
    /// Pitch angle in radians (rotation around X axis).
    pub pitch: f32,
    /// Field of view in radians.
    pub fov: f32,
    /// Near clipping plane.
    pub near: f32,
    /// Far clipping plane.
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, 50.0, 0.0], // Start above terrain
            yaw: 0.0,
            pitch: -0.3, // Slight downward look
            fov: std::f32::consts::FRAC_PI_3, // 60 degrees
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    /// Get the forward direction vector (normalized).
    pub fn forward(&self) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        [
            sin_yaw * cos_pitch,
            sin_pitch,
            -cos_yaw * cos_pitch,
        ]
    }

    /// Get the right direction vector (normalized).
    pub fn right(&self) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        [cos_yaw, 0.0, sin_yaw]
    }

    /// Get the up direction vector (normalized).
    pub fn up(&self) -> [f32; 3] {
        let right = self.right();
        let forward = self.forward();
        cross(forward, right)
    }

    /// Build view matrix (world -> camera space).
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        let forward = self.forward();
        let right = self.right();
        let up = self.up();
        let pos = self.position;

        // Camera basis vectors form the rotation part
        // Translation is negated dot products
        [
            [right[0], up[0], -forward[0], 0.0],
            [right[1], up[1], -forward[1], 0.0],
            [right[2], up[2], -forward[2], 0.0],
            [
                -dot(right, pos),
                -dot(up, pos),
                dot(forward, pos),
                1.0,
            ],
        ]
    }

    /// Build projection matrix (camera -> clip space).
    pub fn projection_matrix(&self, aspect: f32) -> [[f32; 4]; 4] {
        let f = 1.0 / (self.fov / 2.0).tan();
        let nf = 1.0 / (self.near - self.far);

        [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, (self.far + self.near) * nf, -1.0],
            [0.0, 0.0, 2.0 * self.far * self.near * nf, 0.0],
        ]
    }

    /// Build combined view-projection matrix.
    pub fn view_projection_matrix(&self, aspect: f32) -> [[f32; 4]; 4] {
        let view = self.view_matrix();
        let proj = self.projection_matrix(aspect);
        mat4_mul(proj, view)
    }

    /// Move camera forward/backward.
    pub fn move_forward(&mut self, amount: f32) {
        let forward = self.forward();
        self.position[0] += forward[0] * amount;
        self.position[1] += forward[1] * amount;
        self.position[2] += forward[2] * amount;
    }

    /// Move camera left/right.
    pub fn move_right(&mut self, amount: f32) {
        let right = self.right();
        self.position[0] += right[0] * amount;
        self.position[2] += right[2] * amount;
    }

    /// Move camera up/down (world Y axis).
    pub fn move_up(&mut self, amount: f32) {
        self.position[1] += amount;
    }

    /// Rotate camera yaw (left/right).
    pub fn rotate_yaw(&mut self, amount: f32) {
        self.yaw += amount;
    }

    /// Rotate camera pitch (up/down), clamped to avoid gimbal issues.
    pub fn rotate_pitch(&mut self, amount: f32) {
        const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = (self.pitch + amount).clamp(-MAX_PITCH, MAX_PITCH);
    }
}

/// Camera input state for smooth movement.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub yaw_left: bool,
    pub yaw_right: bool,
    pub pitch_up: bool,
    pub pitch_down: bool,
}

impl CameraInput {
    /// Apply input to camera with given delta time.
    pub fn apply(&self, camera: &mut Camera, dt: f32) {
        const MOVE_SPEED: f32 = 30.0;
        const ROTATE_SPEED: f32 = 1.5;

        // Movement
        if self.forward {
            camera.move_forward(MOVE_SPEED * dt);
        }
        if self.backward {
            camera.move_forward(-MOVE_SPEED * dt);
        }
        if self.left {
            camera.move_right(-MOVE_SPEED * dt);
        }
        if self.right {
            camera.move_right(MOVE_SPEED * dt);
        }
        if self.up {
            camera.move_up(MOVE_SPEED * dt);
        }
        if self.down {
            camera.move_up(-MOVE_SPEED * dt);
        }

        // Rotation
        if self.yaw_left {
            camera.rotate_yaw(-ROTATE_SPEED * dt);
        }
        if self.yaw_right {
            camera.rotate_yaw(ROTATE_SPEED * dt);
        }
        if self.pitch_up {
            camera.rotate_pitch(ROTATE_SPEED * dt);
        }
        if self.pitch_down {
            camera.rotate_pitch(-ROTATE_SPEED * dt);
        }
    }
}

// Math helpers

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            result[i][j] = a[i][0] * b[0][j]
                + a[i][1] * b[1][j]
                + a[i][2] * b[2][j]
                + a[i][3] * b[3][j];
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_default() {
        let cam = Camera::default();
        let forward = cam.forward();
        // Default yaw=0, pitch=-0.3 should face roughly -Z with slight down
        assert!(forward[2] < 0.0, "Should face -Z");
    }

    #[test]
    fn test_view_projection() {
        let cam = Camera::default();
        let vp = cam.view_projection_matrix(16.0 / 9.0);
        // Just verify it produces a matrix without NaN
        for row in &vp {
            for val in row {
                assert!(val.is_finite(), "View-projection should be finite");
            }
        }
    }
}
