use dioxus::prelude::*;
use std::sync::{Arc, OnceLock};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use iroh::EndpointId;
use imax_core::network::node::IrohNode;

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

// ── P2P network state ──

/// Raw Ed25519 signing key bytes — used to create iroh SecretKey and ChatManager.
pub static SIGNING_KEY_BYTES: GlobalSignal<[u8; 32]> = Signal::global(|| [0u8; 32]);
/// Whether the iroh node has started and is online.
pub static NODE_STARTED: GlobalSignal<bool> = Signal::global(|| false);
/// Human-readable connection status: "offline", "connecting", "online", or "error: …"
pub static CONNECTION_STATUS: GlobalSignal<String> = Signal::global(|| "offline".to_string());

// ── Global iroh node (single shared instance) ──

pub static IROH_NODE: OnceLock<Arc<IrohNode>> = OnceLock::new();

// ── Outgoing message channel (UI → P2P task) ──

/// Sender half of the outgoing message channel.
/// Set once by the P2P background task; used by MessageInput to enqueue messages.
pub static OUTGOING_TX: OnceLock<mpsc::UnboundedSender<OutgoingMessage>> = OnceLock::new();

// ── Peer ID registry (chat_id → EndpointId) ──

/// Maps chat_id to the peer's EndpointId so the P2P task knows where to send.
/// iroh resolves transport addresses from its connection cache.
pub static PEER_IDS: OnceLock<Mutex<HashMap<String, EndpointId>>> = OnceLock::new();

/// Register a peer's EndpointId for a given chat_id.
pub fn register_peer(chat_id: String, peer_id: EndpointId) {
    let map = PEER_IDS.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock().unwrap().insert(chat_id, peer_id);
}

/// Look up the EndpointId for a given chat_id.
pub fn get_peer_id(chat_id: &str) -> Option<EndpointId> {
    PEER_IDS.get()?.lock().unwrap().get(chat_id).cloned()
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

/// Append a message to the per-chat store and, if this chat is currently active, also update MESSAGES for the UI.
pub fn add_message(chat_id: &str, msg: Message) {
    ALL_MESSAGES.write().entry(chat_id.to_string()).or_default().push(msg);
    // If this chat is the one the user is looking at, refresh the view signal.
    let active = ACTIVE_CHAT_ID.read().clone();
    if active.as_deref() == Some(chat_id) {
        let msgs = ALL_MESSAGES.read().get(chat_id).cloned().unwrap_or_default();
        *MESSAGES.write() = msgs;
    }
}
