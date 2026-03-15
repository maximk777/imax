use dioxus::prelude::*;
use std::sync::{Arc, OnceLock, LazyLock};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use iroh::{EndpointId, SecretKey};
use imax_core::network::node::IrohNode;
use imax_core::network::discovery::{InviteCode, InvitePayload};
use imax_core::storage::Database;
use imax_core::storage::models;
use crate::components::test_p2p::start_message_loop;

// ── UI update channel (tokio task → Dioxus runtime) ──

/// Events sent from the P2P background task to the Dioxus UI coroutine.
#[derive(Debug, Clone)]
pub enum UiUpdate {
    PeerConnected {
        chat_id: String,
        peer_name: String,
        public_key_byte: u8,
    },
    MessageReceived {
        chat_id: String,
        message: Message,
    },
    ChatPreviewUpdate {
        chat_id: String,
        last_message: String,
    },
    MessageStatusUpdate {
        message_id: String,
        status: String,
    },
}

pub static UI_UPDATE_TX: OnceLock<mpsc::UnboundedSender<UiUpdate>> = OnceLock::new();
pub static UI_UPDATE_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<UiUpdate>>>> = OnceLock::new();

/// A chat preview shown in the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatPreview {
    pub id: String,
    pub peer_name: String,
    pub last_message: String,
    pub time: String,
    pub avatar_color: usize,
}

/// A single message in the active conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub id: String,
    pub content: String,
    pub is_mine: bool,
    pub time: String,
    pub status: String,
}

// ── SQLite database ──

pub static DB: OnceLock<Mutex<Database>> = OnceLock::new();

/// Open the SQLite database at ~/.imax/app.db and store in DB.
pub fn init_db() {
    let mut p = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push(".imax");
    let _ = std::fs::create_dir_all(&p);
    p.push("app.db");
    let db = Database::open(p.to_str().unwrap()).expect("Failed to open database");
    let _ = DB.set(Mutex::new(db));
}

pub fn db() -> MutexGuard<'static, Database> {
    DB.get().expect("DB not initialized").lock().unwrap()
}

/// An outgoing message queued by the UI and consumed by the P2P background task.
#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub chat_id: String,
    pub text: String,
}

// ── Global signals ──

pub static IS_ONBOARDED: GlobalSignal<bool> = Signal::global(|| false);
pub static NICKNAME: GlobalSignal<String> = Signal::global(|| String::new());
pub static SEED_PHRASE: GlobalSignal<String> = Signal::global(|| String::new());
pub static ACTIVE_CHAT_ID: GlobalSignal<Option<String>> = Signal::global(|| None);
pub static CHATS: GlobalSignal<Vec<ChatPreview>> = Signal::global(Vec::new);
pub static MESSAGES: GlobalSignal<Vec<Message>> = Signal::global(Vec::new);
/// All messages indexed by chat_id — the single source of truth.
pub static ALL_MESSAGES: GlobalSignal<HashMap<String, Vec<Message>>> = Signal::global(HashMap::new);
pub static INVITE_CODE: GlobalSignal<String> = Signal::global(|| String::new());
pub static SHOW_INVITE_MODAL: GlobalSignal<bool> = Signal::global(|| false);
pub static SHOW_SETTINGS_MODAL: GlobalSignal<bool> = Signal::global(|| false);

// ── Multi-profile signals ──
pub static ACTIVE_PROFILE_ID: GlobalSignal<i64> = Signal::global(|| 0);
pub static ADDING_PROFILE: GlobalSignal<bool> = Signal::global(|| false);

// ── P2P network state ──

/// Raw Ed25519 signing key bytes — used to create iroh SecretKey and ChatManager.
pub static SIGNING_KEY_BYTES: GlobalSignal<[u8; 32]> = Signal::global(|| [0u8; 32]);
/// Our Ed25519 verifying (public) key bytes — sent in Hello messages.
pub static MY_PUBKEY_BYTES: GlobalSignal<[u8; 32]> = Signal::global(|| [0u8; 32]);
/// Whether the iroh node has started and is online.
pub static NODE_STARTED: GlobalSignal<bool> = Signal::global(|| false);
/// Human-readable connection status: "offline", "connecting", "online", or "error: …"
pub static CONNECTION_STATUS: GlobalSignal<String> = Signal::global(|| "offline".to_string());

// ── Resettable globals (LazyLock<Mutex<Option<...>>>) ──

pub static IROH_NODE: LazyLock<Mutex<Option<Arc<IrohNode>>>> = LazyLock::new(|| Mutex::new(None));
pub static OUTGOING_TX: LazyLock<Mutex<Option<mpsc::UnboundedSender<OutgoingMessage>>>> = LazyLock::new(|| Mutex::new(None));
pub static PEER_IDS: LazyLock<Mutex<HashMap<String, EndpointId>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
pub static NODE_CANCEL: LazyLock<Mutex<CancellationToken>> = LazyLock::new(|| Mutex::new(CancellationToken::new()));

/// Peer Ed25519 public keys, keyed by chat_id.
pub static PEER_PUBKEYS: LazyLock<Mutex<HashMap<String, [u8; 32]>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Cached symmetric keys derived via DH, keyed by chat_id.
pub static SYM_KEYS: LazyLock<Mutex<HashMap<String, [u8; 32]>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get a clone of the current iroh node (if running).
pub fn get_iroh_node() -> Option<Arc<IrohNode>> {
    IROH_NODE.lock().unwrap().clone()
}

/// Get a clone of the outgoing message sender (if running).
pub fn get_outgoing_tx() -> Option<mpsc::UnboundedSender<OutgoingMessage>> {
    OUTGOING_TX.lock().unwrap().clone()
}

/// Register a peer's EndpointId for a given chat_id.
pub fn register_peer(chat_id: String, peer_id: EndpointId) {
    PEER_IDS.lock().unwrap().insert(chat_id, peer_id);
}

/// Look up the EndpointId for a given chat_id.
pub fn get_peer_id(chat_id: &str) -> Option<EndpointId> {
    PEER_IDS.lock().unwrap().get(chat_id).cloned()
}

/// Store a peer's Ed25519 public key for a given chat_id.
pub fn register_peer_pubkey(chat_id: &str, pubkey: [u8; 32]) {
    PEER_PUBKEYS.lock().unwrap().insert(chat_id.to_string(), pubkey);
}

/// Look up a peer's Ed25519 public key for a given chat_id.
pub fn get_peer_pubkey(chat_id: &str) -> Option<[u8; 32]> {
    PEER_PUBKEYS.lock().unwrap().get(chat_id).copied()
}

/// Get or derive the symmetric key for a chat via X25519 DH + HKDF.
pub fn get_or_derive_sym_key(chat_id: &str, sk_bytes: &[u8; 32], peer_pubkey: &[u8; 32]) -> Option<[u8; 32]> {
    let mut keys = SYM_KEYS.lock().unwrap();
    if let Some(key) = keys.get(chat_id) {
        return Some(*key);
    }
    let signing_key = ed25519_dalek::SigningKey::from_bytes(sk_bytes);
    let my_pubkey = signing_key.verifying_key().to_bytes();
    let x25519_secret = imax_core::identity::keypair::to_x25519_secret(&signing_key);
    let peer_x25519 = imax_core::identity::keypair::x25519_public_from_bytes(peer_pubkey).ok()?;
    let shared = x25519_secret.diffie_hellman(&peer_x25519);
    let sym_key = imax_core::crypto::e2e::derive_symmetric_key(shared.as_bytes(), &my_pubkey, peer_pubkey);
    keys.insert(chat_id.to_string(), sym_key);
    Some(sym_key)
}

/// Hex-encode a byte slice (e.g. `&[0xab, 0xcd]` → `"abcd"`).
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Initialize the UI update channel. Call once at startup.
pub fn init_ui_channel() {
    let (tx, rx) = mpsc::unbounded_channel::<UiUpdate>();
    let _ = UI_UPDATE_TX.set(tx);
    let _ = UI_UPDATE_RX.set(Mutex::new(Some(rx)));
}

/// Shut down the current P2P node and clear all node-related state.
pub fn shutdown_node() {
    // 1. Cancel message loop
    NODE_CANCEL.lock().unwrap().cancel();
    // 2. Close outgoing channel
    *OUTGOING_TX.lock().unwrap() = None;
    // 3. Shutdown iroh node (closes QUIC connections, unblocks accept loops)
    if let Some(node) = IROH_NODE.lock().unwrap().take() {
        tokio::spawn(async move {
            node.shutdown().await.ok();
        });
    }
    // 4. Clear peers and crypto state
    PEER_IDS.lock().unwrap().clear();
    PEER_PUBKEYS.lock().unwrap().clear();
    SYM_KEYS.lock().unwrap().clear();
    // 5. Reset signals
    *NODE_STARTED.write() = false;
    *CONNECTION_STATUS.write() = "offline".into();
    *INVITE_CODE.write() = String::new();
    // 6. Fresh cancel token
    *NODE_CANCEL.lock().unwrap() = CancellationToken::new();
}

/// Switch to a different profile: shut down node, load profile data, auto-restart.
pub fn switch_profile(profile_id: i64) {
    shutdown_node();
    // Mark active in DB
    {
        let db = db();
        models::set_active_profile(&db, profile_id).unwrap();
    }
    // Load profile data into signals
    load_profile(profile_id);
}

/// Load a profile's data into signals (chats, messages, identity info).
fn load_profile(profile_id: i64) {
    let db_guard = db();
    if let Ok(Some(p)) = models::get_profile(&db_guard, profile_id) {
        *SEED_PHRASE.write() = p.seed_phrase;
        *NICKNAME.write() = p.nickname;
        *ACTIVE_PROFILE_ID.write() = profile_id;
        *IS_ONBOARDED.write() = true;
    }

    // Load chats for this profile
    let mut chats_vec = Vec::new();
    let mut all_msgs: HashMap<String, Vec<Message>> = HashMap::new();
    if let Ok(chat_rows) = models::get_all_chats(&db_guard, profile_id) {
        for r in &chat_rows {
            if let Ok(msg_rows) = models::get_messages_for_chat(&db_guard, &r.id, profile_id) {
                let msgs: Vec<Message> = msg_rows
                    .into_iter()
                    .map(|m| Message {
                        id: m.id,
                        content: m.content,
                        is_mine: m.is_mine,
                        time: m.time,
                        status: m.status,
                    })
                    .collect();
                if !msgs.is_empty() {
                    all_msgs.insert(r.id.clone(), msgs);
                }
            }
        }
        chats_vec = chat_rows
            .into_iter()
            .map(|r| ChatPreview {
                id: r.id,
                peer_name: r.peer_name,
                last_message: r.last_message,
                time: r.time,
                avatar_color: r.avatar_color as usize,
            })
            .collect();
    }
    drop(db_guard);

    *CHATS.write() = chats_vec;
    *ALL_MESSAGES.write() = all_msgs;
    *ACTIVE_CHAT_ID.write() = None;
    *MESSAGES.write() = vec![];
}

/// Load persisted state from SQLite and restore it into GlobalSignals.
pub fn load_and_restore() {
    let db_guard = db();

    // Restore active profile
    if let Ok(Some(profile)) = models::get_active_profile(&db_guard) {
        if !profile.seed_phrase.is_empty() {
            println!("[imax] Restoring persisted state (nickname: {})", profile.nickname);
            *SEED_PHRASE.write() = profile.seed_phrase;
            *NICKNAME.write() = profile.nickname;
            *ACTIVE_PROFILE_ID.write() = profile.id;
            *IS_ONBOARDED.write() = true;

            // Restore chats
            if let Ok(chat_rows) = models::get_all_chats(&db_guard, profile.id) {
                let chats: Vec<ChatPreview> = chat_rows
                    .into_iter()
                    .map(|r| ChatPreview {
                        id: r.id,
                        peer_name: r.peer_name,
                        last_message: r.last_message,
                        time: r.time,
                        avatar_color: r.avatar_color as usize,
                    })
                    .collect();

                // Restore messages for each chat
                let mut all_msgs: HashMap<String, Vec<Message>> = HashMap::new();
                for chat in &chats {
                    if let Ok(msg_rows) = models::get_messages_for_chat(&db_guard, &chat.id, profile.id) {
                        let msgs: Vec<Message> = msg_rows
                            .into_iter()
                            .map(|r| Message {
                                id: r.id,
                                content: r.content,
                                is_mine: r.is_mine,
                                time: r.time,
                                status: r.status,
                            })
                            .collect();
                        if !msgs.is_empty() {
                            all_msgs.insert(chat.id.clone(), msgs);
                        }
                    }
                }

                *CHATS.write() = chats;
                *ALL_MESSAGES.write() = all_msgs;
            }
        }
    }
}

/// Shared startup logic: takes signing key bytes, pubkey bytes, seed phrase string,
/// and nickname, then spawns the iroh node and message loop.
pub fn start_node(sk_bytes: [u8; 32], pubkey_bytes: [u8; 32], seed_phrase: String, name: String) {
    let status = CONNECTION_STATUS.read().clone();
    if status == "connecting" || *NODE_STARTED.read() {
        println!("[imax] start_node skipped (status={status})");
        return;
    }

    *SEED_PHRASE.write() = seed_phrase.clone();
    *SIGNING_KEY_BYTES.write() = sk_bytes;
    *MY_PUBKEY_BYTES.write() = pubkey_bytes;
    *NICKNAME.write() = name.clone();
    *CONNECTION_STATUS.write() = "connecting".to_string();
    *IS_ONBOARDED.write() = true;

    let cancel = NODE_CANCEL.lock().unwrap().clone();

    spawn(async move {
        println!("[imax] Starting iroh node...");
        let iroh_key = SecretKey::from_bytes(&sk_bytes);
        match IrohNode::new(iroh_key).await {
            Ok(new_node) => {
                println!("[imax] Node created, waiting for relay...");

                let online_result = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    new_node.endpoint().online()
                ).await;

                match online_result {
                    Ok(_) => println!("[imax] Node connected to relay!"),
                    Err(_) => println!("[imax] Relay timeout (15s), proceeding anyway"),
                }

                // Generate real invite code
                let addr = new_node.endpoint().addr();
                let node_id = new_node.node_id();
                let addrs: Vec<std::net::SocketAddr> =
                    addr.ip_addrs().cloned().collect();
                let relay_url = addr.relay_urls().next().map(|u| u.to_string());

                println!("[imax] Node ID: {:?}", node_id);
                println!("[imax] Direct addrs: {:?}", addrs);
                println!("[imax] Relay URL: {:?}", relay_url);

                let expires = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() + 86400;

                let payload = InvitePayload {
                    public_key: pubkey_bytes,
                    node_id: *node_id.as_bytes(),
                    addrs,
                    relay_url,
                    expires,
                };

                match InviteCode::encode(&payload) {
                    Ok(code) => {
                        println!("[imax] Invite code generated ({} chars)", code.0.len());
                        *INVITE_CODE.write() = code.0;
                    }
                    Err(e) => println!("[imax] Invite encode error: {e}"),
                }

                *CONNECTION_STATUS.write() = "online".to_string();
                *NODE_STARTED.write() = true;

                // Store the node globally and start the shared message loop
                let node = Arc::new(new_node);
                *IROH_NODE.lock().unwrap() = Some(node.clone());
                start_message_loop(node, sk_bytes, pubkey_bytes, name.clone(), cancel);
            }
            Err(e) => {
                println!("[imax] Failed to start node: {e}");
                *CONNECTION_STATUS.write() = format!("error: {e}");
            }
        }
    });
}

/// Append a message to the per-chat store, update MESSAGES if active, and persist to SQLite.
pub fn add_message(chat_id: &str, msg: Message) {
    let profile_id = *ACTIVE_PROFILE_ID.read();
    // Persist to SQLite
    {
        let db = db();
        if let Err(e) = models::insert_message(
            &db, &msg.id, chat_id, profile_id, &msg.content, msg.is_mine, &msg.time, &msg.status,
        ) {
            eprintln!("[imax] Failed to persist message: {e}");
        }
    }

    ALL_MESSAGES.write().entry(chat_id.to_string()).or_default().push(msg);
    // If this chat is the one the user is looking at, refresh the view signal.
    let active = ACTIVE_CHAT_ID.read().clone();
    if active.as_deref() == Some(chat_id) {
        let msgs = ALL_MESSAGES.read().get(chat_id).cloned().unwrap_or_default();
        *MESSAGES.write() = msgs;
    }
}

/// Upsert a chat preview to SQLite.
pub fn db_upsert_chat(chat: &ChatPreview) {
    let profile_id = *ACTIVE_PROFILE_ID.read();
    let db = db();
    if let Err(e) = models::upsert_chat(
        &db, &chat.id, profile_id, &chat.peer_name, &chat.last_message, &chat.time, chat.avatar_color as i32,
    ) {
        eprintln!("[imax] Failed to upsert chat: {e}");
    }
}

/// Update a message's status in memory and SQLite.
pub fn update_message_status(message_id: &str, new_status: &str) {
    // Update in ALL_MESSAGES
    let mut all = ALL_MESSAGES.write();
    for msgs in all.values_mut() {
        if let Some(m) = msgs.iter_mut().find(|m| m.id == message_id) {
            m.status = new_status.to_string();
            break;
        }
    }
    drop(all);

    // Update MESSAGES if active chat contains this message
    let active = ACTIVE_CHAT_ID.read().clone();
    if let Some(chat_id) = active {
        let msgs = ALL_MESSAGES.read().get(&chat_id).cloned().unwrap_or_default();
        if msgs.iter().any(|m| m.id == message_id) {
            *MESSAGES.write() = msgs;
        }
    }

    // Persist to SQLite
    {
        let db = db();
        if let Err(e) = models::update_message_status(&db, message_id, new_status) {
            eprintln!("[imax] Failed to update message status: {e}");
        }
    }
}

/// Update chat preview's last_message in SQLite.
pub fn db_update_chat_preview(id: &str, last_message: &str) {
    let profile_id = *ACTIVE_PROFILE_ID.read();
    let db = db();
    if let Err(e) = models::update_chat_preview(&db, id, profile_id, last_message) {
        eprintln!("[imax] Failed to update chat preview: {e}");
    }
}
