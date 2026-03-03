//! Protocol envelope and serialization helpers.
//!
//! All messages are wrapped in an Envelope for wire transmission.
//! Format: { protocol_version, msg_type, request_id, payload }

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::messages::Message;

/// Protocol version constant. Increment when breaking changes are made.
pub const PROTOCOL_VERSION: u16 = 1;

/// Errors that can occur during protocol encoding/decoding.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Protocol version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u16, got: u16 },

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Unknown message type: {0}")]
    UnknownMessageType(u16),
}

/// Wire envelope for all protocol messages.
///
/// The envelope provides versioning, message type identification,
/// and request correlation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    /// Protocol version for compatibility checking.
    pub protocol_version: u16,
    /// Message type discriminant (see msg_type module).
    pub msg_type: u16,
    /// Optional request ID for request/response correlation.
    pub request_id: Option<u32>,
    /// Serialized message payload (bincode).
    pub payload: Vec<u8>,
}

impl Envelope {
    /// Create a new envelope wrapping the given message.
    pub fn new(message: &Message, request_id: Option<u32>) -> Result<Self, ProtocolError> {
        let payload = bincode::serialize(message)?;
        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            msg_type: message.msg_type(),
            request_id,
            payload,
        })
    }

    /// Encode the envelope to bytes for transmission.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        Ok(bincode::serialize(self)?)
    }

    /// Decode bytes into an envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(bincode::deserialize(bytes)?)
    }

    /// Extract the message from this envelope.
    pub fn into_message(self) -> Result<Message, ProtocolError> {
        Ok(bincode::deserialize(&self.payload)?)
    }

    /// Check if the protocol version matches current.
    pub fn check_version(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: self.protocol_version,
            });
        }
        Ok(())
    }
}

/// Helper to encode a message directly to bytes.
pub fn encode_message(message: impl Into<Message>, request_id: Option<u32>) -> Result<Vec<u8>, ProtocolError> {
    let msg = message.into();
    Envelope::new(&msg, request_id)?.encode()
}

/// Helper to decode bytes directly to a message.
pub fn decode_message(bytes: &[u8]) -> Result<(Message, Option<u32>), ProtocolError> {
    let envelope = Envelope::decode(bytes)?;
    envelope.check_version()?;
    let request_id = envelope.request_id;
    let message = envelope.into_message()?;
    Ok((message, request_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::*;
    use crate::ids::CharacterId;

    #[test]
    fn envelope_roundtrip() {
        let msg = Message::ZoneHello(ZoneHello {
            character_id: CharacterId::new(),
            protocol_version: PROTOCOL_VERSION,
        });
        
        let env = Envelope::new(&msg, Some(42)).unwrap();
        let bytes = env.encode().unwrap();
        let decoded = Envelope::decode(&bytes).unwrap();
        
        assert_eq!(decoded.protocol_version, PROTOCOL_VERSION);
        assert_eq!(decoded.request_id, Some(42));
        
        let restored = decoded.into_message().unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn zone_hello_roundtrip() {
        let original = ZoneHello {
            character_id: CharacterId::new(),
            protocol_version: PROTOCOL_VERSION,
        };
        
        let bytes = encode_message(original.clone(), None).unwrap();
        let (decoded, req_id) = decode_message(&bytes).unwrap();
        
        assert!(req_id.is_none());
        match decoded {
            Message::ZoneHello(zh) => {
                assert_eq!(zh.character_id, original.character_id);
                assert_eq!(zh.protocol_version, original.protocol_version);
            }
            _ => panic!("Wrong message type decoded"),
        }
    }

    #[test]
    fn input_move_roundtrip() {
        let original = InputMove {
            move_x: 0.5,
            move_y: -0.3,
            yaw: 1.57,
            client_tick: 100,
        };
        
        let bytes = encode_message(original.clone(), Some(99)).unwrap();
        let (decoded, req_id) = decode_message(&bytes).unwrap();
        
        assert_eq!(req_id, Some(99));
        match decoded {
            Message::InputMove(im) => {
                assert!((im.move_x - original.move_x).abs() < f32::EPSILON);
                assert!((im.move_y - original.move_y).abs() < f32::EPSILON);
                assert!((im.yaw - original.yaw).abs() < f32::EPSILON);
                assert_eq!(im.client_tick, original.client_tick);
            }
            _ => panic!("Wrong message type decoded"),
        }
    }

    #[test]
    fn world_snapshot_roundtrip() {
        use crate::ids::EntityId;
        use crate::math::Transform;
        use glam::Vec3;

        let original = WorldSnapshot {
            server_tick: 12345,
            entities: vec![
                EntitySnapshot {
                    id: EntityId::new(1),
                    transform: Transform::new(Vec3::new(10.0, 0.0, 20.0), 0.5),
                },
                EntitySnapshot {
                    id: EntityId::new(2),
                    transform: Transform::new(Vec3::new(-5.0, 1.0, 15.0), 3.14),
                },
            ],
        };
        
        let bytes = encode_message(original.clone(), None).unwrap();
        let (decoded, _) = decode_message(&bytes).unwrap();
        
        match decoded {
            Message::WorldSnapshot(ws) => {
                assert_eq!(ws.server_tick, original.server_tick);
                assert_eq!(ws.entities.len(), 2);
                assert_eq!(ws.entities[0].id, EntityId::new(1));
            }
            _ => panic!("Wrong message type decoded"),
        }
    }
}
