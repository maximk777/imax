use dioxus::prelude::*;

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
