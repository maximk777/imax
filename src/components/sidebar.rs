use dioxus::prelude::*;
use crate::state::{ACTIVE_CHAT_ID, CHATS, MESSAGES, NICKNAME, SHOW_INVITE_MODAL, SHOW_SETTINGS_MODAL};

const AVATAR_COLORS: [&str; 4] = ["blue", "green", "purple", "orange"];

#[component]
pub fn Sidebar() -> Element {
    let chats = CHATS.read().clone();
    let nickname = NICKNAME.read().clone();

    rsx! {
        div { class: "sidebar",
            // Header
            div { class: "sidebar-header",
                div { class: "sidebar-header-left",
                    span { class: "sidebar-brand", "iMax" }
                    if !nickname.is_empty() {
                        span { class: "sidebar-nick", "· {nickname}" }
                    }
                }
                div { class: "sidebar-header-actions",
                    button {
                        class: "sidebar-btn",
                        title: "New Chat",
                        onclick: move |_| {
                            *SHOW_INVITE_MODAL.write() = true;
                        },
                        "+"
                    }
                    button {
                        class: "sidebar-btn",
                        title: "Settings",
                        onclick: move |_| {
                            *SHOW_SETTINGS_MODAL.write() = true;
                        },
                        "\u{2699}"
                    }
                }
            }

            // Chat list
            div { class: "chat-list",
                if chats.is_empty() {
                    div { class: "chat-list-empty",
                        p { "No chats yet" }
                        p { class: "chat-list-empty-hint", "Tap + to start a new conversation" }
                    }
                }
                for chat in chats {
                    {
                        let chat_id = chat.id.clone();
                        let chat_id2 = chat.id.clone();
                        let is_active = *ACTIVE_CHAT_ID.read() == Some(chat.id.clone());
                        let item_class = if is_active { "chat-item active" } else { "chat-item" };
                        let avatar_color = AVATAR_COLORS[chat.avatar_color % 4];
                        let first = chat.peer_name.chars().next().unwrap_or('?').to_uppercase().to_string();

                        rsx! {
                            div {
                                key: "{chat_id}",
                                class: "{item_class}",
                                onclick: move |_| {
                                    *ACTIVE_CHAT_ID.write() = Some(chat_id2.clone());
                                    // Messages would be loaded from ChatManager in real flow
                                    *MESSAGES.write() = vec![];
                                },
                                div { class: "chat-avatar color-{avatar_color}", "{first}" }
                                div { class: "chat-item-info",
                                    div { class: "chat-item-top",
                                        span { class: "chat-item-name", "{chat.peer_name}" }
                                        span { class: "chat-item-time", "{chat.time}" }
                                    }
                                    div { class: "chat-item-preview", "{chat.last_message}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
