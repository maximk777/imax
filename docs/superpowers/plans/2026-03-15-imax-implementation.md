# iMax P2P Messenger Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working P2P encrypted desktop messenger (iMax) with 1-to-1 chat, E2E encryption, invite codes, and Telegram-style UI.

**Architecture:** Cargo workspace with `imax-core` library crate (identity, crypto, storage, network, chat modules) and a Dioxus desktop binary. Core modules are built bottom-up: identity → crypto → storage → network → chat manager. UI connects to core via ChatManager API + broadcast events.

**Tech Stack:** Rust 2024, iroh (P2P/QUIC), Dioxus 0.7 (desktop UI), SQLite (rusqlite), XChaCha20-Poly1305 + X25519 (E2E), BIP39 (seed phrase), postcard (serialization)

**Spec:** `docs/superpowers/specs/2026-03-15-dchat-p2p-messenger-design.md`

---

## File Map

### imax-core (library crate: `crates/imax-core/`)

| File | Responsibility |
|------|---------------|
| `src/lib.rs` | Re-exports all public types and modules |
| `src/error.rs` | Unified `Error` enum with per-module variants |
| `src/identity/mod.rs` | Re-exports identity module |
| `src/identity/keypair.rs` | BIP39 seed → HKDF → Ed25519 → X25519 derivation |
| `src/identity/profile.rs` | UserProfile struct (pubkey, nickname) |
| `src/crypto/mod.rs` | Re-exports crypto module |
| `src/crypto/e2e.rs` | Shared secret derivation, encrypt/decrypt messages |
| `src/storage/mod.rs` | Re-exports storage module |
| `src/storage/db.rs` | Database struct, connection, migrations |
| `src/storage/models.rs` | CRUD for identity, contacts, chats, messages |
| `src/network/mod.rs` | Re-exports network module |
| `src/network/protocol.rs` | WireMessage enum, serialization, framing |
| `src/network/node.rs` | IrohNode: endpoint, accept/connect, send/receive |
| `src/network/discovery.rs` | InviteCode: generate, parse, encode/decode |
| `src/chat/mod.rs` | Re-exports chat module |
| `src/chat/types.rs` | ChatId, MessageId, ChatPreview, ChatEvent enums |
| `src/chat/manager.rs` | ChatManager: orchestrates all modules, event broadcast |

### imax (binary: `src/`)

| File | Responsibility |
|------|---------------|
| `src/main.rs` | Dioxus desktop launch |
| `src/app.rs` | Root component, router |
| `src/state.rs` | Global signals, ChatManager bridge |
| `src/components/sidebar.rs` | Chat list panel |
| `src/components/chat_view.rs` | Message area |
| `src/components/message_bubble.rs` | Single message bubble |
| `src/components/message_input.rs` | Input field + send |
| `src/components/chat_header.rs` | Peer info header |
| `src/views/onboarding.rs` | Nickname input screen |
| `src/views/main_layout.rs` | Two-panel layout |

---

## Chunk 1: Workspace Setup + Identity Module

### Task 1: Create Cargo workspace

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/imax-core/Cargo.toml`
- Create: `crates/imax-core/src/lib.rs`
- Create: `crates/imax-core/src/error.rs`

- [ ] **Step 1: Convert root to workspace, create imax-core crate**

Root `Cargo.toml`:
```toml
[workspace]
members = ["crates/imax-core"]
resolver = "2"

[package]
name = "imax"
version = "0.1.0"
edition = "2024"

[dependencies]
imax-core = { path = "crates/imax-core" }
tokio = { version = "1", features = ["full"] }
```

`crates/imax-core/Cargo.toml`:
```toml
[package]
name = "imax-core"
version = "0.1.0"
edition = "2024"

[dependencies]
thiserror = "2"
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v4", "serde"] }
tokio = { version = "1", features = ["sync"] }

[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
```

`crates/imax-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Chat error: {0}")]
    Chat(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

`crates/imax-core/src/lib.rs`:
```rust
pub mod error;
pub use error::{Error, Result};
```

`src/main.rs`:
```rust
fn main() {
    println!("iMax - P2P Encrypted Messenger");
}
```

- [ ] **Step 2: Verify workspace compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml crates/ src/main.rs
git commit -m "feat: initialize cargo workspace with imax-core crate"
```

---

### Task 2: Identity — keypair module

**Files:**
- Modify: `crates/imax-core/Cargo.toml` (add crypto deps)
- Create: `crates/imax-core/src/identity/mod.rs`
- Create: `crates/imax-core/src/identity/keypair.rs`
- Create: `crates/imax-core/src/identity/profile.rs`

- [ ] **Step 1: Add identity crypto dependencies**

Add to `crates/imax-core/Cargo.toml` `[dependencies]`:
```toml
bip39 = { version = "2", features = ["rand"] }
ed25519-dalek = { version = "2", features = ["rand_core"] }
x25519-dalek = { version = "2", features = ["static_secrets"] }
curve25519-dalek = { version = "4", features = ["digest"] }
hkdf = "0.12"
sha2 = "0.10"
rand = "0.8"
```

- [ ] **Step 2: Write failing tests for keypair**

Create `crates/imax-core/src/identity/keypair.rs` with tests only:
```rust
use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use bip39::Mnemonic;
use hkdf::Hkdf;
use sha2::Sha256;
use rand::rngs::OsRng;
use crate::Result;

/// Generate a new BIP39 24-word mnemonic
pub fn generate_mnemonic() -> Result<Mnemonic> {
    todo!()
}

/// Parse a mnemonic from a string of space-separated words
pub fn parse_mnemonic(phrase: &str) -> Result<Mnemonic> {
    todo!()
}

/// Derive Ed25519 signing key from mnemonic via HKDF
pub fn derive_signing_key(mnemonic: &Mnemonic) -> SigningKey {
    todo!()
}

/// Convert Ed25519 signing key to X25519 secret (for DH).
/// Uses SHA-512 hash of secret key bytes, clamp first 32 bytes.
pub fn to_x25519_secret(signing_key: &SigningKey) -> X25519Secret {
    todo!()
}

/// Get X25519 public key from Ed25519 verifying key.
/// Decompresses Edwards point and converts to Montgomery form.
pub fn to_x25519_public(verifying_key: &VerifyingKey) -> X25519PublicKey {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic_produces_24_words() {
        let mnemonic = generate_mnemonic().unwrap();
        let words: Vec<&str> = mnemonic.word_iter().collect();
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p imax-core -- identity::keypair`
Expected: FAIL — `todo!()` panics

- [ ] **Step 4: Implement keypair functions**

Replace the `todo!()` bodies:
```rust
pub fn generate_mnemonic() -> Result<Mnemonic> {
    let mnemonic = Mnemonic::generate_in(bip39::Language::English, 24)
        .map_err(|e| crate::Error::Identity(e.to_string()))?;
    Ok(mnemonic)
}

pub fn parse_mnemonic(phrase: &str) -> Result<Mnemonic> {
    let mnemonic = Mnemonic::parse_in(bip39::Language::English, phrase)
        .map_err(|e| crate::Error::Identity(e.to_string()))?;
    Ok(mnemonic)
}

pub fn derive_signing_key(mnemonic: &Mnemonic) -> SigningKey {
    let entropy = mnemonic.to_entropy();
    let hk = Hkdf::<Sha256>::new(None, &entropy);
    let mut okm = [0u8; 32];
    hk.expand(b"imax-identity", &mut okm)
        .expect("32 bytes is valid for HKDF-SHA256");
    SigningKey::from_bytes(&okm)
}

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

pub fn to_x25519_public(verifying_key: &VerifyingKey) -> X25519PublicKey {
    use curve25519_dalek::edwards::CompressedEdwardsY;
    let compressed = CompressedEdwardsY::from_slice(verifying_key.as_bytes())
        .expect("valid 32-byte Edwards point");
    let edwards = compressed.decompress().expect("valid point on curve");
    let montgomery = edwards.to_montgomery();
    X25519PublicKey::from(montgomery.to_bytes())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p imax-core -- identity::keypair`
Expected: All 6 tests PASS

- [ ] **Step 6: Create profile and mod.rs**

`crates/imax-core/src/identity/profile.rs`:
```rust
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
```

`crates/imax-core/src/identity/mod.rs`:
```rust
pub mod keypair;
pub mod profile;

pub use keypair::*;
pub use profile::UserProfile;
```

Update `crates/imax-core/src/lib.rs`:
```rust
pub mod error;
pub mod identity;

pub use error::{Error, Result};
```

- [ ] **Step 7: Run all tests**

Run: `cargo test -p imax-core`
Expected: All 7 tests PASS

- [ ] **Step 8: Commit**

```bash
git add crates/imax-core/
git commit -m "feat: add identity module — BIP39 seed, Ed25519/X25519 keypair derivation"
```

---

## Chunk 2: Crypto + Storage Modules

### Task 3: Crypto — E2E encryption

**Files:**
- Create: `crates/imax-core/src/crypto/mod.rs`
- Create: `crates/imax-core/src/crypto/e2e.rs`
- Modify: `crates/imax-core/Cargo.toml` (add chacha20poly1305)
- Modify: `crates/imax-core/src/lib.rs`

- [ ] **Step 1: Add crypto dependency**

Add to `crates/imax-core/Cargo.toml` `[dependencies]`:
```toml
chacha20poly1305 = "0.10"
```

- [ ] **Step 2: Write failing tests for E2E crypto**

Create `crates/imax-core/src/crypto/e2e.rs`:
```rust
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::RngCore;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};
use crate::Result;

/// Derive a symmetric encryption key from X25519 shared secret
pub fn derive_symmetric_key(
    shared_secret: &[u8; 32],
    our_pubkey: &[u8; 32],
    their_pubkey: &[u8; 32],
) -> [u8; 32] {
    todo!()
}

/// Encrypt plaintext with the symmetric key and message_id as AAD
pub fn encrypt(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, [u8; 24])> {
    todo!()
}

/// Decrypt ciphertext with the symmetric key and message_id as AAD
pub fn decrypt(
    key: &[u8; 32],
    ciphertext: &[u8],
    nonce: &[u8; 24],
    aad: &[u8],
) -> Result<Vec<u8>> {
    todo!()
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

        // Both sides compute same shared secret
        let shared_alice = x_secret_alice.diffie_hellman(&x_pub_bob);
        let shared_bob = x_secret_bob.diffie_hellman(&x_pub_alice);

        let pk_a = sk_alice.verifying_key().to_bytes();
        let pk_b = sk_bob.verifying_key().to_bytes();

        let key_alice = derive_symmetric_key(shared_alice.as_bytes(), &pk_a, &pk_b);
        let key_bob = derive_symmetric_key(shared_bob.as_bytes(), &pk_b, &pk_a);
        assert_eq!(key_alice, key_bob);

        // Alice encrypts, Bob decrypts
        let msg_id = b"msg-uuid-001";
        let (ct, nonce) = encrypt(&key_alice, b"Hello Bob!", msg_id).unwrap();
        let pt = decrypt(&key_bob, &ct, &nonce, msg_id).unwrap();
        assert_eq!(pt, b"Hello Bob!");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p imax-core -- crypto::e2e`
Expected: FAIL — `todo!()` panics

- [ ] **Step 4: Implement crypto functions**

Replace `todo!()` bodies:
```rust
pub fn derive_symmetric_key(
    shared_secret: &[u8; 32],
    our_pubkey: &[u8; 32],
    their_pubkey: &[u8; 32],
) -> [u8; 32] {
    // Sort pubkeys so key is the same regardless of who computes it
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

pub fn encrypt(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, [u8; 24])> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let payload = chacha20poly1305::aead::Payload { msg: plaintext, aad };
    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| crate::Error::Crypto(format!("encryption failed: {e}")))?;

    Ok((ciphertext, nonce_bytes))
}

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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p imax-core -- crypto::e2e`
Expected: All 6 tests PASS

- [ ] **Step 6: Create crypto mod.rs, update lib.rs**

`crates/imax-core/src/crypto/mod.rs`:
```rust
pub mod e2e;
pub use e2e::*;
```

Add to `crates/imax-core/src/lib.rs`:
```rust
pub mod crypto;
```

- [ ] **Step 7: Commit**

```bash
git add crates/imax-core/
git commit -m "feat: add crypto module — XChaCha20-Poly1305 E2E encryption"
```

---

### Task 4: Storage — SQLite database

**Files:**
- Modify: `crates/imax-core/Cargo.toml` (add rusqlite)
- Create: `crates/imax-core/src/storage/mod.rs`
- Create: `crates/imax-core/src/storage/db.rs`
- Create: `crates/imax-core/src/storage/models.rs`
- Modify: `crates/imax-core/src/lib.rs`

- [ ] **Step 1: Add storage dependencies**

Add to `crates/imax-core/Cargo.toml` `[dependencies]`:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
rusqlite_migration = "1"
```

Add to `[dev-dependencies]`:
```toml
tempfile = "3"
```

- [ ] **Step 2: Write failing tests for db.rs**

Create `crates/imax-core/src/storage/db.rs`:
```rust
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use crate::Result;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open database at the given path (creates if not exists)
    pub fn open(path: &str) -> Result<Self> {
        todo!()
    }

    /// Open in-memory database (for tests)
    pub fn open_in_memory() -> Result<Self> {
        todo!()
    }

    /// Get a reference to the underlying connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE identity (
                id            INTEGER PRIMARY KEY CHECK (id = 1),
                seed_encrypted BLOB NOT NULL,
                seed_nonce    BLOB NOT NULL,
                public_key    BLOB NOT NULL,
                nickname      TEXT NOT NULL,
                created_at    INTEGER NOT NULL
            );

            CREATE TABLE contacts (
                public_key    BLOB PRIMARY KEY,
                nickname      TEXT NOT NULL,
                node_id       BLOB,
                added_at      INTEGER NOT NULL,
                last_seen     INTEGER
            );

            CREATE TABLE chats (
                id            TEXT PRIMARY KEY,
                peer_key      BLOB NOT NULL UNIQUE,
                created_at    INTEGER NOT NULL,
                last_message  TEXT,
                unread_count  INTEGER DEFAULT 0
            );

            CREATE TABLE messages (
                id            TEXT PRIMARY KEY,
                chat_id       TEXT NOT NULL,
                sender_key    BLOB NOT NULL,
                content       TEXT NOT NULL,
                seq           INTEGER NOT NULL,
                status        TEXT NOT NULL CHECK (status IN ('pending','sent','delivered','read')),
                created_at    INTEGER NOT NULL,
                received_at   INTEGER
            );

            CREATE TABLE pending_invites (
                code          TEXT PRIMARY KEY,
                created_at    INTEGER NOT NULL,
                expires_at    INTEGER
            );

            CREATE INDEX idx_messages_chat ON messages(chat_id, created_at);
            CREATE INDEX idx_messages_status ON messages(status) WHERE status = 'pending';"
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory().unwrap();
        // Verify tables exist by querying sqlite_master
        let count: i32 = db.conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('identity','contacts','chats','messages','pending_invites')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 5);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p imax-core -- storage::db`
Expected: FAIL — `todo!()` panic

- [ ] **Step 4: Implement Database**

Replace `todo!()` bodies:
```rust
pub fn open(path: &str) -> Result<Self> {
    let mut conn = Connection::open(path)
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    migrations()
        .to_latest(&mut conn)
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(Self { conn })
}

pub fn open_in_memory() -> Result<Self> {
    let mut conn = Connection::open_in_memory()
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    migrations()
        .to_latest(&mut conn)
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(Self { conn })
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p imax-core -- storage::db`
Expected: PASS

- [ ] **Step 6: Write failing tests for models.rs**

Create `crates/imax-core/src/storage/models.rs` with CRUD operations and tests:
```rust
use rusqlite::params;
use uuid::Uuid;
use crate::storage::db::Database;
use crate::Result;

// ── Identity CRUD ──

pub fn save_identity(db: &Database, seed_encrypted: &[u8], seed_nonce: &[u8], public_key: &[u8; 32], nickname: &str) -> Result<()> {
    todo!()
}

pub fn get_identity(db: &Database) -> Result<Option<(Vec<u8>, Vec<u8>, Vec<u8>, String)>> {
    todo!()
}

// ── Contact CRUD ──

pub fn insert_contact(db: &Database, public_key: &[u8; 32], nickname: &str, node_id: Option<&[u8]>) -> Result<()> {
    todo!()
}

pub fn get_contact(db: &Database, public_key: &[u8; 32]) -> Result<Option<(Vec<u8>, String, Option<Vec<u8>>)>> {
    todo!()
}

// ── Chat CRUD ──

pub fn create_chat(db: &Database, peer_key: &[u8; 32]) -> Result<String> {
    todo!()
}

pub fn get_chats(db: &Database) -> Result<Vec<(String, Vec<u8>, i64, Option<String>, i32)>> {
    todo!()
}

// ── Message CRUD ──

pub fn insert_message(
    db: &Database,
    chat_id: &str,
    sender_key: &[u8; 32],
    content: &str,
    seq: i64,
    status: &str,
) -> Result<String> {
    todo!()
}

pub fn get_messages(db: &Database, chat_id: &str, limit: usize, before_seq: Option<i64>) -> Result<Vec<(String, Vec<u8>, String, i64, String, i64)>> {
    todo!()
}

pub fn update_message_status(db: &Database, message_id: &str, status: &str) -> Result<()> {
    todo!()
}

pub fn get_next_seq(db: &Database, chat_id: &str) -> Result<i64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_save_and_get_identity() {
        let db = setup();
        save_identity(&db, b"encrypted_seed", b"nonce_bytes_here", &[1u8; 32], "Max").unwrap();
        let identity = get_identity(&db).unwrap().unwrap();
        assert_eq!(identity.3, "Max");
        assert_eq!(identity.2, vec![1u8; 32]);
    }

    #[test]
    fn test_insert_and_get_contact() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let contact = get_contact(&db, &pk).unwrap().unwrap();
        assert_eq!(contact.1, "Alice");
    }

    #[test]
    fn test_create_and_get_chats() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        let chats = get_chats(&db).unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].0, chat_id);
    }

    #[test]
    fn test_insert_and_get_messages() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();

        insert_message(&db, &chat_id, &pk, "Hello!", 1, "sent").unwrap();
        insert_message(&db, &chat_id, &pk, "World!", 2, "sent").unwrap();

        let msgs = get_messages(&db, &chat_id, 10, None).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].2, "Hello!");
        assert_eq!(msgs[1].2, "World!");
    }

    #[test]
    fn test_update_message_status() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        let msg_id = insert_message(&db, &chat_id, &pk, "Hi", 1, "pending").unwrap();

        update_message_status(&db, &msg_id, "delivered").unwrap();

        let msgs = get_messages(&db, &chat_id, 10, None).unwrap();
        assert_eq!(msgs[0].4, "delivered");
    }

    #[test]
    fn test_get_next_seq() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();

        assert_eq!(get_next_seq(&db, &chat_id).unwrap(), 1);
        insert_message(&db, &chat_id, &pk, "Hi", 1, "sent").unwrap();
        assert_eq!(get_next_seq(&db, &chat_id).unwrap(), 2);
    }

    #[test]
    fn test_get_messages_pagination() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();

        for i in 1..=5 {
            insert_message(&db, &chat_id, &pk, &format!("msg {i}"), i, "sent").unwrap();
        }

        let msgs = get_messages(&db, &chat_id, 2, Some(4)).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].2, "msg 2");
        assert_eq!(msgs[1].2, "msg 3");
    }
}
```

- [ ] **Step 7: Run tests to verify they fail**

Run: `cargo test -p imax-core -- storage::models`
Expected: FAIL — `todo!()` panics

- [ ] **Step 8: Implement models CRUD**

Replace all `todo!()` bodies:
```rust
pub fn save_identity(db: &Database, seed_encrypted: &[u8], seed_nonce: &[u8], public_key: &[u8; 32], nickname: &str) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT OR REPLACE INTO identity (id, seed_encrypted, seed_nonce, public_key, nickname, created_at) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
        params![seed_encrypted, seed_nonce, public_key.as_slice(), nickname, now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_identity(db: &Database) -> Result<Option<(Vec<u8>, Vec<u8>, Vec<u8>, String)>> {
    let mut stmt = db.conn().prepare(
        "SELECT seed_encrypted, seed_nonce, public_key, nickname FROM identity WHERE id = 1"
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    let result = stmt.query_row([], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, Vec<u8>>(2)?, row.get::<_, String>(3)?))
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::Error::Storage(e.to_string())),
    }
}

pub fn insert_contact(db: &Database, public_key: &[u8; 32], nickname: &str, node_id: Option<&[u8]>) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT OR REPLACE INTO contacts (public_key, nickname, node_id, added_at) VALUES (?1, ?2, ?3, ?4)",
        params![public_key.as_slice(), nickname, node_id, now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_contact(db: &Database, public_key: &[u8; 32]) -> Result<Option<(Vec<u8>, String, Option<Vec<u8>>)>> {
    let mut stmt = db.conn().prepare(
        "SELECT public_key, nickname, node_id FROM contacts WHERE public_key = ?1"
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;

    let result = stmt.query_row(params![public_key.as_slice()], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<Vec<u8>>>(2)?))
    });

    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::Error::Storage(e.to_string())),
    }
}

pub fn create_chat(db: &Database, peer_key: &[u8; 32]) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT INTO chats (id, peer_key, created_at) VALUES (?1, ?2, ?3)",
        params![id, peer_key.as_slice(), now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(id)
}

pub fn get_chats(db: &Database) -> Result<Vec<(String, Vec<u8>, i64, Option<String>, i32)>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, peer_key, created_at, last_message, unread_count FROM chats ORDER BY created_at DESC"
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i32>(4)?,
        ))
    }).map_err(|e| crate::Error::Storage(e.to_string()))?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::Error::Storage(e.to_string()))
}

pub fn insert_message(
    db: &Database,
    chat_id: &str,
    sender_key: &[u8; 32],
    content: &str,
    seq: i64,
    status: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT INTO messages (id, chat_id, sender_key, content, seq, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, chat_id, sender_key.as_slice(), content, seq, status, now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;

    // Update chat's last_message
    db.conn().execute(
        "UPDATE chats SET last_message = ?1 WHERE id = ?2",
        params![id, chat_id],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;

    Ok(id)
}

pub fn get_messages(db: &Database, chat_id: &str, limit: usize, before_seq: Option<i64>) -> Result<Vec<(String, Vec<u8>, String, i64, String, i64)>> {
    let query = match before_seq {
        Some(_) => "SELECT id, sender_key, content, seq, status, created_at FROM messages WHERE chat_id = ?1 AND seq < ?2 ORDER BY seq ASC LIMIT ?3",
        None => "SELECT id, sender_key, content, seq, status, created_at FROM messages WHERE chat_id = ?1 ORDER BY seq ASC LIMIT ?2",
    };

    let mut stmt = db.conn().prepare(query)
        .map_err(|e| crate::Error::Storage(e.to_string()))?;

    let rows = match before_seq {
        Some(seq) => stmt.query_map(params![chat_id, seq, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        }),
        None => stmt.query_map(params![chat_id, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        }),
    }.map_err(|e| crate::Error::Storage(e.to_string()))?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::Error::Storage(e.to_string()))
}

pub fn update_message_status(db: &Database, message_id: &str, status: &str) -> Result<()> {
    db.conn().execute(
        "UPDATE messages SET status = ?1 WHERE id = ?2",
        params![status, message_id],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_next_seq(db: &Database, chat_id: &str) -> Result<i64> {
    let max_seq: Option<i64> = db.conn()
        .query_row(
            "SELECT MAX(seq) FROM messages WHERE chat_id = ?1",
            params![chat_id],
            |r| r.get(0),
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(max_seq.unwrap_or(0) + 1)
}
```

- [ ] **Step 9: Create storage mod.rs, update lib.rs**

`crates/imax-core/src/storage/mod.rs`:
```rust
pub mod db;
pub mod models;

pub use db::Database;
```

Add to `crates/imax-core/src/lib.rs`:
```rust
pub mod storage;
```

- [ ] **Step 10: Run all tests**

Run: `cargo test -p imax-core`
Expected: All tests PASS (identity: 7, crypto: 6, storage: 7 = ~20 tests)

- [ ] **Step 11: Commit**

```bash
git add crates/imax-core/
git commit -m "feat: add storage module — SQLite schema, migrations, CRUD operations"
```

---

## Chunk 3: Network Module

### Task 5: Wire protocol — serialization

**Files:**
- Modify: `crates/imax-core/Cargo.toml` (add postcard, iroh)
- Create: `crates/imax-core/src/network/mod.rs`
- Create: `crates/imax-core/src/network/protocol.rs`

- [ ] **Step 1: Add network dependencies**

Add to `crates/imax-core/Cargo.toml` `[dependencies]`:
```toml
postcard = { version = "1", features = ["alloc"] }
iroh = "0.32"
bytes = "1"
```

- [ ] **Step 2: Write failing tests for protocol**

Create `crates/imax-core/src/network/protocol.rs`:
```rust
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

/// Serialize a WireMessage to length-prefixed bytes
pub fn encode(msg: &WireMessage) -> Result<Vec<u8>> {
    todo!()
}

/// Deserialize a WireMessage from raw bytes (without length prefix)
pub fn decode(data: &[u8]) -> Result<WireMessage> {
    todo!()
}

/// Read a length-prefixed frame from a buffer. Returns (message, bytes_consumed) or None if incomplete.
pub fn decode_frame(buf: &[u8]) -> Result<Option<(WireMessage, usize)>> {
    todo!()
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
        // Give it only half the data
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p imax-core -- network::protocol`
Expected: FAIL — `todo!()` panics

- [ ] **Step 4: Implement encode/decode**

Replace `todo!()` bodies:
```rust
pub fn encode(msg: &WireMessage) -> Result<Vec<u8>> {
    let payload = postcard::to_allocvec(msg)
        .map_err(|e| crate::Error::Network(format!("encode error: {e}")))?;
    let len = (payload.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

pub fn decode(data: &[u8]) -> Result<WireMessage> {
    postcard::from_bytes(data)
        .map_err(|e| crate::Error::Network(format!("decode error: {e}")))
}

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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p imax-core -- network::protocol`
Expected: All 5 tests PASS

- [ ] **Step 6: Create mod.rs, update lib.rs**

`crates/imax-core/src/network/mod.rs`:
```rust
pub mod protocol;

pub use protocol::*;
```

Add to `crates/imax-core/src/lib.rs`:
```rust
pub mod network;
```

- [ ] **Step 7: Commit**

```bash
git add crates/imax-core/
git commit -m "feat: add wire protocol — postcard serialization with length-prefixed framing"
```

---

### Task 6: Network — iroh node + invite codes

**Files:**
- Create: `crates/imax-core/src/network/node.rs`
- Create: `crates/imax-core/src/network/discovery.rs`
- Modify: `crates/imax-core/src/network/mod.rs`

- [ ] **Step 1: Add base58 dependency**

Add to `crates/imax-core/Cargo.toml` `[dependencies]`:
```toml
bs58 = "0.5"
```

- [ ] **Step 2: Write invite code tests and implement**

Create `crates/imax-core/src/network/discovery.rs`:
```rust
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

    pub fn as_str(&self) -> &str {
        &self.0
    }
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
        assert_eq!(decoded.addrs, payload.addrs);
        assert_eq!(decoded.relay_url, payload.relay_url);
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
        assert!(decoded.relay_url.is_none());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p imax-core -- network::discovery`
Expected: All 3 tests PASS

- [ ] **Step 4: Create IrohNode stub**

Create `crates/imax-core/src/network/node.rs`:
```rust
use iroh::{Endpoint, NodeId, SecretKey};
use crate::Result;

pub const ALPN: &[u8] = b"imax/1";

pub struct IrohNode {
    endpoint: Endpoint,
    secret_key: SecretKey,
}

impl IrohNode {
    /// Create and bind a new iroh endpoint
    pub async fn new(secret_key: SecretKey) -> Result<Self> {
        let endpoint = Endpoint::builder()
            .secret_key(secret_key.clone())
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| crate::Error::Network(e.to_string()))?;

        Ok(Self { endpoint, secret_key })
    }

    pub fn node_id(&self) -> NodeId {
        self.endpoint.node_id()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub async fn shutdown(self) -> Result<()> {
        self.endpoint.close().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_iroh_node() {
        let secret_key = SecretKey::generate(rand::rngs::OsRng);
        let node = IrohNode::new(secret_key).await.unwrap();
        // NodeId should be derived from the secret key
        assert!(!node.node_id().as_bytes().iter().all(|&b| b == 0));
        node.shutdown().await.unwrap();
    }
}
```

- [ ] **Step 5: Update network/mod.rs**

```rust
pub mod protocol;
pub mod discovery;
pub mod node;

pub use protocol::*;
pub use discovery::{InviteCode, InvitePayload};
pub use node::IrohNode;
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p imax-core`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/imax-core/
git commit -m "feat: add iroh node and invite code encode/decode"
```

---

## Chunk 4: Chat Module + Integration

### Task 7: Chat types and ChatManager

**Files:**
- Create: `crates/imax-core/src/chat/mod.rs`
- Create: `crates/imax-core/src/chat/types.rs`
- Create: `crates/imax-core/src/chat/manager.rs`
- Modify: `crates/imax-core/src/lib.rs`

- [ ] **Step 1: Create chat types**

`crates/imax-core/src/chat/types.rs`:
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type ChatId = String;
pub type MessageId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPreview {
    pub id: ChatId,
    pub peer_key: [u8; 32],
    pub peer_nickname: String,
    pub last_message_text: Option<String>,
    pub last_message_time: Option<i64>,
    pub unread_count: i32,
    pub is_online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub sender_key: [u8; 32],
    pub content: String,
    pub seq: i64,
    pub status: MessageStatus,
    pub created_at: i64,
    pub is_mine: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Read,
}

impl MessageStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "sent" => Self::Sent,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatEvent {
    NewMessage { chat_id: ChatId, message: Message },
    MessageStatusChanged { message_id: MessageId, status: MessageStatus },
    PeerOnline { public_key: [u8; 32] },
    PeerOffline { public_key: [u8; 32] },
    InviteAccepted { chat_id: ChatId, peer_nickname: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_status_roundtrip() {
        for status in [MessageStatus::Pending, MessageStatus::Sent, MessageStatus::Delivered, MessageStatus::Read] {
            assert_eq!(MessageStatus::from_str(status.as_str()), status);
        }
    }
}
```

- [ ] **Step 2: Create ChatManager**

`crates/imax-core/src/chat/manager.rs`:
```rust
use tokio::sync::broadcast;
use crate::chat::types::*;
use crate::storage::Database;
use crate::storage::models;
use crate::Result;

pub struct ChatManager {
    db: Database,
    my_pubkey: [u8; 32],
    event_tx: broadcast::Sender<ChatEvent>,
}

impl ChatManager {
    pub fn new(db: Database, my_pubkey: [u8; 32]) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self { db, my_pubkey, event_tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent> {
        self.event_tx.subscribe()
    }

    pub fn get_chats(&self) -> Result<Vec<ChatPreview>> {
        let raw_chats = models::get_chats(&self.db)?;
        let mut previews = Vec::new();
        for (id, peer_key_bytes, _created_at, last_msg_id, unread) in raw_chats {
            let pk: [u8; 32] = peer_key_bytes.try_into()
                .map_err(|_| crate::Error::Chat("invalid pubkey length".into()))?;
            let contact = models::get_contact(&self.db, &pk)?;
            let nickname = contact.map(|c| c.1).unwrap_or_else(|| "Unknown".to_string());

            let (last_text, last_time) = if last_msg_id.is_some() {
                // Get latest message for preview
                let latest = self.db.conn().query_row(
                    "SELECT content, created_at FROM messages WHERE chat_id = ?1 ORDER BY seq DESC LIMIT 1",
                    rusqlite::params![id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                ).ok();
                match latest {
                    Some((text, time)) => (Some(text), Some(time)),
                    None => (None, None),
                }
            } else {
                (None, None)
            };

            previews.push(ChatPreview {
                id,
                peer_key: pk,
                peer_nickname: nickname,
                last_message_text: last_text,
                last_message_time: last_time,
                unread_count: unread,
                is_online: false,
            });
        }
        Ok(previews)
    }

    pub fn get_messages(&self, chat_id: &str, limit: usize, before_seq: Option<i64>) -> Result<Vec<Message>> {
        let raw = models::get_messages(&self.db, chat_id, limit, before_seq)?;
        Ok(raw.into_iter().map(|(id, sender, content, seq, status, created_at)| {
            let sk: [u8; 32] = sender.try_into().unwrap_or([0u8; 32]);
            Message {
                id,
                chat_id: chat_id.to_string(),
                sender_key: sk,
                content,
                seq,
                status: MessageStatus::from_str(&status),
                created_at,
                is_mine: sk == self.my_pubkey,
            }
        }).collect())
    }

    pub fn add_contact_and_chat(&self, peer_key: &[u8; 32], nickname: &str) -> Result<ChatId> {
        models::insert_contact(&self.db, peer_key, nickname, None)?;
        models::create_chat(&self.db, peer_key)
    }

    pub fn store_outgoing_message(&self, chat_id: &str, text: &str) -> Result<Message> {
        let seq = models::get_next_seq(&self.db, chat_id)?;
        let msg_id = models::insert_message(&self.db, chat_id, &self.my_pubkey, text, seq, "pending")?;
        let msg = Message {
            id: msg_id,
            chat_id: chat_id.to_string(),
            sender_key: self.my_pubkey,
            content: text.to_string(),
            seq,
            status: MessageStatus::Pending,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            is_mine: true,
        };
        let _ = self.event_tx.send(ChatEvent::NewMessage {
            chat_id: chat_id.to_string(),
            message: msg.clone(),
        });
        Ok(msg)
    }

    pub fn store_incoming_message(&self, chat_id: &str, sender_key: &[u8; 32], text: &str) -> Result<Message> {
        let seq = models::get_next_seq(&self.db, chat_id)?;
        let msg_id = models::insert_message(&self.db, chat_id, sender_key, text, seq, "delivered")?;
        let msg = Message {
            id: msg_id,
            chat_id: chat_id.to_string(),
            sender_key: *sender_key,
            content: text.to_string(),
            seq,
            status: MessageStatus::Delivered,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            is_mine: false,
        };
        let _ = self.event_tx.send(ChatEvent::NewMessage {
            chat_id: chat_id.to_string(),
            message: msg.clone(),
        });
        Ok(msg)
    }

    pub fn update_status(&self, message_id: &str, status: MessageStatus) -> Result<()> {
        models::update_message_status(&self.db, message_id, status.as_str())?;
        let _ = self.event_tx.send(ChatEvent::MessageStatusChanged {
            message_id: message_id.to_string(),
            status,
        });
        Ok(())
    }

    pub fn db(&self) -> &Database {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> ChatManager {
        let db = Database::open_in_memory().unwrap();
        ChatManager::new(db, [0u8; 32])
    }

    #[test]
    fn test_add_contact_and_chat() {
        let mgr = setup();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Alice").unwrap();
        let chats = mgr.get_chats().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].id, chat_id);
        assert_eq!(chats[0].peer_nickname, "Alice");
    }

    #[test]
    fn test_send_and_receive_messages() {
        let mgr = setup();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Bob").unwrap();

        mgr.store_outgoing_message(&chat_id, "Hello Bob!").unwrap();
        mgr.store_incoming_message(&chat_id, &peer, "Hi there!").unwrap();

        let msgs = mgr.get_messages(&chat_id, 10, None).unwrap();
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].is_mine);
        assert_eq!(msgs[0].content, "Hello Bob!");
        assert!(!msgs[1].is_mine);
        assert_eq!(msgs[1].content, "Hi there!");
    }

    #[test]
    fn test_event_broadcast() {
        let mgr = setup();
        let mut rx = mgr.subscribe();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Eve").unwrap();

        mgr.store_outgoing_message(&chat_id, "Test").unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            ChatEvent::NewMessage { message, .. } => {
                assert_eq!(message.content, "Test");
            }
            _ => panic!("expected NewMessage event"),
        }
    }

    #[test]
    fn test_update_status() {
        let mgr = setup();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Dave").unwrap();
        let msg = mgr.store_outgoing_message(&chat_id, "Status test").unwrap();

        mgr.update_status(&msg.id, MessageStatus::Delivered).unwrap();

        let msgs = mgr.get_messages(&chat_id, 10, None).unwrap();
        assert_eq!(msgs[0].status, MessageStatus::Delivered);
    }
}
```

- [ ] **Step 3: Create chat/mod.rs, update lib.rs**

`crates/imax-core/src/chat/mod.rs`:
```rust
pub mod types;
pub mod manager;

pub use types::*;
pub use manager::ChatManager;
```

Add to `crates/imax-core/src/lib.rs`:
```rust
pub mod chat;
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p imax-core`
Expected: All tests PASS (~30+ tests total)

- [ ] **Step 5: Commit**

```bash
git add crates/imax-core/
git commit -m "feat: add chat module — ChatManager, types, event broadcasting"
```

---

## Chunk 5: Dioxus UI

### Task 8: Dioxus desktop setup

**Files:**
- Modify: `Cargo.toml` (add dioxus deps)
- Create: `src/main.rs`, `src/app.rs`, `src/state.rs`
- Create: `Dioxus.toml`
- Create: `assets/main.css`

- [ ] **Step 1: Add Dioxus dependencies to root Cargo.toml**

Update root `Cargo.toml` `[dependencies]`:
```toml
imax-core = { path = "crates/imax-core" }
dioxus = { version = "0.7", features = ["desktop"] }
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Create Dioxus.toml config**

`Dioxus.toml`:
```toml
[application]
name = "iMax"

[web.app]
title = "iMax"

[desktop]
always_on_top = false
```

- [ ] **Step 3: Create CSS file**

Create `assets/main.css` with Telegram-dark color scheme (based on approved mockup). This is a large file — implement the core layout CSS from the mockup HTML.

- [ ] **Step 4: Create src/state.rs**

```rust
use dioxus::prelude::*;
use imax_core::chat::{ChatManager, ChatPreview, Message, ChatEvent};

#[derive(Clone)]
pub struct AppState {
    pub chats: Signal<Vec<ChatPreview>>,
    pub active_chat_id: Signal<Option<String>>,
    pub messages: Signal<Vec<Message>>,
    pub is_onboarded: Signal<bool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            chats: Signal::new(vec![]),
            active_chat_id: Signal::new(None),
            messages: Signal::new(vec![]),
            is_onboarded: Signal::new(false),
        }
    }
}
```

- [ ] **Step 5: Create src/app.rs**

```rust
use dioxus::prelude::*;
use crate::state::AppState;
use crate::views::{onboarding::Onboarding, main_layout::MainLayout};

#[component]
pub fn App() -> Element {
    let state = use_context_provider(|| AppState::new());

    rsx! {
        link { rel: "stylesheet", href: asset!("./assets/main.css") }
        if *state.is_onboarded.read() {
            MainLayout {}
        } else {
            Onboarding {}
        }
    }
}
```

- [ ] **Step 6: Create src/main.rs**

```rust
mod app;
mod state;
mod components;
mod views;

use dioxus::prelude::*;

fn main() {
    dioxus::launch(app::App);
}
```

- [ ] **Step 7: Create stub component/view files**

Create stub files so it compiles:
- `src/components/mod.rs` — `pub mod sidebar; pub mod chat_view; pub mod message_bubble; pub mod message_input; pub mod chat_header;`
- Each component file: empty component returning placeholder div
- `src/views/mod.rs` — `pub mod onboarding; pub mod main_layout;`
- Each view file: empty component returning placeholder div

- [ ] **Step 8: Verify it compiles and runs**

Run: `cargo build`
Run: `cargo run` (should show a window)
Expected: Desktop window opens with placeholder content

- [ ] **Step 9: Commit**

```bash
git add src/ Dioxus.toml assets/
git commit -m "feat: add Dioxus desktop app shell with routing and state"
```

---

### Task 9: Implement UI components

**Files:**
- Modify: `src/views/onboarding.rs`
- Modify: `src/views/main_layout.rs`
- Modify: `src/components/sidebar.rs`
- Modify: `src/components/chat_view.rs`
- Modify: `src/components/message_bubble.rs`
- Modify: `src/components/message_input.rs`
- Modify: `src/components/chat_header.rs`
- Modify: `assets/main.css`

This task implements the Telegram-style UI from the approved mockup. Each component maps to a section of the mockup HTML.

- [ ] **Step 1: Implement Onboarding view**

Simple screen: nickname input + "Start Messaging" button. On submit, creates identity (generate_mnemonic + derive_signing_key), opens DB, sets is_onboarded = true.

- [ ] **Step 2: Implement MainLayout — two-panel split**

Left sidebar (320px) + right chat panel (flex). Uses flexbox.

- [ ] **Step 3: Implement Sidebar component**

Chat list with avatars, names, last message preview, unread badges, online indicators. Maps over `state.chats` signal.

- [ ] **Step 4: Implement ChatHeader component**

Peer avatar, name, online status, SVG action icons.

- [ ] **Step 5: Implement MessageBubble component**

Incoming (left-aligned, dark bg) and outgoing (right-aligned, blue bg) with timestamps and check marks.

- [ ] **Step 6: Implement ChatView component**

Scrollable message list. Maps over `state.messages` signal. Renders MessageBubble for each.

- [ ] **Step 7: Implement MessageInput component**

Text input with attach/emoji/send buttons. On send, calls `ChatManager::store_outgoing_message`.

- [ ] **Step 8: Add full CSS from mockup**

Port all CSS from the approved `imax-telegram-style.html` mockup into `assets/main.css`.

- [ ] **Step 9: Verify UI renders correctly**

Run: `cargo run`
Expected: Two-panel Telegram-style layout, onboarding on first run

- [ ] **Step 10: Commit**

```bash
git add src/ assets/
git commit -m "feat: implement Telegram-style UI components"
```

---

### Task 10: Wire up core ↔ UI

**Files:**
- Modify: `src/state.rs`
- Modify: `src/views/onboarding.rs`
- Modify: `src/views/main_layout.rs`
- Modify: `src/components/message_input.rs`

- [ ] **Step 1: Connect onboarding to identity creation**

On "Start Messaging": generate mnemonic, derive keys, create DB at `~/.imax/data.db`, save identity, set onboarded.

- [ ] **Step 2: Add coroutine for ChatEvent listener**

In MainLayout, spawn `use_coroutine` that reads from `ChatManager::subscribe()` and updates signals (new messages, status changes, online/offline).

- [ ] **Step 3: Connect message input to ChatManager**

On send: call `store_outgoing_message`, update messages signal.

- [ ] **Step 4: Connect sidebar to chats signal**

Click on chat → set active_chat_id → load messages.

- [ ] **Step 5: Verify end-to-end local flow**

Run: `cargo run`
Expected: Can create identity, see empty chat list, manually add a chat would show in list

- [ ] **Step 6: Commit**

```bash
git add src/
git commit -m "feat: wire up core ChatManager to Dioxus UI"
```

---

## Chunk 6: P2P Integration + Final

### Task 11: Connect iroh networking to ChatManager

**Files:**
- Modify: `crates/imax-core/src/chat/manager.rs`
- Modify: `crates/imax-core/src/network/node.rs`

- [ ] **Step 1: Add connection handling to IrohNode**

Implement `accept_connections()` loop that reads incoming WireMessages, processes Hello/ChatMessage/Ack/Sync, and delegates to ChatManager.

- [ ] **Step 2: Add `send_to_peer()` method**

Connect to peer by NodeId, send length-prefixed WireMessage.

- [ ] **Step 3: Integrate invite flow**

`ChatManager::generate_invite()` — creates InvitePayload from iroh endpoint info.
`ChatManager::accept_invite()` — connects to peer, exchanges Hello, saves contact + chat.

- [ ] **Step 4: Integrate message sending**

`ChatManager::send_message()` — encrypts via crypto module, sends ChatMessage via iroh, updates status on ack.

- [ ] **Step 5: Test with two instances**

Run two instances of iMax on the same machine. Generate invite on one, accept on the other. Send messages back and forth.

- [ ] **Step 6: Commit**

```bash
git add crates/imax-core/ src/
git commit -m "feat: integrate iroh P2P networking with ChatManager"
```

---

### Task 12: Final integration tests

**Files:**
- Create: `crates/imax-core/tests/integration.rs`

- [ ] **Step 1: Write integration test — two nodes communicate**

```rust
#[tokio::test]
async fn test_two_nodes_exchange_messages() {
    // 1. Create two IrohNodes with different keys
    // 2. Node A generates invite, Node B accepts
    // 3. Hello exchange
    // 4. A sends encrypted message to B
    // 5. B receives, decrypts, acks
    // 6. Verify message content matches
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test -p imax-core --test integration`
Expected: PASS

- [ ] **Step 3: Final commit**

```bash
git add .
git commit -m "feat: add integration tests for P2P message exchange"
```

---

## Verification

After completing all tasks:

1. `cargo test -p imax-core` — all unit tests pass
2. `cargo test -p imax-core --test integration` — integration test passes
3. `cargo run` — desktop app opens, can create identity
4. Run two instances, exchange invite codes, send messages — E2E works
5. Messages persist after restart (SQLite)
