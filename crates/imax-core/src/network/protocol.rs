use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WireMessage {
    Hello {
        public_key: [u8; 32],
        nickname: String,
        protocol_version: u8,
    },
    ChatMessage {
        id: Uuid,
        ciphertext: Vec<u8>,
        nonce: [u8; 24],
        timestamp: u64,
    },
    Ack {
        message_id: Uuid,
        status: AckStatus,
    },
    SyncRequest {
        last_seq: u64,
    },
    SyncResponse {
        messages: Vec<WireChatMessage>,
        has_more: bool,
    },
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireChatMessage {
    pub id: Uuid,
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 24],
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AckStatus {
    Delivered,
    Read,
}

/// Serialize a WireMessage to length-prefixed bytes (4-byte BE length + postcard payload)
pub fn encode(msg: &WireMessage) -> Result<Vec<u8>> {
    let payload = postcard::to_allocvec(msg)
        .map_err(|e| crate::Error::Network(format!("encode error: {e}")))?;
    let len = (payload.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Deserialize a WireMessage from raw bytes (without length prefix)
pub fn decode(data: &[u8]) -> Result<WireMessage> {
    postcard::from_bytes(data)
        .map_err(|e| crate::Error::Network(format!("decode error: {e}")))
}

/// Read a length-prefixed frame from a buffer. Returns (message, bytes_consumed) or None if incomplete.
pub fn decode_frame(buf: &[u8]) -> Result<Option<(WireMessage, usize)>> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let msg = decode(&buf[4..4 + len])?;
    Ok(Some((msg, 4 + len)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_hello() {
        let msg = WireMessage::Hello {
            public_key: [42u8; 32],
            nickname: "Alice".to_string(),
            protocol_version: 1,
        };
        let encoded = encode(&msg).unwrap();
        let (decoded, consumed) = decode_frame(&encoded).unwrap().unwrap();
        assert_eq!(decoded, msg);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_encode_decode_chat_message() {
        let msg = WireMessage::ChatMessage {
            id: Uuid::new_v4(),
            ciphertext: vec![1, 2, 3, 4, 5],
            nonce: [7u8; 24],
            timestamp: 1234567890,
        };
        let encoded = encode(&msg).unwrap();
        let (decoded, _) = decode_frame(&encoded).unwrap().unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_encode_decode_ping_pong() {
        for msg in [WireMessage::Ping, WireMessage::Pong] {
            let encoded = encode(&msg).unwrap();
            let (decoded, _) = decode_frame(&encoded).unwrap().unwrap();
            assert_eq!(decoded, msg);
        }
    }

    #[test]
    fn test_decode_frame_incomplete() {
        let msg = WireMessage::Ping;
        let encoded = encode(&msg).unwrap();
        let partial = &encoded[..encoded.len() / 2];
        let result = decode_frame(partial).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_frame_multiple_messages() {
        let msg1 = WireMessage::Ping;
        let msg2 = WireMessage::Pong;
        let mut buf = encode(&msg1).unwrap();
        buf.extend_from_slice(&encode(&msg2).unwrap());
        let (decoded1, consumed1) = decode_frame(&buf).unwrap().unwrap();
        assert_eq!(decoded1, msg1);
        let (decoded2, _) = decode_frame(&buf[consumed1..]).unwrap().unwrap();
        assert_eq!(decoded2, msg2);
    }
}
