use dioxus::prelude::*;
use crate::state::{ACTIVE_CHAT_ID, CHATS};

#[component]
pub fn ChatHeader() -> Element {
    let active_id = ACTIVE_CHAT_ID.read().clone();
    let chats = CHATS.read();

    let chat = active_id.as_deref().and_then(|id| {
        chats.iter().find(|c| c.id == id)
    });

    match chat {
        None => rsx! {
            div { class: "chat-header",
                span { style: "color: #6b7a8d; font-size: 14px;", "No chat selected" }
            }
        },
        Some(c) => {
            let first = c.peer_name.chars().next().unwrap_or('?').to_uppercase().to_string();
            let name = c.peer_name.clone();
            rsx! {
                div { class: "chat-header",
                    div { class: "chat-header-avatar", "{first}" }
                    div { class: "chat-header-info",
                        div { class: "chat-header-name", "{name}" }
                        div { class: "chat-header-status",
                            div { class: "status-dot" }
                            span { "online" }
                        }
                    }
                    div { class: "e2e-badge", "E2E" }
                }
            }
        }
    }
}
