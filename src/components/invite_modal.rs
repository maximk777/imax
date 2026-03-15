use dioxus::prelude::*;
use dioxus::document::eval;
use crate::state::{SHOW_INVITE_MODAL, INVITE_CODE};

#[component]
pub fn InviteModal() -> Element {
    let show = *SHOW_INVITE_MODAL.read();
    let invite_code = INVITE_CODE.read().clone();
    let mut paste_input = use_signal(String::new);
    let mut copied = use_signal(|| false);

    if !show {
        return rsx! {};
    }

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| {
                *SHOW_INVITE_MODAL.write() = false;
            },
            div {
                class: "modal",
                onclick: move |evt| evt.stop_propagation(),

                div { class: "modal-title", "New Chat" }

                // Your invite code
                div { class: "modal-section",
                    p { class: "modal-label", "Your invite code" }
                    div { class: "modal-code-box",
                        code { class: "modal-code", "{invite_code}" }
                    }
                    button {
                        class: "modal-btn-secondary",
                        onclick: move |_| {
                            // Copy to clipboard via eval
                            let code = INVITE_CODE.read().clone();
                            eval(&format!(r#"navigator.clipboard.writeText("{code}")"#));
                            *copied.write() = true;
                        },
                        if *copied.read() { "Copied!" } else { "Copy code" }
                    }
                }

                // Divider
                div { class: "modal-divider", "or" }

                // Paste their code
                div { class: "modal-section",
                    p { class: "modal-label", "Paste invite code" }
                    input {
                        class: "modal-input",
                        r#type: "text",
                        placeholder: "imax:...",
                        value: "{paste_input}",
                        oninput: move |evt| {
                            *paste_input.write() = evt.value();
                        },
                    }
                    button {
                        class: "modal-btn-primary",
                        disabled: paste_input.read().is_empty(),
                        onclick: move |_| {
                            let code = paste_input.read().clone();
                            println!("[imax] Connecting to peer: {code}");
                            // TODO: accept_invite flow via ChatManager
                            *SHOW_INVITE_MODAL.write() = false;
                        },
                        "Connect"
                    }
                }
            }
        }
    }
}
