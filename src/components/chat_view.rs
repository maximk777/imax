use dioxus::prelude::*;
use crate::state::{ACTIVE_CHAT_ID, MESSAGES};
use crate::components::chat_header::ChatHeader;
use crate::components::message_bubble::MessageBubble;
use crate::components::message_input::MessageInput;

#[component]
pub fn ChatView() -> Element {
    let active_id = ACTIVE_CHAT_ID.read().clone();

    if active_id.is_none() {
        return rsx! {
            div { class: "chat-view",
                div { class: "chat-placeholder",
                    div { class: "chat-placeholder-icon", "💬" }
                    div { class: "chat-placeholder-text", "Select a chat to start messaging" }
                }
            }
        };
    }

    let messages = MESSAGES.read().clone();

    rsx! {
        div { class: "chat-view",
            ChatHeader {}
            div { class: "message-list",
                for msg in messages {
                    MessageBubble { key: "{msg.id}", message: msg }
                }
            }
            MessageInput {}
        }
    }
}
