use dioxus::prelude::*;
use dioxus::document::eval;
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
    let msg_count = messages.len();

    // Auto-scroll to bottom whenever message count changes
    use_effect(move || {
        let _ = msg_count;
        eval(r#"
            const el = document.querySelector('.message-list');
            if (el) el.scrollTop = el.scrollHeight;
        "#);
    });

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
