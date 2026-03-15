use iroh::SecretKey;
use imax_core::network::node::{IrohNode, ALPN};
use imax_core::network::protocol::{self, WireMessage};
use imax_core::network::discovery::{InviteCode, InvitePayload};
use dioxus::prelude::ReadableExt;
use crate::state::{CHATS, NICKNAME, SIGNING_KEY_BYTES, CONNECTION_STATUS, NODE_STARTED, INVITE_CODE, ChatPreview};

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

    // Keep our_node alive — start accept loop for incoming real connections
    println!("[test] Listening for incoming connections...");
    tokio::spawn(async move {
        loop {
            match our_node.accept_one().await {
                Ok((msg, from_id)) => {
                    println!("[imax] Incoming from {:?}: {:?}", from_id, msg);
                    if let WireMessage::Hello { nickname, public_key, .. } = msg {
                        let exists = CHATS.read().iter().any(|c| {
                            c.id == format!("chat-{:02x}{:02x}", public_key[0], public_key[1])
                        });
                        if !exists {
                            CHATS.write().push(ChatPreview {
                                id: format!("chat-{:02x}{:02x}", public_key[0], public_key[1]),
                                peer_name: nickname,
                                last_message: "Connected!".into(),
                                time: "now".into(),
                                avatar_color: (public_key[0] as usize) % 4,
                            });
                        }
                    }
                }
                Err(e) => {
                    println!("[imax] Accept error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });

    Ok(())
}
