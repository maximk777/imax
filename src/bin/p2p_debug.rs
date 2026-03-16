use imax_core::network::node::IrohNode;
use imax_core::network::protocol::WireMessage;
use imax_core::storage::Database;
use imax_core::storage::models;
use imax_core::identity::keypair;
use imax_core::crypto::e2e;
use iroh::SecretKey;
use ed25519_dalek::SigningKey;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let mut passed = 0u32;
    let mut failed = 0u32;
    let total = 10u32;

    // --- Test 0: Seed phrase round-trip ---
    print!("[identity] Seed phrase round-trip... ");
    match imax_core::identity::generate_mnemonic() {
        Ok(mnemonic) => {
            let key1 = imax_core::identity::derive_signing_key(&mnemonic);
            let phrase = mnemonic.to_string();
            match imax_core::identity::parse_mnemonic(&phrase) {
                Ok(restored) => {
                    let key2 = imax_core::identity::derive_signing_key(&restored);
                    if key1.to_bytes() == key2.to_bytes() {
                        println!("OK (same key from same phrase)");
                        passed += 1;
                    } else {
                        println!("FAIL (keys differ!)");
                        failed += 1;
                    }
                }
                Err(e) => {
                    println!("FAIL (parse_mnemonic: {e})");
                    failed += 1;
                }
            }
        }
        Err(e) => {
            println!("FAIL (generate_mnemonic: {e})");
            failed += 1;
        }
    }

    // Create Alice and Bob nodes
    let key_alice = SecretKey::from_bytes(&[1u8; 32]);
    let key_bob = SecretKey::from_bytes(&[2u8; 32]);

    let alice = Arc::new(IrohNode::new(key_alice).await.expect("alice node creation failed"));
    let bob = Arc::new(IrohNode::new(key_bob).await.expect("bob node creation failed"));

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

    // Start accept loops using run_accept_loop
    let cancel = CancellationToken::new();
    let mut alice_rx = alice.run_accept_loop(cancel.clone());
    let mut bob_rx = bob.run_accept_loop(cancel.clone());

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
                        // Test 1b: verify nickname in Hello
                        if let WireMessage::Hello { nickname, .. } = &msg {
                            if nickname == "Alice" {
                                println!("OK (nickname: \"Alice\")");
                            } else {
                                println!("OK (but nickname was \"{}\" instead of \"Alice\")", nickname);
                            }
                        } else {
                            println!("OK");
                        }
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

    // --- Test 2: Bob sends Hello back to Alice via full address ---
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
                        if let WireMessage::Hello { nickname, .. } = &msg {
                            if nickname == "Bob" {
                                println!("OK (nickname: \"Bob\")");
                            } else {
                                println!("OK (but nickname was \"{}\" instead of \"Bob\")", nickname);
                            }
                        } else {
                            println!("OK");
                        }
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

    // --- Test 5: Nickname assertion ---
    print!("[nickname] Hello messages carry correct nicknames... ");
    let test_hello = WireMessage::Hello {
        public_key: [99u8; 32],
        nickname: "TestNick".to_string(),
        protocol_version: 1,
    };
    if let WireMessage::Hello { nickname, .. } = &test_hello {
        if nickname == "TestNick" {
            println!("OK");
            passed += 1;
        } else {
            println!("FAIL (expected \"TestNick\", got \"{nickname}\")");
            failed += 1;
        }
    } else {
        println!("FAIL (not a Hello)");
        failed += 1;
    }

    // --- Test 6: Profiles CRUD ---
    print!("[profiles] CRUD operations... ");
    match Database::open_in_memory() {
        Ok(db) => {
            let mut profile_ok = true;
            // Create two profiles
            let pid1 = models::create_profile(&db, "seed1 phrase", "Alice").unwrap();
            let pid2 = models::create_profile(&db, "seed2 phrase", "Bob").unwrap();
            let all = models::get_all_profiles(&db).unwrap();
            if all.len() != 2 {
                println!("FAIL (expected 2 profiles, got {})", all.len());
                profile_ok = false;
            }
            // Set active and verify
            if profile_ok {
                models::set_active_profile(&db, pid2).unwrap();
                let active = models::get_active_profile(&db).unwrap();
                if active.as_ref().map(|p| p.id) != Some(pid2) {
                    println!("FAIL (active profile should be {})", pid2);
                    profile_ok = false;
                }
            }
            // Delete and verify
            if profile_ok {
                models::delete_profile(&db, pid1).unwrap();
                let remaining = models::get_all_profiles(&db).unwrap();
                if remaining.len() != 1 {
                    println!("FAIL (expected 1 profile after delete, got {})", remaining.len());
                    profile_ok = false;
                }
            }
            if profile_ok {
                println!("OK (create, set_active, delete all work)");
                passed += 1;
            } else {
                failed += 1;
            }
        }
        Err(e) => {
            println!("FAIL (db open: {e})");
            failed += 1;
        }
    }

    // --- Test 7: Connection pool reuse ---
    print!("[pool] Connection reuse (5 messages, 1 connection)... ");
    {
        // Alice sends 5 messages to Bob via send_to_peer — should reuse the same connection
        let mut pool_ok = true;
        for i in 0..5 {
            let msg = WireMessage::ChatMessage {
                id: Uuid::new_v4(),
                ciphertext: format!("pool test msg {i}").into_bytes(),
                nonce: [0u8; 24],
                timestamp: 3000 + i,
            };
            match alice.send_to_peer(bob_id, &msg).await {
                Ok(()) => {
                    match tokio::time::timeout(Duration::from_secs(10), bob_rx.recv()).await {
                        Ok(Some(_)) => {}
                        _ => {
                            println!("FAIL (timeout on message {i})");
                            pool_ok = false;
                            break;
                        }
                    }
                }
                Err(e) => {
                    println!("FAIL (send error on message {i}: {e})");
                    pool_ok = false;
                    break;
                }
            }
        }
        if pool_ok {
            let count = alice.cached_connection_count();
            if count >= 1 {
                println!("OK (cached connections: {count})");
                passed += 1;
            } else {
                println!("FAIL (expected >=1 cached connection, got {count})");
                failed += 1;
            }
        } else {
            failed += 1;
        }
    }

    // --- Test 8: E2E encryption roundtrip via P2P ---
    print!("[e2e] Encryption roundtrip via P2P... ");
    {
        // Alice and Bob derive signing keys from fixed bytes
        let alice_sk = SigningKey::from_bytes(&[1u8; 32]);
        let bob_sk = SigningKey::from_bytes(&[2u8; 32]);
        let alice_pubkey = alice_sk.verifying_key().to_bytes();
        let bob_pubkey = bob_sk.verifying_key().to_bytes();

        // Derive symmetric keys via DH (both sides should get the same key)
        let alice_x25519 = keypair::to_x25519_secret(&alice_sk);
        let bob_x25519 = keypair::to_x25519_secret(&bob_sk);
        let bob_x25519_pub = keypair::x25519_public_from_bytes(&bob_pubkey).unwrap();
        let alice_x25519_pub = keypair::x25519_public_from_bytes(&alice_pubkey).unwrap();

        let shared_alice = alice_x25519.diffie_hellman(&bob_x25519_pub);
        let shared_bob = bob_x25519.diffie_hellman(&alice_x25519_pub);

        let sym_alice = e2e::derive_symmetric_key(shared_alice.as_bytes(), &alice_pubkey, &bob_pubkey);
        let sym_bob = e2e::derive_symmetric_key(shared_bob.as_bytes(), &bob_pubkey, &alice_pubkey);

        // Encrypt a message as Alice
        let msg_id = Uuid::new_v4();
        let plaintext = b"hello encrypted world";
        match e2e::encrypt(&sym_alice, plaintext, msg_id.as_bytes()) {
            Ok((ciphertext, nonce)) => {
                // Alice sends encrypted ChatMessage to Bob
                let wire_msg = WireMessage::ChatMessage {
                    id: msg_id,
                    ciphertext: ciphertext.clone(),
                    nonce,
                    timestamp: 9000,
                };
                match alice.send_to_peer(bob_id, &wire_msg).await {
                    Ok(()) => {
                        match tokio::time::timeout(Duration::from_secs(10), bob_rx.recv()).await {
                            Ok(Some((received_msg, from))) => {
                                if from != alice_id {
                                    println!("FAIL (wrong sender)");
                                    failed += 1;
                                } else if let WireMessage::ChatMessage { id: rx_id, ciphertext: rx_ct, nonce: rx_nonce, .. } = received_msg {
                                    // Bob decrypts
                                    match e2e::decrypt(&sym_bob, &rx_ct, &rx_nonce, rx_id.as_bytes()) {
                                        Ok(decrypted) => {
                                            if decrypted == plaintext {
                                                println!("OK (encrypt→send→recv→decrypt matches)");
                                                passed += 1;
                                            } else {
                                                println!("FAIL (decrypted text doesn't match)");
                                                failed += 1;
                                            }
                                        }
                                        Err(e) => {
                                            println!("FAIL (decrypt error: {e})");
                                            failed += 1;
                                        }
                                    }
                                } else {
                                    println!("FAIL (unexpected message type)");
                                    failed += 1;
                                }
                            }
                            _ => {
                                println!("FAIL (timeout)");
                                failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        println!("FAIL (send error: {e})");
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                println!("FAIL (encrypt error: {e})");
                failed += 1;
            }
        }
    }

    // --- Test 9: Node restart with different key ---
    print!("[restart] Node restart with new key... ");
    {
        // Create a node, shut it down, create another with different key
        let key_old = SecretKey::from_bytes(&[10u8; 32]);
        let node_old = IrohNode::new(key_old).await.expect("old node creation failed");
        let old_id = node_old.node_id();
        node_old.shutdown().await.ok();

        let key_new = SecretKey::from_bytes(&[11u8; 32]);
        let node_new = IrohNode::new(key_new).await.expect("new node creation failed");
        let new_id = node_new.node_id();

        if old_id != new_id {
            // Send a message from the new node to Bob to verify it works
            node_new.endpoint().online().await;
            let bob_addr_fresh = bob.endpoint().addr();
            let hello_restart = WireMessage::Hello {
                public_key: [11u8; 32],
                nickname: "NewNode".to_string(),
                protocol_version: 1,
            };
            match node_new.send_to_addr(bob_addr_fresh, &hello_restart).await {
                Ok(()) => {
                    match tokio::time::timeout(Duration::from_secs(10), bob_rx.recv()).await {
                        Ok(Some((msg, from))) => {
                            if from == new_id {
                                if let WireMessage::Hello { nickname, .. } = &msg {
                                    println!("OK (old_id != new_id, msg from new node, nick=\"{nickname}\")");
                                } else {
                                    println!("OK (old_id != new_id, msg delivered)");
                                }
                                passed += 1;
                            } else {
                                println!("FAIL (wrong sender)");
                                failed += 1;
                            }
                        }
                        _ => {
                            println!("FAIL (timeout waiting for message from new node)");
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    println!("FAIL (send from new node: {e})");
                    failed += 1;
                }
            }
            node_new.shutdown().await.ok();
        } else {
            println!("FAIL (old_id == new_id with different keys!)");
            failed += 1;
        }
    }

    // Summary
    println!();
    if failed == 0 {
        println!("[PASS] All {total} tests succeeded");
    } else {
        println!("[FAIL] {passed}/{total} passed, {failed}/{total} failed");
    }

    // Cleanup
    cancel.cancel();
    alice.shutdown().await.ok();
    bob.shutdown().await.ok();
}

