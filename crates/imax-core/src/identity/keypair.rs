use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use bip39::Mnemonic;
use hkdf::Hkdf;
use sha2::Sha256;
use crate::Result;

/// Generate a new BIP39 24-word mnemonic
pub fn generate_mnemonic() -> Result<Mnemonic> {
    let mnemonic = Mnemonic::generate_in(bip39::Language::English, 24)
        .map_err(|e| crate::Error::Identity(e.to_string()))?;
    Ok(mnemonic)
}

/// Parse a mnemonic from a string of space-separated words
pub fn parse_mnemonic(phrase: &str) -> Result<Mnemonic> {
    let mnemonic = Mnemonic::parse_in(bip39::Language::English, phrase)
        .map_err(|e| crate::Error::Identity(e.to_string()))?;
    Ok(mnemonic)
}

/// Derive Ed25519 signing key from mnemonic via HKDF
pub fn derive_signing_key(mnemonic: &Mnemonic) -> SigningKey {
    let entropy = mnemonic.to_entropy();
    let hk = Hkdf::<Sha256>::new(None, &entropy);
    let mut okm = [0u8; 32];
    hk.expand(b"imax-identity", &mut okm)
        .expect("32 bytes is valid for HKDF-SHA256");
    SigningKey::from_bytes(&okm)
}

/// Convert Ed25519 signing key to X25519 secret (for DH).
/// Uses SHA-512 hash of secret key bytes, clamp first 32 bytes.
pub fn to_x25519_secret(signing_key: &SigningKey) -> X25519Secret {
    use sha2::{Sha512, Digest};
    let mut hasher = Sha512::new();
    hasher.update(signing_key.to_bytes());
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash[..32]);
    // Clamp per RFC 7748
    key[0] &= 248;
    key[31] &= 127;
    key[31] |= 64;
    X25519Secret::from(key)
}

/// Get X25519 public key from Ed25519 verifying key.
/// Decompresses Edwards point and converts to Montgomery form.
pub fn to_x25519_public(verifying_key: &VerifyingKey) -> X25519PublicKey {
    use curve25519_dalek::edwards::CompressedEdwardsY;
    let compressed = CompressedEdwardsY::from_slice(verifying_key.as_bytes())
        .expect("valid 32-byte Edwards point");
    let edwards = compressed.decompress().expect("valid point on curve");
    let montgomery = edwards.to_montgomery();
    X25519PublicKey::from(montgomery.to_bytes())
}

/// Convert raw Ed25519 public key bytes to X25519 public key for DH.
pub fn x25519_public_from_bytes(ed25519_pubkey: &[u8; 32]) -> Result<X25519PublicKey> {
    use curve25519_dalek::edwards::CompressedEdwardsY;
    let compressed = CompressedEdwardsY::from_slice(ed25519_pubkey)
        .map_err(|e| crate::Error::Identity(format!("invalid Edwards point: {e}")))?;
    let edwards = compressed.decompress()
        .ok_or_else(|| crate::Error::Identity("failed to decompress Edwards point".into()))?;
    let montgomery = edwards.to_montgomery();
    Ok(X25519PublicKey::from(montgomery.to_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic_produces_24_words() {
        let mnemonic = generate_mnemonic().unwrap();
        let words: Vec<&str> = mnemonic.words().collect();
        assert_eq!(words.len(), 24);
    }

    #[test]
    fn test_parse_mnemonic_valid() {
        let mnemonic = generate_mnemonic().unwrap();
        let phrase = mnemonic.to_string();
        let parsed = parse_mnemonic(&phrase).unwrap();
        assert_eq!(mnemonic.to_string(), parsed.to_string());
    }

    #[test]
    fn test_parse_mnemonic_invalid() {
        let result = parse_mnemonic("not a valid mnemonic phrase");
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_signing_key_deterministic() {
        let mnemonic = generate_mnemonic().unwrap();
        let key1 = derive_signing_key(&mnemonic);
        let key2 = derive_signing_key(&mnemonic);
        assert_eq!(key1.to_bytes(), key2.to_bytes());
    }

    #[test]
    fn test_derive_signing_key_different_mnemonics() {
        let m1 = generate_mnemonic().unwrap();
        let m2 = generate_mnemonic().unwrap();
        let k1 = derive_signing_key(&m1);
        let k2 = derive_signing_key(&m2);
        assert_ne!(k1.to_bytes(), k2.to_bytes());
    }

    #[test]
    fn test_x25519_key_exchange() {
        let m_alice = generate_mnemonic().unwrap();
        let m_bob = generate_mnemonic().unwrap();

        let sk_alice = derive_signing_key(&m_alice);
        let sk_bob = derive_signing_key(&m_bob);

        let x_secret_alice = to_x25519_secret(&sk_alice);
        let x_secret_bob = to_x25519_secret(&sk_bob);

        let x_pub_alice = to_x25519_public(&sk_alice.verifying_key());
        let x_pub_bob = to_x25519_public(&sk_bob.verifying_key());

        let shared_alice = x_secret_alice.diffie_hellman(&x_pub_bob);
        let shared_bob = x_secret_bob.diffie_hellman(&x_pub_alice);

        assert_eq!(shared_alice.as_bytes(), shared_bob.as_bytes());
    }
}
