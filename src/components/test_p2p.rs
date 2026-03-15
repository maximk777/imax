use std::sync::Arc;
use iroh::SecretKey;
use tokio_util::sync::CancellationToken;
use imax_core::network::node::IrohNode;
use imax_core::network::protocol::{WireMessage, AckStatus};
use imax_core::network::discovery::{InviteCode, InvitePayload};
use dioxus::prelude::ReadableExt;
use uuid::Uuid;
use crate::state::{
    NICKNAME, SIGNING_KEY_BYTES, CONNECTION_STATUS, NODE_STARTED,
    INVITE_CODE, Message, OutgoingMessage, UiUpdate,
    UI_UPDATE_TX, get_peer_id, register_peer, hex,
    get_iroh_node, IROH_NODE, OUTGOING_TX, NODE_CANCEL,
};

/// Start the shared accept + outgoing message loop on the global node.
/// Each call starts a new loop (previous one should be cancelled via CancellationToken).
pub fn start_message_loop(node: Arc<IrohNode>, sk_bytes: [u8; 32], nickname: String, cancel: CancellationToken) {
    // Set up the outgoing message channel
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<OutgoingMessage>();
    *OUTGOING_TX.lock().unwrap() = Some(tx);

    println!("[imax] Starting message loop...");
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // ── Cancelled ──
                _ = cancel.cancelled() => {
                    println!("[imax] Message loop cancelled");
                    break;
                }

                // ── Incoming connection ──
                accept_result = node.accept_one() => {
                    match accept_result {
                        Ok((msg, from_id)) => {
                            println!("[imax] Incoming from {:?}: {:?}", from_id, msg);
                            match msg {
                                WireMessage::Hello { nickname: peer_nick, public_key, .. } => {
                                    let id_bytes = from_id.as_bytes();
                                    let chat_id = format!("chat-{}", hex(&id_bytes[..4]));

                                    // Check if peer is already registered (prevents Hello loop)
                                    let already_known = {
                                        get_peer_id(&chat_id).is_some()
                                    };

                                    // Register peer ID — iroh caches their transport addresses
                                    register_peer(chat_id.clone(), from_id);

                                    // Notify UI via channel (not direct GlobalSignal access)
                                    if let Some(tx) = UI_UPDATE_TX.get() {
                                        let _ = tx.send(UiUpdate::PeerConnected {
                                            chat_id: chat_id.clone(),
                                            peer_name: peer_nick,
                                            public_key_byte: public_key[0],
                                        });
                                    }

                                    // Auto-respond with our Hello if this is a new peer
                                    if !already_known {
                                        let hello_back = WireMessage::Hello {
                                            public_key: sk_bytes,
                                            nickname: nickname.clone(),
                                            protocol_version: 1,
                                        };
                                        if let Err(e) = node.send_to_peer(from_id, &hello_back).await {
                                            println!("[imax] Failed to send Hello back: {e}");
                                        }
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

                                    // Send Ack::Delivered back to peer
                                    let ack = WireMessage::Ack {
                                        message_id: id,
                                        status: AckStatus::Delivered,
                                    };
                                    if let Err(e) = node.send_to_peer(from_id, &ack).await {
                                        println!("[imax] Failed to send Delivered ack: {e}");
                                    }

                                    // Notify UI via channel
                                    if let Some(tx) = UI_UPDATE_TX.get() {
                                        // Ensure chat exists before delivering message
                                        let _ = tx.send(UiUpdate::PeerConnected {
                                            chat_id: chat_id.clone(),
                                            peer_name: "Unknown".into(),
                                            public_key_byte: from_id.as_bytes()[0],
                                        });
                                        let preview = content_preview(&content);
                                        let _ = tx.send(UiUpdate::ChatPreviewUpdate {
                                            chat_id: chat_id.clone(),
                                            last_message: preview,
                                        });
                                        let _ = tx.send(UiUpdate::MessageReceived {
                                            chat_id,
                                            message: Message {
                                                id: msg_id,
                                                content,
                                                is_mine: false,
                                                time: ts,
                                                status: "delivered".into(),
                                            },
                                        });
                                    }
                                }
                                WireMessage::Ack { message_id, status } => {
                                    let new_status = match status {
                                        AckStatus::Delivered => "delivered",
                                        AckStatus::Read => "read",
                                    };
                                    println!("[imax] Ack received: msg={message_id} status={new_status}");
                                    if let Some(tx) = UI_UPDATE_TX.get() {
                                        let _ = tx.send(UiUpdate::MessageStatusUpdate {
                                            message_id: message_id.to_string(),
                                            status: new_status.to_string(),
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
    let nickname = NICKNAME.read().clone();

    // Reuse global node if it exists, otherwise create one
    let node = if let Some(existing) = get_iroh_node() {
        println!("[test] Reusing existing node");
        existing
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
        *IROH_NODE.lock().unwrap() = Some(node.clone());
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

    // Start the shared message loop
    let cancel = NODE_CANCEL.lock().unwrap().clone();
    start_message_loop(node, sk_bytes, nickname, cancel);

    Ok(())
}

/// Get the current local time as HH:MM.
pub fn local_time_now() -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_timestamp_local(now_secs)
}

/// Format a UNIX timestamp as local HH:MM using platform C localtime.
fn format_timestamp_local(ts: u64) -> String {
    use std::sync::OnceLock;
    static UTC_OFFSET_SECS: OnceLock<i64> = OnceLock::new();

    let offset = *UTC_OFFSET_SECS.get_or_init(|| {
        match std::process::Command::new("date").arg("+%z").output() {
            Ok(out) => {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if s.len() >= 5 {
                    let sign: i64 = if s.starts_with('-') { -1 } else { 1 };
                    let hours: i64 = s[1..3].parse().unwrap_or(0);
                    let mins: i64 = s[3..5].parse().unwrap_or(0);
                    sign * (hours * 3600 + mins * 60)
                } else {
                    0
                }
            }
            Err(_) => 0,
        }
    });

    let local_ts = ts as i64 + offset;
    let secs_in_day = ((local_ts % 86400) + 86400) % 86400;
    let h = secs_in_day / 3600;
    let m = (secs_in_day % 3600) / 60;
    format!("{:02}:{:02}", h, m)
}

/// Format a UNIX timestamp as local HH:MM.
fn format_timestamp(ts: u64) -> String {
    format_timestamp_local(ts)
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
