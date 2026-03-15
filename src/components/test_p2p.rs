use std::sync::{Arc, OnceLock};
use iroh::SecretKey;
use imax_core::network::node::{IrohNode, ALPN};
use imax_core::network::protocol::{self, WireMessage};
use imax_core::network::discovery::{InviteCode, InvitePayload};
use dioxus::prelude::ReadableExt;
use uuid::Uuid;
use crate::state::{
    CHATS, NICKNAME, SIGNING_KEY_BYTES, CONNECTION_STATUS, NODE_STARTED,
    INVITE_CODE, OUTGOING_TX, IROH_NODE, ChatPreview, Message, OutgoingMessage,
    get_peer_id, register_peer, add_message, hex,
};

static MESSAGE_LOOP_STARTED: OnceLock<()> = OnceLock::new();

/// Start the shared accept + outgoing message loop on the global node.
/// Safe to call multiple times — only the first call starts the loop.
pub fn start_message_loop(node: Arc<IrohNode>) {
    if MESSAGE_LOOP_STARTED.set(()).is_err() {
        return; // already started
    }

    // Set up the outgoing message channel
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<OutgoingMessage>();
    let _ = OUTGOING_TX.set(tx);

    println!("[imax] Starting message loop...");
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // ── Incoming connection ──
                accept_result = node.accept_one() => {
                    match accept_result {
                        Ok((msg, from_id)) => {
                            println!("[imax] Incoming from {:?}: {:?}", from_id, msg);
                            match msg {
                                WireMessage::Hello { nickname, public_key, .. } => {
                                    let id_bytes = from_id.as_bytes();
                                    let chat_id = format!("chat-{}", hex(&id_bytes[..4]));
                                    // Register peer ID — iroh caches their transport addresses
                                    register_peer(chat_id.clone(), from_id);
                                    let exists = CHATS.read().iter().any(|c| c.id == chat_id);
                                    if !exists {
                                        CHATS.write().push(ChatPreview {
                                            id: chat_id,
                                            peer_name: nickname,
                                            last_message: "Connected!".into(),
                                            time: "now".into(),
                                            avatar_color: (public_key[0] as usize) % 4,
                                        });
                                    }
                                }
                                WireMessage::ChatMessage { id, ciphertext, timestamp, .. } => {
                                    let content = String::from_utf8(ciphertext)
                                        .unwrap_or_else(|_| "(binary message)".to_string());
                                    let msg_id = id.to_string();
                                    let ts = format_timestamp(timestamp);

                                    let chat_id = {
                                        let id_bytes = from_id.as_bytes();
                                        format!("chat-{}", hex(&id_bytes[..4]))
                                    };

                                    // Update the chat preview last_message
                                    {
                                        let preview = content_preview(&content);
                                        let mut chats = CHATS.write();
                                        if let Some(c) = chats.iter_mut().find(|c| c.id == chat_id) {
                                            c.last_message = preview;
                                        }
                                    }

                                    add_message(&chat_id, Message {
                                        id: msg_id,
                                        content,
                                        is_mine: false,
                                        time: ts,
                                        status: "received".into(),
                                    });
                                }
                                _ => {
                                    println!("[imax] Unhandled message type from {:?}", from_id);
                                }
                            }
                        }
                        Err(e) => {
                            println!("[imax] Accept error: {e}");
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
                }

                // ── Outgoing message from UI ──
                Some(outgoing) = rx.recv() => {
                    println!("[imax] Sending message to chat {}: {}", outgoing.chat_id, outgoing.text);

                    match get_peer_id(&outgoing.chat_id) {
                        Some(peer_id) => {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            let wire_msg = WireMessage::ChatMessage {
                                id: Uuid::new_v4(),
                                ciphertext: outgoing.text.into_bytes(),
                                nonce: [0u8; 24],
                                timestamp: now,
                            };

                            match node.send_to_peer(peer_id, &wire_msg).await {
                                Ok(_) => println!("[imax] Message sent successfully"),
                                Err(e) => println!("[imax] Send error: {e}"),
                            }
                        }
                        None => {
                            println!("[imax] No peer ID for chat_id: {}", outgoing.chat_id);
                        }
                    }
                }
            }
        }
    });
}

/// Generate an invite code from a live node and store it in INVITE_CODE.
fn generate_invite_code(node: &IrohNode, sk_bytes: &[u8; 32]) {
    let addr = node.endpoint().addr();
    let node_id = node.node_id();
    let addrs: Vec<std::net::SocketAddr> = addr.ip_addrs().cloned().collect();
    let relay_url = addr.relay_urls().next().map(|u| u.to_string());
    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 86400;

    let payload = InvitePayload {
        public_key: *sk_bytes,
        node_id: *node_id.as_bytes(),
        addrs,
        relay_url,
        expires,
    };
    if let Ok(code) = InviteCode::encode(&payload) {
        println!("[imax] Invite code ready ({} chars)", code.0.len());
        *INVITE_CODE.write() = code.0;
    }
}

pub async fn run_test_p2p() -> Result<(), String> {
    let sk_bytes = *SIGNING_KEY_BYTES.read();

    // Reuse global node if it exists, otherwise create one
    let node = if let Some(existing) = IROH_NODE.get() {
        println!("[test] Reusing existing node");
        existing.clone()
    } else {
        println!("[test] Creating node...");
        let our_key = SecretKey::from_bytes(&sk_bytes);
        let new_node = IrohNode::new(our_key).await.map_err(|e| e.to_string())?;

        println!("[test] Waiting for relay...");
        let online = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            new_node.endpoint().online()
        ).await;
        match online {
            Ok(_) => println!("[test] Connected to relay!"),
            Err(_) => println!("[test] Relay timeout, proceeding anyway"),
        }

        let node = Arc::new(new_node);
        let _ = IROH_NODE.set(node.clone());
        node
    };

    // Generate invite code from the live node
    generate_invite_code(&node, &sk_bytes);

    let node_id = node.node_id();
    let addr = node.endpoint().addr();
    let relay_url = addr.relay_urls().next().map(|u| u.to_string());
    println!("[test] Node ID: {:?}", node_id);
    println!("[test] Relay: {:?}", relay_url);

    *CONNECTION_STATUS.write() = "online".to_string();
    *NODE_STARTED.write() = true;

    // Quick self-test with Bob (separate identity — no conflict)
    let bob_key = SecretKey::from_bytes(&[42u8; 32]);
    let bob_node = IrohNode::new(bob_key).await.map_err(|e| e.to_string())?;
    bob_node.endpoint().online().await;
    let bob_addr = bob_node.endpoint().addr();

    let nickname = NICKNAME.read().clone();
    let hello = WireMessage::Hello {
        public_key: sk_bytes,
        nickname: nickname.clone(),
        protocol_version: 1,
    };

    let bob_accept = tokio::spawn(async move {
        match bob_node.accept_one().await {
            Ok((msg, _)) => { println!("[test-bob] Got: {:?}", msg); Some(()) }
            Err(e) => { println!("[test-bob] Error: {e}"); None }
        }
    });

    let conn = node.endpoint()
        .connect(bob_addr, ALPN).await
        .map_err(|e| format!("connect: {e}"))?;
    let (mut send, _) = conn.open_bi().await.map_err(|e| format!("open_bi: {e}"))?;
    let encoded = protocol::encode(&hello).map_err(|e| format!("encode: {e}"))?;
    send.write_all(&encoded).await.map_err(|e| format!("write: {e}"))?;
    send.finish().map_err(|e| format!("finish: {e}"))?;

    let ok = bob_accept.await.map_err(|e| format!("bob: {e}"))?.is_some();
    drop(conn);

    if ok {
        let exists = CHATS.read().iter().any(|c| c.id == "chat-test-bob");
        if !exists {
            CHATS.write().push(ChatPreview {
                id: "chat-test-bob".into(),
                peer_name: "Bob (test peer)".into(),
                last_message: "P2P connected!".into(),
                time: "now".into(),
                avatar_color: 1,
            });
        }
        println!("[test] Self-test passed!");
    }

    // Start the shared message loop (no-op if already started)
    start_message_loop(node);

    Ok(())
}

fn format_timestamp(ts: u64) -> String {
    let secs = ts % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    format!("{:02}:{:02}", h, m)
}

fn content_preview(s: &str) -> String {
    if s.is_empty() {
        "(message)".to_string()
    } else if s.len() > 40 {
        format!("{}…", &s[..40])
    } else {
        s.to_string()
    }
}
