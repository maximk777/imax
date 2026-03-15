use dioxus::prelude::*;
use std::sync::OnceLock;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use iroh::EndpointAddr;

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

// ── Outgoing message channel (UI → P2P task) ──

/// Sender half of the outgoing message channel.
/// Set once by the P2P background task; used by MessageInput to enqueue messages.
pub static OUTGOING_TX: OnceLock<mpsc::UnboundedSender<OutgoingMessage>> = OnceLock::new();

// ── Peer address registry (chat_id → EndpointAddr) ──

/// Maps chat_id to the peer's EndpointAddr so the P2P task knows where to send.
pub static PEER_ADDRS: OnceLock<Mutex<HashMap<String, EndpointAddr>>> = OnceLock::new();

/// Register a peer address for a given chat_id.
pub fn register_peer_addr(chat_id: String, addr: EndpointAddr) {
    let map = PEER_ADDRS.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock().unwrap().insert(chat_id, addr);
}

/// Look up the EndpointAddr for a given chat_id.
pub fn get_peer_addr(chat_id: &str) -> Option<EndpointAddr> {
    PEER_ADDRS.get()?.lock().unwrap().get(chat_id).cloned()
}
