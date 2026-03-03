//! Common types and protocol definitions shared between client and server.
//!
//! This crate contains:
//! - Protocol version constants
//! - Entity/Account/Character ID types
//! - Math type re-exports (glam)
//! - Protocol message enums
//! - Envelope serialization helpers

pub mod ids;
pub mod math;
pub mod messages;
pub mod protocol;

pub use ids::{AccountId, CharacterId, EntityId};
pub use math::{Transform, Vec2, Vec3};
pub use messages::*;
pub use protocol::{decode_message, encode_message, Envelope, ProtocolError, PROTOCOL_VERSION};
