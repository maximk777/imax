use dioxus::prelude::*;
use dioxus::document::eval;
use iroh::{SecretKey, EndpointId, EndpointAddr, TransportAddr, RelayUrl};
use imax_core::network::node::IrohNode;
use imax_core::network::discovery::InviteCode;
use imax_core::network::protocol::WireMessage;
use crate::state::{
    SHOW_INVITE_MODAL, INVITE_CODE, NICKNAME, SIGNING_KEY_BYTES,
    CONNECTION_STATUS, NODE_STARTED, CHATS, ChatPreview,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[component]
pub fn InviteModal() -> Element {
    let show = *SHOW_INVITE_MODAL.read();
    let invite_code = INVITE_CODE.read().clone();
    let node_ready = *NODE_STARTED.read();
    let mut paste_input = use_signal(String::new);
    let mut copied = use_signal(|| false);
    let mut connect_status = use_signal(String::new);
    let mut connecting = use_signal(|| false);

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

                // Your invite code section
                div { class: "modal-section",
                    p { class: "modal-label", "Your invite code" }

                    if node_ready {
                        // Node is online — show real invite code
                        div { class: "modal-code-box",
                            code { class: "modal-code", "{invite_code}" }
                        }
                        button {
                            class: "modal-btn-secondary",
                            onclick: move |_| {
                                let code = INVITE_CODE.read().clone();
                                eval(&format!(r#"navigator.clipboard.writeText("{code}")"#));
                                *copied.write() = true;
                            },
                            if *copied.read() { "Copied!" } else { "Copy code" }
                        }
                    } else {
                        // Node still connecting
                        div { class: "modal-connecting",
                            p { "Connecting to P2P network..." }
                            p { class: "modal-connecting-hint", "This usually takes 2-5 seconds" }
                        }
                    }
                }

                // Divider
                div { class: "modal-divider", "or" }

                // Paste their code
                div { class: "modal-section",
                    p { class: "modal-label", "Paste their invite code" }
                    input {
                        class: "modal-input",
                        r#type: "text",
                        placeholder: "imax:...",
                        value: "{paste_input}",
                        disabled: !node_ready,
                        oninput: move |evt| {
                            *paste_input.write() = evt.value();
                            *connect_status.write() = String::new();
                        },
                    }

                    if !connect_status.read().is_empty() {
                        p { class: "modal-status", "{connect_status}" }
                    }

                    button {
                        class: "modal-btn-primary",
                        disabled: paste_input.read().is_empty() || !node_ready || *connecting.read(),
                        onclick: move |_| {
                            let code = paste_input.read().trim().to_string();
                            if code.is_empty() { return; }

                            *connect_status.write() = "Connecting...".into();
                            *connecting.write() = true;

                            spawn(async move {
                                match InviteCode::decode(&code) {
                                    Ok(payload) => {
                                        let sk_bytes = *SIGNING_KEY_BYTES.read();
                                        let iroh_key = SecretKey::from_bytes(&sk_bytes);

                                        match IrohNode::new(iroh_key).await {
                                            Ok(node) => {
                                                node.endpoint().online().await;

                                                let peer_id = match EndpointId::from_bytes(&payload.node_id) {
                                                    Ok(id) => id,
                                                    Err(e) => {
                                                        *connect_status.write() = format!("Invalid peer ID: {e}");
                                                        *connecting.write() = false;
                                                        return;
                                                    }
                                                };

                                                let mut transport_addrs: Vec<TransportAddr> = payload
                                                    .addrs.iter().map(|a| TransportAddr::Ip(*a)).collect();

                                                if let Some(relay_str) = &payload.relay_url {
                                                    if let Ok(relay_url) = relay_str.parse::<RelayUrl>() {
                                                        transport_addrs.push(TransportAddr::Relay(relay_url));
                                                    }
                                                }

                                                let peer_addr = EndpointAddr::from_parts(peer_id, transport_addrs);

                                                let nickname = NICKNAME.read().clone();
                                                let hello = WireMessage::Hello {
                                                    public_key: sk_bytes,
                                                    nickname: nickname.clone(),
                                                    protocol_version: 1,
                                                };

                                                match node.send_to_addr(peer_addr, &hello).await {
                                                    Ok(_) => {
                                                        let peer_name = format!("Peer {}", bs58::encode(&payload.public_key[..4]).into_string());
                                                        let chat = ChatPreview {
                                                            id: format!("chat-{}", hex(&payload.node_id[..4])),
                                                            peer_name,
                                                            last_message: "Connected!".into(),
                                                            time: "now".into(),
                                                            avatar_color: (payload.public_key[0] as usize) % 4,
                                                        };
                                                        CHATS.write().push(chat);
                                                        *CONNECTION_STATUS.write() = "connected".into();
                                                        *SHOW_INVITE_MODAL.write() = false;
                                                    }
                                                    Err(e) => {
                                                        *connect_status.write() = format!("Connection failed: {e}");
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                *connect_status.write() = format!("Network error: {e}");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        *connect_status.write() = format!("Invalid code: {e}");
                                    }
                                }
                                *connecting.write() = false;
                            });
                        },
                        if *connecting.read() { "Connecting..." } else if !node_ready { "Waiting for network..." } else { "Connect" }
                    }
                }
            }
        }
    }
}
