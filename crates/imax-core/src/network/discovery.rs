use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePayload {
    pub public_key: [u8; 32],
    pub node_id: [u8; 32],
    pub addrs: Vec<SocketAddr>,
    pub relay_url: Option<String>,
    pub expires: u64,
}

pub struct InviteCode(pub String);

impl InviteCode {
    pub fn encode(payload: &InvitePayload) -> Result<Self> {
        let bytes = postcard::to_allocvec(payload)
            .map_err(|e| crate::Error::Network(format!("invite encode: {e}")))?;
        Ok(Self(format!("imax:{}", bs58::encode(&bytes).into_string())))
    }

    pub fn decode(code: &str) -> Result<InvitePayload> {
        let raw = code.strip_prefix("imax:")
            .ok_or_else(|| crate::Error::Network("invalid invite prefix".into()))?;
        let bytes = bs58::decode(raw).into_vec()
            .map_err(|e| crate::Error::Network(format!("base58 decode: {e}")))?;
        postcard::from_bytes(&bytes)
            .map_err(|e| crate::Error::Network(format!("invite decode: {e}")))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invite_roundtrip() {
        let payload = InvitePayload {
            public_key: [1u8; 32],
            node_id: [2u8; 32],
            addrs: vec!["127.0.0.1:4433".parse().unwrap()],
            relay_url: Some("https://relay.example.com".to_string()),
            expires: 9999999999,
        };
        let code = InviteCode::encode(&payload).unwrap();
        assert!(code.as_str().starts_with("imax:"));
        let decoded = InviteCode::decode(code.as_str()).unwrap();
        assert_eq!(decoded.public_key, payload.public_key);
        assert_eq!(decoded.node_id, payload.node_id);
    }

    #[test]
    fn test_invite_invalid_prefix() {
        let result = InviteCode::decode("notmax:abc123");
        assert!(result.is_err());
    }

    #[test]
    fn test_invite_no_addrs() {
        let payload = InvitePayload {
            public_key: [5u8; 32],
            node_id: [6u8; 32],
            addrs: vec![],
            relay_url: None,
            expires: 0,
        };
        let code = InviteCode::encode(&payload).unwrap();
        let decoded = InviteCode::decode(code.as_str()).unwrap();
        assert!(decoded.addrs.is_empty());
    }
}
