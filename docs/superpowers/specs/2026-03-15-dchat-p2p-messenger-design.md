# iMax — P2P Encrypted Messenger Design Spec

## Overview

iMax is a peer-to-peer encrypted messenger inspired by Keet.io, built entirely in Rust. It uses iroh for P2P networking, Dioxus for cross-platform UI, and provides end-to-end encryption for all messages.

**Target platforms:** Desktop, Web, Mobile (via Dioxus)

## Architecture

### Approach: Monolith Core

Single library crate (`imax-core`) containing all business logic, P2P networking, cryptography, and storage. The Dioxus UI is a thin shell that calls into core via Rust API.

```
┌─────────────────────────────────┐
│  Dioxus UI (Desktop/Web/Mobile) │
└──────────────┬──────────────────┘
               │ Rust API calls
┌──────────────▼──────────────────┐
│   imax-core (library crate)  │
│  ┌────────┐ ┌──────┐ ┌───────┐ │
│  │Network │ │Crypto│ │Storage│ │
│  │ (iroh) │ │(E2E) │ │(SQLite)│ │
│  └────────┘ └──────┘ └───────┘ │
└─────────────────────────────────┘
```

### Project Structure

```
imax/
├── Cargo.toml                  # workspace root
│
├── crates/
│   └── imax-core/             # library — all logic
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs           # re-exports
│           ├── identity/        # keypair, seed phrase, nickname
│           │   ├── mod.rs
│           │   ├── keypair.rs   # Ed25519 from BIP39
│           │   └── profile.rs   # nickname, avatar (later)
│           ├── network/         # P2P via iroh
│           │   ├── mod.rs
│           │   ├── node.rs      # iroh endpoint, connections
│           │   ├── discovery.rs # invite codes, DHT lookup
│           │   └── protocol.rs  # wire message protocol
│           ├── crypto/          # E2E encryption
│           │   ├── mod.rs
│           │   └── e2e.rs       # encrypt/decrypt with X25519
│           ├── storage/         # SQLite
│           │   ├── mod.rs
│           │   ├── db.rs        # schema, migrations
│           │   └── models.rs    # Message, Contact, Chat
│           └── chat/            # chat business logic
│               ├── mod.rs
│               ├── manager.rs   # ChatManager — orchestrator
│               └── types.rs     # Chat, Message, Event
│
└── src/                        # Dioxus UI (binary)
    ├── main.rs                  # entry point
    ├── app.rs                   # root component
    ├── state.rs                 # global state (signals)
    ├── components/              # UI components
    │   ├── sidebar.rs           # chat list panel
    │   ├── chat_view.rs         # message area
    │   ├── message_bubble.rs    # single message
    │   ├── message_input.rs     # input field
    │   └── chat_header.rs       # peer info header
    └── views/                   # screens
        ├── onboarding.rs        # seed phrase / restore
        └── main_layout.rs       # two-panel layout
```

## Technology Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (edition 2024) |
| P2P Networking | iroh (QUIC, NAT traversal, DHT) |
| UI Framework | Dioxus (desktop, web, mobile) |
| Storage | SQLite (via rusqlite) |
| Serialization | postcard (serde, compact binary) |
| E2E Encryption | XChaCha20-Poly1305 + X25519 DH |
| Identity | Ed25519 keypair from BIP39 seed phrase |

## Identity

### Key Derivation

```
seed_phrase (24 words, BIP39)
    │
    ▼  HKDF-SHA256 (info = "imax-identity")
┌──────────────────────┐
│  master_secret       │
│  (32 bytes)          │
└──────┬───────────────┘
       │
       └──▶ Ed25519 keypair        // identity (signatures, iroh NodeId)
             ├── public_key         // = user identifier = iroh NodeId
             └── secret_key         // stored locally
                    │
                    ▼  RFC 7748 conversion (clamped scalar)
              X25519 keypair        // encryption (DH key exchange)
                ├── public_key      // derived from Ed25519, sent in Hello
                └── secret_key      // derived from Ed25519
```

X25519 keypair is derived from Ed25519 via standard RFC 7748 conversion (not independent derivation). This ensures a single keypair identity.

**iroh NodeId = Ed25519 public key.** iroh's QUIC TLS already authenticates the peer by NodeId, so the Hello message's public_key is verified by the transport layer. No additional signing of Hello is needed.

### User Profile

- Public key serves as the unique identifier
- User sets a human-readable nickname
- Nickname is transmitted to peers during handshake

## Discovery

### First Contact: Invite Code

1. Alice generates an invite code containing her public key + iroh connection info
2. Alice sends the code to Bob via any channel (messenger, voice, QR)
3. Bob enters the code in iMax
4. iroh establishes a QUIC connection
5. Key exchange occurs (Hello messages)
6. Both sides save each other as contacts

### Reconnection: DHT

After first contact, peers reconnect automatically via iroh DHT using the known NodeId (public key). No invite code needed for subsequent connections.

## Data Model (SQLite)

```sql
-- Local user profile
CREATE TABLE identity (
    id            INTEGER PRIMARY KEY CHECK (id = 1),  -- singleton
    seed_encrypted BLOB NOT NULL,         -- BIP39 seed phrase, encrypted with OS keychain key
    seed_nonce    BLOB NOT NULL,          -- nonce for seed encryption
    public_key    BLOB NOT NULL,          -- Ed25519 public key
    nickname      TEXT NOT NULL,          -- display name
    created_at    INTEGER NOT NULL        -- unix timestamp
);
-- Seed phrase encryption: XChaCha20-Poly1305 with a key stored in OS keychain
-- (macOS Keychain, Windows Credential Manager, Linux Secret Service).
-- Fallback: user-provided password → Argon2id → encryption key.

-- Contacts (known peers)
CREATE TABLE contacts (
    public_key    BLOB PRIMARY KEY,       -- Ed25519 peer public key
    nickname      TEXT NOT NULL,          -- peer nickname
    node_id       BLOB,                  -- iroh NodeId for reconnect
    added_at      INTEGER NOT NULL,      -- unix timestamp
    last_seen     INTEGER                -- last online
);

-- Chats (1-to-1 for MVP)
CREATE TABLE chats (
    id            TEXT PRIMARY KEY,       -- UUID
    peer_key      BLOB NOT NULL UNIQUE,   -- FK → contacts.public_key
    created_at    INTEGER NOT NULL,
    last_message  TEXT,                   -- FK → messages.id (for preview)
    unread_count  INTEGER DEFAULT 0
);

-- Messages (stored decrypted locally; ciphertext exists only on the wire)
CREATE TABLE messages (
    id            TEXT PRIMARY KEY,       -- UUID (generated by sender)
    chat_id       TEXT NOT NULL,          -- FK → chats.id
    sender_key    BLOB NOT NULL,          -- who sent it
    content       TEXT NOT NULL,          -- plaintext (decrypted locally)
    seq           INTEGER NOT NULL,       -- per-chat sequence number (for sync ordering)
    status        TEXT NOT NULL           -- pending | sent | delivered | read
                  CHECK (status IN ('pending','sent','delivered','read')),
    created_at    INTEGER NOT NULL,       -- send timestamp
    received_at   INTEGER                 -- receive timestamp
);

-- Pending invite codes
CREATE TABLE pending_invites (
    code          TEXT PRIMARY KEY,       -- invite code
    created_at    INTEGER NOT NULL,
    expires_at    INTEGER                 -- TTL
);

-- Indexes
CREATE INDEX idx_messages_chat ON messages(chat_id, created_at);
CREATE INDEX idx_messages_status ON messages(status) WHERE status = 'pending';
```

## Wire Protocol

### Transport

- iroh QUIC connections with ALPN: `imax/1`
- Length-prefixed framing: 4-byte length (u32) + postcard-encoded payload
- Bidirectional QUIC streams

### Message Types

```rust
enum WireMessage {
    // Handshake
    Hello {
        public_key: [u8; 32],
        nickname: String,
        protocol_version: u8,       // 1
    },

    // Chat messages
    ChatMessage {
        id: Uuid,
        ciphertext: Vec<u8>,
        nonce: [u8; 24],
        timestamp: u64,
    },

    // Acknowledgements
    Ack {
        message_id: Uuid,
        status: AckStatus,          // Delivered | Read
    },

    // Offline sync (sequence-based ordering)
    SyncRequest {
        last_seq: u64,              // last known sequence number
    },
    SyncResponse {
        messages: Vec<ChatMessage>,
        has_more: bool,             // pagination: more messages available
    },

    // Presence
    Ping,
    Pong,
}

enum AckStatus {
    Delivered,
    Read,
}
```

### Connection Flow

```
Alice                              Bob
  │                                   │
  │──── QUIC connect (ALPN=imax/1) ─▶│
  │                                   │
  │──── Hello { pubkey, nick, v=1 } ─▶│
  │◀─── Hello { pubkey, nick, v=1 } ──│
  │                                   │
  │  // shared secret = DH(alice_priv, bob_pub)
  │                                   │
  │──── SyncRequest { last_seen } ───▶│
  │◀─── SyncResponse { missed } ──────│
  │                                   │
  │◀═══ ChatMessage (encrypted) ═════▶│
  │◀═══ Ack ═════════════════════════▶│
  │                                   │
  │──── Ping ────────────────────────▶│  // every 30 sec
  │◀─── Pong ─────────────────────────│
```

## E2E Encryption

### Key Exchange

For each peer pair, a shared secret is computed:

```
shared_secret = X25519_DH(my_x25519_secret, peer_x25519_public)
    │
    ▼  HKDF-SHA256 (salt = sorted_pubkeys)
    │
symmetric_key (32 bytes)
```

### Message Encryption

```
nonce = random_24_bytes()

ciphertext = XChaCha20-Poly1305(
    key   = symmetric_key,
    nonce = nonce,
    aad   = message_id,    // associated data — tamper protection
    data  = plaintext
)
```

### Security Properties

- **Confidentiality**: XChaCha20-Poly1305 AEAD, only participants can read
- **Integrity**: Poly1305 MAC + message_id as AAD, tampering detected
- **Authentication**: Ed25519 identity tied to iroh NodeId
- **Replay protection**: UUID per message + duplicate check in SQLite
- **No forward secrecy** (MVP): Double Ratchet planned for future

### Rust Crates

```toml
bip39           = "2"        # seed phrase generation/parsing
ed25519-dalek   = "2"        # Ed25519 signatures
x25519-dalek    = "2"        # X25519 DH key exchange
chacha20poly1305 = "0.10"   # XChaCha20-Poly1305 AEAD
hkdf            = "0.12"    # HKDF key derivation
sha2            = "0.10"    # SHA-256 for HKDF
rand            = "0.8"     # nonce generation
```

## Data Flows

### Sending a Message

```
UI → ChatManager → Crypto (encrypt) → Storage (save, status=sent) → Network (send) → Ack → Storage (status=delivered)
```

### Receiving a Message

```
Network (incoming) → ChatManager → Crypto (decrypt) → Storage (save) → UI (Event::NewMessage) → Network (send ack)
```

### Offline Behavior

- Peer offline: message saved to SQLite with status `pending`
- On reconnect: automatic resend of pending messages via SyncRequest/SyncResponse
- Reconnection: iroh finds peer via DHT by NodeId (no invite needed)

## UI Design

### Layout: Two-Panel (Telegram Style)

Desktop window with:
- **Left sidebar** (320px): hamburger menu, search bar, chat list with round avatars, online dots, unread badges, message preview with "You:" prefix for outgoing
- **Right panel** (flex): chat header (avatar, name, online status, SVG action icons), message area with grouped bubbles, input bar (attach, text, emoji, send)

### Color Scheme (Telegram Dark)

- Primary background: #17212b (sidebar, headers)
- Secondary background: #0e1621 (chat area, inputs)
- Outgoing messages: #2b5278
- Incoming messages: #182533
- Accent: #6ab3f3 (links, active states)
- Online indicator: #4dcd68 (green)
- Read check marks: #4dcd68 (green), unread: rgba(255,255,255,0.45)
- Avatars: solid color circles (purple, blue, green, orange, red, teal)

### Screens

1. **Onboarding**: simple — nickname input + "Start Messaging" / "I have a seed phrase" link. No seed phrase shown here.
2. **Main Layout**: two-panel Telegram-style (sidebar + chat)
3. **New Chat Modal**: generate invite code / paste invite code
4. **Settings Modal**: profile editing, seed phrase (hidden by default, revealed on click), encryption info

### Message Statuses

- `✓` — sent
- `✓✓` — delivered
- `✓✓` (green) — read

### Icons

All icons are inline SVG (no emoji). Feather-style line icons for: menu, search, phone, attachment, emoji, send, close, lock, shield.

### Dioxus State Management

- Dioxus Signals for reactive state
- ChatManager events → Signal updates → automatic UI re-render
- `use_coroutine` for P2P message loop

## Platform Considerations

### Desktop (Primary)
- Full iroh P2P, direct connections
- Native file system access for SQLite
- No restrictions

### Web
- iroh cannot use raw QUIC from browser
- Requires relay server or WebRTC transport adapter
- SQLite via IndexedDB or in-memory with sync
- Second priority after desktop MVP

### Mobile
- Dioxus mobile support is experimental
- iroh should work on mobile (QUIC over UDP)
- Background connectivity challenges (OS restrictions)
- Third priority

## Threat Model

### Defended Against (MVP)

- **Network eavesdropping**: E2E encryption, QUIC TLS transport
- **Message tampering**: AEAD with message_id as AAD
- **Replay attacks**: UUID deduplication in SQLite
- **Identity spoofing**: iroh NodeId = Ed25519 pubkey, TLS-authenticated
- **Seed phrase extraction from DB**: encrypted at rest via OS keychain

### NOT Defended Against (MVP — Known Limitations)

- **Compromised device**: if attacker has full device access, they can read decrypted messages in SQLite
- **No forward secrecy**: static DH key — compromise of private key exposes all past/future messages. Mitigated by Double Ratchet in post-MVP
- **Metadata leakage**: iroh DHT reveals that two NodeIds are communicating (timing, frequency). Content is hidden but connection patterns are not
- **Denial of service**: no rate limiting on incoming connections or messages

## Non-Goals (MVP)

- Group chats
- File sharing
- Voice/video calls
- Multi-device sync
- Message editing/deletion
- Typing indicators
- User blocking
- Rate limiting / spam protection
- Web or mobile builds (desktop first)

## ChatManager API

Core orchestrator interface:

```rust
impl ChatManager {
    // Lifecycle
    pub async fn new(db: Database, config: Config) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;           // start iroh node, begin listening
    pub async fn shutdown(&self) -> Result<()>;

    // Identity
    pub fn create_identity(nickname: &str) -> Result<(SeedPhrase, Identity)>;
    pub fn restore_identity(seed: &SeedPhrase, nickname: &str) -> Result<Identity>;

    // Contacts & Discovery
    pub fn generate_invite(&self) -> Result<InviteCode>;
    pub async fn accept_invite(&self, code: &InviteCode) -> Result<Chat>;
    pub fn get_contacts(&self) -> Result<Vec<Contact>>;

    // Chats
    pub fn get_chats(&self) -> Result<Vec<ChatPreview>>;
    pub fn get_messages(&self, chat_id: &ChatId, limit: usize, before_seq: Option<u64>) -> Result<Vec<Message>>;

    // Messaging
    pub async fn send_message(&self, chat_id: &ChatId, text: &str) -> Result<Message>;
    pub fn mark_read(&self, chat_id: &ChatId) -> Result<()>;

    // Events (core → UI)
    pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent>;
}

enum ChatEvent {
    NewMessage { chat_id: ChatId, message: Message },
    MessageStatusChanged { message_id: MessageId, status: Status },
    PeerOnline { public_key: PublicKey },
    PeerOffline { public_key: PublicKey },
    InviteAccepted { chat: Chat },
}
```

UI subscribes via `tokio::sync::broadcast` channel. Dioxus coroutine listens to the channel and updates Signals.

## Invite Code Format

```
imax:<base58(payload)>

payload = {
    public_key: [u8; 32],       // Ed25519 public key
    node_id: [u8; 32],          // iroh NodeId
    addrs: Vec<SocketAddr>,     // direct addresses (may be empty)
    relay_url: Option<String>,  // iroh relay URL
    expires: u64,               // unix timestamp
}
```

- Base58 encoding (Bitcoin alphabet) — no ambiguous characters, copy-paste friendly
- Typical length: ~120 characters
- Prefix `imax:` for protocol identification

## Error Handling

- Core uses a unified `imax_core::Error` enum with variants per module (`NetworkError`, `CryptoError`, `StorageError`, etc.)
- All public API methods return `Result<T, Error>`
- Decryption failure on incoming message: log warning, send Nack, skip message (do not crash)
- DB corruption: surface error to UI, suggest re-import from seed phrase
- Malformed wire message: disconnect peer, log error
- UI displays errors via a toast/notification system (non-blocking)

## Future Enhancements (Post-MVP)

- Double Ratchet (forward secrecy)
- Group chats
- File sharing (iroh-blobs)
- Voice/video calls (WebRTC)
- Multi-device sync
- QR code for invite
- Message search
- Avatars/profile pictures
- Read receipts toggle
- Typing indicators
