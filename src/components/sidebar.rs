use dioxus::prelude::*;
use crate::state::{ACTIVE_CHAT_ID, CHATS, MESSAGES, ALL_MESSAGES, NICKNAME, SHOW_INVITE_MODAL, SHOW_SETTINGS_MODAL, CONNECTION_STATUS};
use crate::components::test_p2p::run_test_p2p;

const AVATAR_COLORS: [&str; 4] = ["blue", "green", "purple", "orange"];

#[component]
pub fn Sidebar() -> Element {
    let chats = CHATS.read().clone();
    let nickname = NICKNAME.read().clone();
    let status = CONNECTION_STATUS.read().clone();
    let mut test_status = use_signal(|| String::new());

    // Map status string to a CSS modifier class and display label.
    let (status_class, status_label) = match status.as_str() {
        "online" => ("status-dot online", "online"),
        "connecting" => ("status-dot connecting", "connecting"),
        s if s.starts_with("connected") => ("status-dot online", "connected"),
        s if s.starts_with("error") => ("status-dot error", "error"),
        _ => ("status-dot offline", "offline"),
    };

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

            // Connection status footer
            div { class: "sidebar-status-bar",
                span { class: "{status_class}" }
                span { class: "sidebar-status-label", "{status_label}" }
            }

            // Test P2P button
            button {
                class: "sidebar-test-btn",
                onclick: move |_| {
                    test_status.set("Creating test peer...".into());
                    spawn(async move {
                        match run_test_p2p().await {
                            Ok(_) => test_status.set("Test peer connected!".into()),
                            Err(e) => test_status.set(format!("Test failed: {e}")),
                        }
                    });
                },
                "Test P2P"
            }
            {
                let ts = test_status.read().clone();
                if !ts.is_empty() {
                    rsx! {
                        div { class: "sidebar-test-status", "{ts}" }
                    }
                } else {
                    rsx! {}
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
                                    // Load messages for this chat from the persistent store
                                    let msgs = ALL_MESSAGES.read().get(&chat_id2).cloned().unwrap_or_default();
                                    *MESSAGES.write() = msgs;
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
