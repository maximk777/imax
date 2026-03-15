use dioxus::prelude::*;
use crate::state::{
    IS_ONBOARDED, CHATS, ChatPreview, UiUpdate, UI_UPDATE_RX,
    add_message,
};
use crate::views::onboarding::Onboarding;
use crate::views::main_layout::MainLayout;

const CSS: &str = include_str!("../assets/main.css");

#[component]
pub fn App() -> Element {
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
                UiUpdate::PeerConnected { chat_id, peer_name, public_key_byte } => {
                    let exists = CHATS.read().iter().any(|c| c.id == chat_id);
                    if !exists {
                        CHATS.write().push(ChatPreview {
                            id: chat_id,
                            peer_name,
                            last_message: "Connected!".into(),
                            time: "now".into(),
                            avatar_color: (public_key_byte as usize) % 4,
                        });
                    }
                }
                UiUpdate::MessageReceived { chat_id, message } => {
                    add_message(&chat_id, message);
                }
                UiUpdate::ChatPreviewUpdate { chat_id, last_message } => {
                    let mut chats = CHATS.write();
                    if let Some(c) = chats.iter_mut().find(|c| c.id == chat_id) {
                        c.last_message = last_message;
                    }
                }
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
