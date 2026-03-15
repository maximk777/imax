use imax_core::network::node::IrohNode;
use imax_core::network::protocol::WireMessage;
use iroh::SecretKey;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let mut passed = 0u32;
    let mut failed = 0u32;
    let total = 4u32;

    // Create Alice and Bob nodes
    let key_alice = SecretKey::from_bytes(&[1u8; 32]);
    let key_bob = SecretKey::from_bytes(&[2u8; 32]);

    let alice = IrohNode::new(key_alice).await.expect("alice node creation failed");
    let bob = IrohNode::new(key_bob).await.expect("bob node creation failed");

    let alice_id = alice.node_id();
    let bob_id = bob.node_id();

    println!("[alice] Node online, ID: {}", alice_id);
    println!("[bob]   Node online, ID: {}", bob_id);

    // Wait for both to connect to relay
    alice.endpoint().online().await;
    bob.endpoint().online().await;
    println!("[info] Both nodes connected to relay\n");

    // Get full addresses for initial connections
    let alice_addr = alice.endpoint().addr();
    let bob_addr = bob.endpoint().addr();

    // Channel for received messages
    let (alice_tx, mut alice_rx) = mpsc::channel::<(WireMessage, iroh::EndpointId)>(16);
    let (bob_tx, mut bob_rx) = mpsc::channel::<(WireMessage, iroh::EndpointId)>(16);

    // Spawn accept loops
    let alice_accept = {
        let tx = alice_tx;
        let endpoint = alice.endpoint().clone();
        tokio::spawn(async move {
            loop {
                let incoming = match endpoint.accept().await {
                    Some(inc) => inc,
                    None => break,
                };
                let conn = match incoming.await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[alice] Accept error: {e}");
                        continue;
                    }
                };
                let remote_id = conn.remote_id();
                let (_, mut recv) = match conn.accept_bi().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[alice] accept_bi error: {e}");
                        continue;
                    }
                };
                let bytes = match recv.read_to_end(16 * 1024 * 1024).await {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[alice] read error: {e}");
                        continue;
                    }
                };
                let _ = recv.stop(0u32.into());
                let (msg, _) = imax_core::network::protocol::decode_frame(&bytes)
                    .expect("decode failed")
                    .expect("incomplete frame");
                let _ = tx.send((msg, remote_id)).await;
            }
        })
    };

    let bob_accept = {
        let tx = bob_tx;
        let endpoint = bob.endpoint().clone();
        tokio::spawn(async move {
            loop {
                let incoming = match endpoint.accept().await {
                    Some(inc) => inc,
                    None => break,
                };
                let conn = match incoming.await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[bob] Accept error: {e}");
                        continue;
                    }
                };
                let remote_id = conn.remote_id();
                let (_, mut recv) = match conn.accept_bi().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[bob] accept_bi error: {e}");
                        continue;
                    }
                };
                let bytes = match recv.read_to_end(16 * 1024 * 1024).await {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[bob] read error: {e}");
                        continue;
                    }
                };
                let _ = recv.stop(0u32.into());
                let (msg, _) = imax_core::network::protocol::decode_frame(&bytes)
                    .expect("decode failed")
                    .expect("incomplete frame");
                let _ = tx.send((msg, remote_id)).await;
            }
        })
    };

    // --- Test 1: Alice sends Hello to Bob via full address ---
    print!("[alice→bob] Sending Hello... ");
    let hello = WireMessage::Hello {
        public_key: [1u8; 32],
        nickname: "Alice".to_string(),
        protocol_version: 1,
    };
    match alice.send_to_addr(bob_addr, &hello).await {
        Ok(()) => {
            match tokio::time::timeout(Duration::from_secs(10), bob_rx.recv()).await {
                Ok(Some((msg, from))) => {
                    if from == alice_id {
                        println!("OK");
                        println!("[bob] Received {:?} from alice", msg_label(&msg));
                        passed += 1;
                    } else {
                        println!("FAIL (wrong sender)");
                        failed += 1;
                    }
                }
                _ => {
                    println!("FAIL (timeout waiting for message)");
                    failed += 1;
                }
            }
        }
        Err(e) => {
            println!("FAIL ({e})");
            failed += 1;
        }
    }

    // --- Test 2: Bob sends Hello back to Alice via peer ID (cached) ---
    print!("[bob→alice] Sending Hello back... ");
    let hello_back = WireMessage::Hello {
        public_key: [2u8; 32],
        nickname: "Bob".to_string(),
        protocol_version: 1,
    };
    match bob.send_to_addr(alice_addr, &hello_back).await {
        Ok(()) => {
            match tokio::time::timeout(Duration::from_secs(10), alice_rx.recv()).await {
                Ok(Some((msg, from))) => {
                    if from == bob_id {
                        println!("OK");
                        println!("[alice] Received {:?} from bob", msg_label(&msg));
                        passed += 1;
                    } else {
                        println!("FAIL (wrong sender)");
                        failed += 1;
                    }
                }
                _ => {
                    println!("FAIL (timeout waiting for message)");
                    failed += 1;
                }
            }
        }
        Err(e) => {
            println!("FAIL ({e})");
            failed += 1;
        }
    }

    // --- Test 3: Alice sends ChatMessage to Bob ---
    print!("[alice→bob] Sending ChatMessage \"hello from alice\"... ");
    let chat_msg = WireMessage::ChatMessage {
        id: Uuid::new_v4(),
        ciphertext: b"hello from alice".to_vec(),
        nonce: [0u8; 24],
        timestamp: 1000,
    };
    match alice.send_to_peer(bob_id, &chat_msg).await {
        Ok(()) => {
            match tokio::time::timeout(Duration::from_secs(10), bob_rx.recv()).await {
                Ok(Some((_msg, from))) => {
                    if from == alice_id {
                        println!("OK");
                        println!("[bob] Received ChatMessage from alice");
                        passed += 1;
                    } else {
                        println!("FAIL (wrong sender: {from})");
                        failed += 1;
                    }
                }
                _ => {
                    println!("FAIL (timeout waiting for message)");
                    failed += 1;
                }
            }
        }
        Err(e) => {
            println!("FAIL ({e})");
            failed += 1;
        }
    }

    // --- Test 4: Bob sends ChatMessage to Alice ---
    print!("[bob→alice] Sending ChatMessage \"hello from bob\"... ");
    let chat_msg2 = WireMessage::ChatMessage {
        id: Uuid::new_v4(),
        ciphertext: b"hello from bob".to_vec(),
        nonce: [1u8; 24],
        timestamp: 2000,
    };
    match bob.send_to_peer(alice_id, &chat_msg2).await {
        Ok(()) => {
            match tokio::time::timeout(Duration::from_secs(10), alice_rx.recv()).await {
                Ok(Some((_msg, from))) => {
                    if from == bob_id {
                        println!("OK");
                        println!("[alice] Received ChatMessage from bob");
                        passed += 1;
                    } else {
                        println!("FAIL (wrong sender: {from})");
                        failed += 1;
                    }
                }
                _ => {
                    println!("FAIL (timeout waiting for message)");
                    failed += 1;
                }
            }
        }
        Err(e) => {
            println!("FAIL ({e})");
            failed += 1;
        }
    }

    // Summary
    println!();
    if failed == 0 {
        println!("[PASS] All {total} message exchanges succeeded");
    } else {
        println!("[FAIL] {passed}/{total} passed, {failed}/{total} failed");
    }

    // Cleanup
    alice_accept.abort();
    bob_accept.abort();
    alice.shutdown().await.ok();
    bob.shutdown().await.ok();
}

fn msg_label(msg: &WireMessage) -> &'static str {
    match msg {
        WireMessage::Hello { .. } => "Hello",
        WireMessage::ChatMessage { .. } => "ChatMessage",
        WireMessage::Ack { .. } => "Ack",
        WireMessage::SyncRequest { .. } => "SyncRequest",
        WireMessage::SyncResponse { .. } => "SyncResponse",
        WireMessage::Ping => "Ping",
        WireMessage::Pong => "Pong",
    }
}
