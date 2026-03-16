use dioxus::prelude::*;
use crate::state::{
    IS_ONBOARDED, CHATS, ChatPreview, UiUpdate, UI_UPDATE_RX,
    NODE_STARTED, SEED_PHRASE, NICKNAME, SIGNING_KEY_BYTES,
    add_message, db_upsert_chat, db_update_chat_preview, start_node,
    update_message_status, load_and_restore,
};
use crate::components::test_p2p::local_time_now;
use crate::views::onboarding::Onboarding;
use crate::views::main_layout::MainLayout;

const CSS: &str = include_str!("../assets/main.css");

#[component]
pub fn App() -> Element {
    // Restore persisted state from SQLite (runs once, inside the Dioxus runtime)
    use_hook(|| load_and_restore());

    // Coroutine that drains UiUpdate events from the P2P background task
    // and applies them to Dioxus GlobalSignals (which require the Dioxus runtime).
    use_coroutine(|_: UnboundedReceiver<()>| async move {
        let rx = {
            let guard = UI_UPDATE_RX.get().expect("UI_UPDATE_RX not initialized");
            guard.lock().unwrap().take().expect("UI_UPDATE_RX already taken")
        };
        let mut rx = rx;
        while let Some(update) = rx.recv().await {
            match update {
                UiUpdate::PeerConnected { chat_id, peer_name, public_key_byte, peer_node_id, peer_pubkey } => {
                    let mut chats = CHATS.write();
                    if let Some(c) = chats.iter_mut().find(|c| c.id == chat_id) {
                        // Update name if we got a better one
                        if !peer_name.is_empty() && (c.peer_name.is_empty() || c.peer_name == "Unknown" || c.peer_name.starts_with("chat-") || c.peer_name.starts_with("Peer ")) {
                            c.peer_name = peer_name;
                            c.avatar_color = (public_key_byte as usize) % 4;
                        }
                        // Update peer info if not set yet
                        if c.peer_node_id.is_none() {
                            c.peer_node_id = Some(peer_node_id);
                        }
                        if c.peer_pubkey.is_none() {
                            c.peer_pubkey = Some(peer_pubkey);
                        }
                    } else {
                        chats.push(ChatPreview {
                            id: chat_id.clone(),
                            peer_name: peer_name.clone(),
                            last_message: "Connected!".into(),
                            time: local_time_now(),
                            avatar_color: (public_key_byte as usize) % 4,
                            peer_node_id: Some(peer_node_id),
                            peer_pubkey: Some(peer_pubkey),
                        });
                    }
                    // Find the chat we just updated/added and persist it
                    if let Some(c) = chats.iter().find(|c| c.id == chat_id) {
                        let chat_copy = c.clone();
                        drop(chats);
                        db_upsert_chat(&chat_copy);
                    } else {
                        drop(chats);
                    }
                }
                UiUpdate::MessageReceived { chat_id, message } => {
                    add_message(&chat_id, message);
                }
                UiUpdate::ChatPreviewUpdate { chat_id, last_message } => {
                    let mut chats = CHATS.write();
                    if let Some(c) = chats.iter_mut().find(|c| c.id == chat_id) {
                        c.last_message = last_message.clone();
                    }
                    drop(chats);
                    db_update_chat_preview(&chat_id, &last_message);
                }
                UiUpdate::MessageStatusUpdate { message_id, status } => {
                    update_message_status(&message_id, &status);
                }
            }
        }
    });

    // Auto-start the iroh node when restored from persisted state
    use_effect(move || {
        let onboarded = *IS_ONBOARDED.read();
        let node_started = *NODE_STARTED.read();
        let seed = SEED_PHRASE.read().clone();
        if onboarded && !node_started && !seed.is_empty() {
            let nickname = NICKNAME.read().clone();
            // Derive pubkey from seed phrase
            if let Ok(mnemonic) = imax_core::identity::parse_mnemonic(&seed) {
                let signing_key = imax_core::identity::derive_signing_key(&mnemonic);
                let pubkey_bytes = signing_key.verifying_key().to_bytes();
                let sk = signing_key.to_bytes();
                *SIGNING_KEY_BYTES.write() = sk;
                start_node(sk, pubkey_bytes, seed, nickname);
            }
        }
    });

    let onboarded = IS_ONBOARDED.read();

    rsx! {
        style { {CSS} }
        if *onboarded {
            MainLayout {}
        } else {
            Onboarding {}
        }
    }
}
