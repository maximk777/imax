use iroh::SecretKey;
use imax_core::network::node::{IrohNode, ALPN};
use imax_core::network::protocol::{self, WireMessage};
use imax_core::network::discovery::{InviteCode, InvitePayload};
use dioxus::prelude::ReadableExt;
use uuid::Uuid;
use crate::state::{
    CHATS, MESSAGES, NICKNAME, SIGNING_KEY_BYTES, CONNECTION_STATUS, NODE_STARTED,
    INVITE_CODE, OUTGOING_TX, ChatPreview, Message, OutgoingMessage, get_peer_addr,
};

pub async fn run_test_p2p() -> Result<(), String> {
    let sk_bytes = *SIGNING_KEY_BYTES.read();
    let our_key = SecretKey::from_bytes(&sk_bytes);

    println!("[test] Creating node...");
    let our_node = IrohNode::new(our_key).await.map_err(|e| e.to_string())?;

    println!("[test] Waiting for relay...");
    let online = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        our_node.endpoint().online()
    ).await;
    match online {
        Ok(_) => println!("[test] Connected to relay!"),
        Err(_) => println!("[test] Relay timeout, proceeding anyway"),
    }

    // Generate invite code from our live node
    let addr = our_node.endpoint().addr();
    let node_id = our_node.node_id();
    let addrs: Vec<std::net::SocketAddr> = addr.ip_addrs().cloned().collect();
    let relay_url = addr.relay_urls().next().map(|u| u.to_string());
    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 86400;

    println!("[test] Node ID: {:?}", node_id);
    println!("[test] Relay: {:?}", relay_url);
    println!("[test] Addrs: {:?}", addrs);

    let payload = InvitePayload {
        public_key: sk_bytes,
        node_id: *node_id.as_bytes(),
        addrs,
        relay_url,
        expires,
    };
    if let Ok(code) = InviteCode::encode(&payload) {
        println!("[test] Invite code ready ({} chars)", code.0.len());
        *INVITE_CODE.write() = code.0;
    }

    *CONNECTION_STATUS.write() = "online".to_string();
    *NODE_STARTED.write() = true;

    // Now do a quick self-test with Bob
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

    let conn = our_node.endpoint()
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

    // Set up the outgoing message channel
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<OutgoingMessage>();
    let _ = OUTGOING_TX.set(tx);

    // Keep our_node alive — start accept loop for incoming real connections,
    // and also handle outgoing messages from the UI.
    println!("[test] Listening for incoming connections...");
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // ── Incoming connection ──
                accept_result = our_node.accept_one() => {
                    match accept_result {
                        Ok((msg, from_id)) => {
                            println!("[imax] Incoming from {:?}: {:?}", from_id, msg);
                            match msg {
                                WireMessage::Hello { nickname, public_key, .. } => {
                                    let chat_id = format!("chat-{:02x}{:02x}", public_key[0], public_key[1]);
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
                                    // For demo: ciphertext is actually plaintext UTF-8
                                    let content = String::from_utf8(ciphertext)
                                        .unwrap_or_else(|_| "(binary message)".to_string());
                                    let msg_id = id.to_string();
                                    let ts = format_timestamp(timestamp);

                                    // Find the chat_id for this sender
                                    let chat_id = {
                                        let id_bytes = from_id.as_bytes();
                                        format!("chat-{:02x}{:02x}", id_bytes[0], id_bytes[1])
                                    };

                                    // Update the chat preview last_message
                                    {
                                        let preview = content_preview(&content);
                                        let mut chats = CHATS.write();
                                        if let Some(c) = chats.iter_mut().find(|c| c.id == chat_id) {
                                            c.last_message = preview;
                                        }
                                    }

                                    // Only display if the message is for the active chat
                                    let active = crate::state::ACTIVE_CHAT_ID.read().clone();
                                    if active.as_deref() == Some(&chat_id) {
                                        MESSAGES.write().push(Message {
                                            id: msg_id,
                                            content,
                                            is_mine: false,
                                            time: ts,
                                            status: "received".into(),
                                        });
                                    }
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

                    match get_peer_addr(&outgoing.chat_id) {
                        Some(peer_addr) => {
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

                            match our_node.send_to_addr(peer_addr, &wire_msg).await {
                                Ok(_) => println!("[imax] Message sent successfully"),
                                Err(e) => println!("[imax] Send error: {e}"),
                            }
                        }
                        None => {
                            println!("[imax] No peer addr for chat_id: {}", outgoing.chat_id);
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

fn format_timestamp(ts: u64) -> String {
    // Simple HH:MM from unix seconds
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
