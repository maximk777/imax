use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::RngCore;
use crate::Result;

/// Derive a symmetric encryption key from X25519 shared secret.
/// Sorts pubkeys so both sides derive the same key regardless of order.
pub fn derive_symmetric_key(
    shared_secret: &[u8; 32],
    our_pubkey: &[u8; 32],
    their_pubkey: &[u8; 32],
) -> [u8; 32] {
    let salt = if our_pubkey < their_pubkey {
        [our_pubkey.as_slice(), their_pubkey.as_slice()].concat()
    } else {
        [their_pubkey.as_slice(), our_pubkey.as_slice()].concat()
    };
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared_secret);
    let mut okm = [0u8; 32];
    hk.expand(b"imax-e2e-v1", &mut okm)
        .expect("32 bytes is valid for HKDF-SHA256");
    okm
}

/// Encrypt plaintext with XChaCha20-Poly1305. Returns (ciphertext, nonce).
pub fn encrypt(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, [u8; 24])> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let payload = chacha20poly1305::aead::Payload { msg: plaintext, aad };
    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| crate::Error::Crypto(format!("encryption failed: {e}")))?;
    Ok((ciphertext, nonce_bytes))
}

/// Decrypt ciphertext with XChaCha20-Poly1305.
pub fn decrypt(
    key: &[u8; 32],
    ciphertext: &[u8],
    nonce: &[u8; 24],
    aad: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce);
    let payload = chacha20poly1305::aead::Payload { msg: ciphertext, aad };
    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|e| crate::Error::Crypto(format!("decryption failed: {e}")))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::keypair;

    #[test]
    fn test_derive_symmetric_key_deterministic() {
        let secret = [42u8; 32];
        let pk_a = [1u8; 32];
        let pk_b = [2u8; 32];
        let k1 = derive_symmetric_key(&secret, &pk_a, &pk_b);
        let k2 = derive_symmetric_key(&secret, &pk_a, &pk_b);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_symmetric_key_order_independent() {
        let secret = [42u8; 32];
        let pk_a = [1u8; 32];
        let pk_b = [2u8; 32];
        let k1 = derive_symmetric_key(&secret, &pk_a, &pk_b);
        let k2 = derive_symmetric_key(&secret, &pk_b, &pk_a);
        assert_eq!(k1, k2, "Key must be the same regardless of pubkey order");
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let plaintext = b"Hello, iMax!";
        let aad = b"message-id-123";
        let (ciphertext, nonce) = encrypt(&key, plaintext, aad).unwrap();
        let decrypted = decrypt(&key, &ciphertext, &nonce, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key = [42u8; 32];
        let wrong_key = [99u8; 32];
        let (ciphertext, nonce) = encrypt(&key, b"secret", b"aad").unwrap();
        let result = decrypt(&wrong_key, &ciphertext, &nonce, b"aad");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_wrong_aad_fails() {
        let key = [42u8; 32];
        let (ciphertext, nonce) = encrypt(&key, b"secret", b"correct-aad").unwrap();
        let result = decrypt(&key, &ciphertext, &nonce, b"wrong-aad");
        assert!(result.is_err());
    }

    #[test]
    fn test_full_e2e_flow_with_keypairs() {
        let m_alice = keypair::generate_mnemonic().unwrap();
        let m_bob = keypair::generate_mnemonic().unwrap();
        let sk_alice = keypair::derive_signing_key(&m_alice);
        let sk_bob = keypair::derive_signing_key(&m_bob);
        let x_secret_alice = keypair::to_x25519_secret(&sk_alice);
        let x_secret_bob = keypair::to_x25519_secret(&sk_bob);
        let x_pub_alice = keypair::to_x25519_public(&sk_alice.verifying_key());
        let x_pub_bob = keypair::to_x25519_public(&sk_bob.verifying_key());
        let shared_alice = x_secret_alice.diffie_hellman(&x_pub_bob);
        let shared_bob = x_secret_bob.diffie_hellman(&x_pub_alice);
        let pk_a = sk_alice.verifying_key().to_bytes();
        let pk_b = sk_bob.verifying_key().to_bytes();
        let key_alice = derive_symmetric_key(shared_alice.as_bytes(), &pk_a, &pk_b);
        let key_bob = derive_symmetric_key(shared_bob.as_bytes(), &pk_b, &pk_a);
        assert_eq!(key_alice, key_bob);
        let msg_id = b"msg-uuid-001";
        let (ct, nonce) = encrypt(&key_alice, b"Hello Bob!", msg_id).unwrap();
        let pt = decrypt(&key_bob, &ct, &nonce, msg_id).unwrap();
        assert_eq!(pt, b"Hello Bob!");
    }
}
