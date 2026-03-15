use std::sync::Arc;
use dioxus::prelude::*;
use iroh::SecretKey;
use imax_core::network::node::IrohNode;
use imax_core::network::discovery::{InviteCode, InvitePayload};
use crate::state::{
    IS_ONBOARDED, NICKNAME, SEED_PHRASE, INVITE_CODE,
    SIGNING_KEY_BYTES, NODE_STARTED, CONNECTION_STATUS, IROH_NODE,
};
use crate::components::test_p2p::start_message_loop;

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

                *SEED_PHRASE.write() = mnemonic.to_string();
                let sk_bytes = signing_key.to_bytes();
                *SIGNING_KEY_BYTES.write() = sk_bytes;
                *NICKNAME.write() = name.clone();
                *CONNECTION_STATUS.write() = "connecting".to_string();
                *IS_ONBOARDED.write() = true;

                // Spawn background iroh node
                spawn(async move {
                    println!("[imax] Starting iroh node...");
                    let iroh_key = SecretKey::from_bytes(&sk_bytes);
                    match IrohNode::new(iroh_key).await {
                        Ok(new_node) => {
                            println!("[imax] Node created, waiting for relay...");

                            let online_result = tokio::time::timeout(
                                std::time::Duration::from_secs(15),
                                new_node.endpoint().online()
                            ).await;

                            match online_result {
                                Ok(_) => println!("[imax] Node connected to relay!"),
                                Err(_) => println!("[imax] Relay timeout (15s), proceeding anyway"),
                            }

                            // Generate real invite code
                            let addr = new_node.endpoint().addr();
                            let node_id = new_node.node_id();
                            let addrs: Vec<std::net::SocketAddr> =
                                addr.ip_addrs().cloned().collect();
                            let relay_url = addr.relay_urls().next().map(|u| u.to_string());

                            println!("[imax] Node ID: {:?}", node_id);
                            println!("[imax] Direct addrs: {:?}", addrs);
                            println!("[imax] Relay URL: {:?}", relay_url);

                            let expires = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() + 86400;

                            let payload = InvitePayload {
                                public_key: pubkey_bytes,
                                node_id: *node_id.as_bytes(),
                                addrs,
                                relay_url,
                                expires,
                            };

                            match InviteCode::encode(&payload) {
                                Ok(code) => {
                                    println!("[imax] Invite code generated ({} chars)", code.0.len());
                                    *INVITE_CODE.write() = code.0;
                                }
                                Err(e) => println!("[imax] Invite encode error: {e}"),
                            }

                            *CONNECTION_STATUS.write() = "online".to_string();
                            *NODE_STARTED.write() = true;

                            // Store the node globally and start the shared message loop
                            let node = Arc::new(new_node);
                            let _ = IROH_NODE.set(node.clone());
                            start_message_loop(node, sk_bytes, name.clone());
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
