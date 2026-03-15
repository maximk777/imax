use iroh::SecretKey;
use imax_core::network::node::{IrohNode, ALPN};
use imax_core::network::protocol::{self, WireMessage};
use dioxus::prelude::ReadableExt;
use crate::state::{CHATS, NICKNAME, SIGNING_KEY_BYTES, CONNECTION_STATUS, ChatPreview};

pub async fn run_test_p2p() -> Result<(), String> {
    let sk_bytes = *SIGNING_KEY_BYTES.read();
    let our_key = SecretKey::from_bytes(&sk_bytes);

    let bob_key = SecretKey::from_bytes(&[42u8; 32]);

    // Start both nodes
    let our_node = IrohNode::new(our_key).await.map_err(|e| e.to_string())?;
    let bob_node = IrohNode::new(bob_key).await.map_err(|e| e.to_string())?;

    // Wait for relay
    our_node.endpoint().online().await;
    bob_node.endpoint().online().await;

    let bob_addr = bob_node.endpoint().addr();
    let _our_nickname = SIGNING_KEY_BYTES.read().clone();

    let nickname = NICKNAME.read().clone();
    let hello = WireMessage::Hello {
        public_key: sk_bytes,
        nickname: nickname.clone(),
        protocol_version: 1,
    };

    // Spawn Bob's accept loop FIRST
    let bob_accept = tokio::spawn(async move {
        match bob_node.accept_one().await {
            Ok((msg, _from_id)) => {
                println!("[test-bob] Received: {:?}", msg);
                Some((bob_node, msg))
            }
            Err(e) => {
                println!("[test-bob] Accept error: {e}");
                None
            }
        }
    });

    // Send Hello directly via endpoint (keeping connection alive)
    let conn = our_node.endpoint()
        .connect(bob_addr, ALPN)
        .await
        .map_err(|e| format!("connect failed: {e}"))?;

    let (mut send, _recv) = conn.open_bi()
        .await
        .map_err(|e| format!("open_bi failed: {e}"))?;

    let encoded = protocol::encode(&hello).map_err(|e| format!("encode failed: {e}"))?;
    send.write_all(&encoded)
        .await
        .map_err(|e| format!("write failed: {e}"))?;
    send.finish()
        .map_err(|e| format!("finish failed: {e}"))?;

    // IMPORTANT: Keep conn alive until Bob reads
    // Wait for Bob to receive
    let bob_result = bob_accept.await.map_err(|e| format!("bob task panic: {e}"))?;

    // Now we can drop connection
    drop(conn);

    if bob_result.is_some() {
        let already_exists = CHATS.read().iter().any(|c| c.id == "chat-test-bob");
        if !already_exists {
            let chat = ChatPreview {
                id: "chat-test-bob".into(),
                peer_name: "Bob (test peer)".into(),
                last_message: "P2P connected!".into(),
                time: "now".into(),
                avatar_color: 1,
            };
            CHATS.write().push(chat);
        }
        *CONNECTION_STATUS.write() = "online".to_string();
        println!("[test] P2P test successful! Bob received Hello.");
    } else {
        return Err("Bob did not receive the Hello".into());
    }

    our_node.shutdown().await.map_err(|e| format!("shutdown: {e}"))?;
    Ok(())
}
