use dioxus::prelude::*;
use crate::state::{ACTIVE_CHAT_ID, MESSAGES, Message, OUTGOING_TX, OutgoingMessage};

#[component]
pub fn MessageInput() -> Element {
    let mut draft = use_signal(String::new);

    let send = move |_| {
        let text = draft.read().trim().to_string();
        if text.is_empty() {
            return;
        }

        // Append the new message to the global messages list.
        let active = ACTIVE_CHAT_ID.read().clone();
        if let Some(ref chat_id) = active {
            let msg = Message {
                id: uuid(),
                content: text.clone(),
                is_mine: true,
                time: "now".into(),
                status: "sent".into(),
            };
            MESSAGES.write().push(msg);

            // Enqueue for P2P delivery
            if let Some(tx) = OUTGOING_TX.get() {
                let _ = tx.send(OutgoingMessage {
                    chat_id: chat_id.clone(),
                    text: text.clone(),
                });
            }
        }

        println!("[imax] send: {text}");
        *draft.write() = String::new();
    };

    let on_keydown = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter {
            let text = draft.read().trim().to_string();
            if text.is_empty() {
                return;
            }
            let active = ACTIVE_CHAT_ID.read().clone();
            if let Some(ref chat_id) = active {
                let msg = Message {
                    id: uuid(),
                    content: text.clone(),
                    is_mine: true,
                    time: "now".into(),
                    status: "sent".into(),
                };
                MESSAGES.write().push(msg);

                // Enqueue for P2P delivery
                if let Some(tx) = OUTGOING_TX.get() {
                    let _ = tx.send(OutgoingMessage {
                        chat_id: chat_id.clone(),
                        text: text.clone(),
                    });
                }
            }
            println!("[imax] send: {text}");
            *draft.write() = String::new();
        }
    };

    rsx! {
        div { class: "message-input-bar",
            input {
                r#type: "text",
                placeholder: "Write a message…",
                value: "{draft}",
                oninput: move |evt| *draft.write() = evt.value(),
                onkeydown: on_keydown,
            }
            button {
                class: "send-button",
                onclick: send,
                "➤"
            }
        }
    }
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("msg-{t}")
}
