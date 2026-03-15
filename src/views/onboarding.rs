use dioxus::prelude::*;
use iroh::SecretKey;
use imax_core::network::node::IrohNode;
use imax_core::network::discovery::{InviteCode, InvitePayload};
use crate::state::{
    IS_ONBOARDED, NICKNAME, SEED_PHRASE, INVITE_CODE,
    SIGNING_KEY_BYTES, NODE_STARTED, CONNECTION_STATUS, CHATS, ChatPreview,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[component]
pub fn Onboarding() -> Element {
    let mut nick_input = use_signal(String::new);
    let mut error_msg = use_signal(String::new);
    let mut loading = use_signal(|| false);

    let on_start = move |_| {
        let name = nick_input.read().trim().to_string();
        if name.is_empty() {
            *error_msg.write() = "Please enter a nickname.".into();
            return;
        }

        *loading.write() = true;
        *error_msg.write() = String::new();

        match imax_core::identity::generate_mnemonic() {
            Ok(mnemonic) => {
                let signing_key = imax_core::identity::derive_signing_key(&mnemonic);
                let pubkey = signing_key.verifying_key();
                let pubkey_bytes = pubkey.to_bytes();

                // Save seed phrase for settings
                *SEED_PHRASE.write() = mnemonic.to_string();

                // Store signing key bytes for later use by the network layer
                let sk_bytes = signing_key.to_bytes();
                *SIGNING_KEY_BYTES.write() = sk_bytes;

                // Set a preliminary invite code (pubkey-only) so the UI shows something
                // while the iroh node starts up in the background.
                let preliminary_code = format!("imax:{}", bs58::encode(&pubkey_bytes).into_string());
                *INVITE_CODE.write() = preliminary_code;

                *NICKNAME.write() = name.clone();
                *CONNECTION_STATUS.write() = "connecting".to_string();
                *IS_ONBOARDED.write() = true;

                // Spawn async task: start the iroh node, then update to real invite code
                // and begin accepting incoming P2P connections.
                spawn(async move {
                    let iroh_key = SecretKey::from_bytes(&sk_bytes);
                    match IrohNode::new(iroh_key).await {
                        Ok(node) => {
                            // Wait until the endpoint has a relay connection so addr() is useful.
                            node.endpoint().online().await;

                            // Build a real invite payload with iroh addressing info.
                            let addr = node.endpoint().addr();
                            let node_id = node.node_id();
                            let addrs: Vec<std::net::SocketAddr> =
                                addr.ip_addrs().cloned().collect();
                            let relay_url = addr.relay_urls().next().map(|u| u.to_string());
                            let expires = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                                + 86400; // valid for 24 hours

                            let payload = InvitePayload {
                                public_key: pubkey_bytes,
                                node_id: *node_id.as_bytes(),
                                addrs,
                                relay_url,
                                expires,
                            };

                            if let Ok(code) = InviteCode::encode(&payload) {
                                *INVITE_CODE.write() = code.0;
                            }

                            *CONNECTION_STATUS.write() = "online".to_string();
                            *NODE_STARTED.write() = true;

                            println!("[imax] Node online — node_id = {}", hex(node_id.as_bytes()));

                            // Accept loop: handle incoming P2P connections.
                            loop {
                                match node.accept_one().await {
                                    Ok((msg, from_id)) => {
                                        println!(
                                            "[imax] Received from {}: {:?}",
                                            hex(from_id.as_bytes()),
                                            msg
                                        );
                                        match msg {
                                            imax_core::network::protocol::WireMessage::Hello {
                                                nickname,
                                                public_key,
                                                ..
                                            } => {
                                                let chat = ChatPreview {
                                                    id: format!("chat-{}", hex(&public_key[..4])),
                                                    peer_name: nickname,
                                                    last_message: "Connected!".into(),
                                                    time: "now".into(),
                                                    avatar_color: (public_key[0] as usize) % 4,
                                                };
                                                CHATS.write().push(chat);
                                            }
                                            imax_core::network::protocol::WireMessage::ChatMessage {
                                                ciphertext,
                                                ..
                                            } => {
                                                println!(
                                                    "[imax] Got encrypted message, {} bytes",
                                                    ciphertext.len()
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                    Err(e) => {
                                        println!("[imax] Accept error: {e}");
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("[imax] Failed to start node: {e}");
                            *CONNECTION_STATUS.write() = format!("error: {e}");
                        }
                    }
                });
            }
            Err(e) => {
                *error_msg.write() = format!("Identity error: {e}");
                *loading.write() = false;
            }
        }
    };

    rsx! {
        div { class: "onboarding-screen",
            div { class: "onboarding-card",
                div { class: "onboarding-logo", "iMax" }
                p { class: "onboarding-tagline", "P2P · End-to-End Encrypted · Open Source" }

                div { class: "onboarding-form",
                    div {
                        p { class: "onboarding-label", "Your nickname" }
                        input {
                            class: "onboarding-input",
                            r#type: "text",
                            placeholder: "e.g. Alice",
                            value: "{nick_input}",
                            oninput: move |evt| {
                                *nick_input.write() = evt.value();
                                *error_msg.write() = String::new();
                            },
                        }
                    }

                    if !error_msg.read().is_empty() {
                        p { class: "onboarding-error", "{error_msg}" }
                    }

                    button {
                        class: "onboarding-btn-primary",
                        disabled: *loading.read(),
                        onclick: on_start,
                        if *loading.read() { "Generating keys..." } else { "Start Messaging" }
                    }
                }

                button {
                    class: "onboarding-link",
                    onclick: |_| {},
                    "I have a seed phrase"
                }
            }
        }
    }
}
