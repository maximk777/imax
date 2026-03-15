use dioxus::prelude::*;
use dioxus::document::eval;
use iroh::{SecretKey, EndpointId, EndpointAddr, TransportAddr, RelayUrl};
use imax_core::network::node::IrohNode;
use imax_core::network::discovery::InviteCode;
use imax_core::network::protocol::WireMessage;
use crate::state::{
    SHOW_INVITE_MODAL, INVITE_CODE, NICKNAME, SIGNING_KEY_BYTES,
    CONNECTION_STATUS, CHATS, ChatPreview,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[component]
pub fn InviteModal() -> Element {
    let show = *SHOW_INVITE_MODAL.read();
    let invite_code = INVITE_CODE.read().clone();
    let mut paste_input = use_signal(String::new);
    let mut copied = use_signal(|| false);
    let mut connect_status = use_signal(String::new);

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
                            *connect_status.write() = String::new();
                        },
                    }

                    if !connect_status.read().is_empty() {
                        p { class: "modal-status", "{connect_status}" }
                    }

                    button {
                        class: "modal-btn-primary",
                        disabled: paste_input.read().is_empty(),
                        onclick: move |_| {
                            let code = paste_input.read().clone();
                            *connect_status.write() = "Connecting…".into();

                            spawn(async move {
                                match InviteCode::decode(&code) {
                                    Ok(payload) => {
                                        let sk_bytes = *SIGNING_KEY_BYTES.read();
                                        let iroh_key = SecretKey::from_bytes(&sk_bytes);

                                        match IrohNode::new(iroh_key).await {
                                            Ok(node) => {
                                                node.endpoint().online().await;

                                                // Reconstruct EndpointAddr from invite payload
                                                let peer_id = match EndpointId::from_bytes(&payload.node_id) {
                                                    Ok(id) => id,
                                                    Err(e) => {
                                                        println!("[imax] Invalid node_id in invite: {e}");
                                                        *connect_status.write() = format!("Invalid invite: {e}");
                                                        return;
                                                    }
                                                };

                                                // Build EndpointAddr with all transport addresses from payload
                                                let mut transport_addrs: Vec<TransportAddr> = payload
                                                    .addrs
                                                    .iter()
                                                    .map(|a| TransportAddr::Ip(*a))
                                                    .collect();

                                                if let Some(relay_str) = &payload.relay_url {
                                                    if let Ok(relay_url) = relay_str.parse::<RelayUrl>() {
                                                        transport_addrs.push(TransportAddr::Relay(relay_url));
                                                    }
                                                }

                                                let peer_addr = EndpointAddr::from_parts(peer_id, transport_addrs);

                                                let nickname = NICKNAME.read().clone();
                                                let my_pubkey = payload.public_key; // peer's public_key field — we send ours
                                                let my_sk_bytes = *SIGNING_KEY_BYTES.read();

                                                let hello = WireMessage::Hello {
                                                    public_key: my_sk_bytes,
                                                    nickname: nickname.clone(),
                                                    protocol_version: 1,
                                                };

                                                match node.send_to_addr(peer_addr, &hello).await {
                                                    Ok(_) => {
                                                        println!("[imax] Hello sent to peer {}", hex(&payload.node_id[..4]));

                                                        let peer_display = format!(
                                                            "Peer {}",
                                                            bs58::encode(&payload.public_key[..4]).into_string()
                                                        );
                                                        let chat = ChatPreview {
                                                            id: format!("chat-{}", hex(&payload.node_id[..4])),
                                                            peer_name: peer_display,
                                                            last_message: "Connected!".into(),
                                                            time: "now".into(),
                                                            avatar_color: (payload.public_key[0] as usize) % 4,
                                                        };
                                                        CHATS.write().push(chat);
                                                        *CONNECTION_STATUS.write() = "connected".into();
                                                        *SHOW_INVITE_MODAL.write() = false;
                                                    }
                                                    Err(e) => {
                                                        println!("[imax] Connect failed: {e}");
                                                        *connect_status.write() = format!("Connect failed: {e}");
                                                    }
                                                }
                                                let _ = my_pubkey; // suppress unused warning
                                            }
                                            Err(e) => {
                                                println!("[imax] Node error: {e}");
                                                *connect_status.write() = format!("Node error: {e}");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("[imax] Invalid invite code: {e}");
                                        *connect_status.write() = format!("Invalid invite code: {e}");
                                    }
                                }
                            });
                        },
                        "Connect"
                    }
                }
            }
        }
    }
}
