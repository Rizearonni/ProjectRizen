//! Input handling for keyboard and mouse.

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::game_state::MoveInput;

/// Update move input state based on keyboard event.
pub fn handle_key_event(input: &mut MoveInput, event: &KeyEvent) {
    let pressed = event.state == ElementState::Pressed;

    if let PhysicalKey::Code(code) = event.physical_key {
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => input.forward = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => input.backward = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => input.left = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => input.right = pressed,
            // Simple yaw control with Q/E keys
            KeyCode::KeyQ => {
                if pressed {
                    input.yaw -= 0.1;
                }
            }
            KeyCode::KeyE => {
                if pressed {
                    input.yaw += 0.1;
                }
            }
            _ => {}
        }
    }
}

/// Check if input has any movement keys pressed.
pub fn has_movement(input: &MoveInput) -> bool {
    input.forward || input.backward || input.left || input.right
}
