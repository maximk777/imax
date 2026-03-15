use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub public_key: [u8; 32],
    pub nickname: String,
}

impl UserProfile {
    pub fn new(verifying_key: &VerifyingKey, nickname: String) -> Self {
        Self {
            public_key: verifying_key.to_bytes(),
            nickname,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::keypair;

    #[test]
    fn test_user_profile_creation() {
        let mnemonic = keypair::generate_mnemonic().unwrap();
        let signing_key = keypair::derive_signing_key(&mnemonic);
        let profile = UserProfile::new(&signing_key.verifying_key(), "Alice".to_string());
        assert_eq!(profile.nickname, "Alice");
        assert_eq!(profile.public_key, signing_key.verifying_key().to_bytes());
    }
}
