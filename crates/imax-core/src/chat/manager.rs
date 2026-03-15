use tokio::sync::broadcast;
use uuid::Uuid;
use x25519_dalek::StaticSecret as X25519Secret;
use crate::chat::types::*;
use crate::storage::Database;
use crate::storage::models;
use crate::network::node::IrohNode;
use crate::network::discovery::{InviteCode, InvitePayload};
use crate::network::protocol::WireMessage;
use crate::crypto::e2e;
use crate::identity::keypair;
use crate::Result;

pub struct ChatManager {
    db: Database,
    my_pubkey: [u8; 32],
    my_x25519_secret: X25519Secret,
    event_tx: broadcast::Sender<ChatEvent>,
}

impl ChatManager {
    pub fn new(db: Database, my_pubkey: [u8; 32], my_x25519_secret: X25519Secret) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self { db, my_pubkey, my_x25519_secret, event_tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent> {
        self.event_tx.subscribe()
    }

    pub fn get_chats(&self) -> Result<Vec<ChatPreview>> {
        let raw_chats = models::get_chats(&self.db)?;
        let mut previews = Vec::new();
        for (id, peer_key_bytes, _created_at, last_msg_id, unread) in raw_chats {
            let pk: [u8; 32] = peer_key_bytes.try_into()
                .map_err(|_| crate::Error::Chat("invalid pubkey length".into()))?;
            let contact = models::get_contact(&self.db, &pk)?;
            let nickname = contact.map(|c| c.1).unwrap_or_else(|| "Unknown".to_string());

            let (last_text, last_time) = if last_msg_id.is_some() {
                let latest = self.db.conn().query_row(
                    "SELECT content, created_at FROM messages WHERE chat_id = ?1 ORDER BY seq DESC LIMIT 1",
                    rusqlite::params![id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                ).ok();
                match latest {
                    Some((text, time)) => (Some(text), Some(time)),
                    None => (None, None),
                }
            } else {
                (None, None)
            };

            previews.push(ChatPreview {
                id, peer_key: pk, peer_nickname: nickname,
                last_message_text: last_text, last_message_time: last_time,
                unread_count: unread, is_online: false,
            });
        }
        Ok(previews)
    }

    pub fn get_messages(&self, chat_id: &str, limit: usize, before_seq: Option<i64>) -> Result<Vec<Message>> {
        let raw = models::get_messages(&self.db, chat_id, limit, before_seq)?;
        Ok(raw.into_iter().map(|(id, sender, content, seq, status, created_at)| {
            let sk: [u8; 32] = sender.try_into().unwrap_or([0u8; 32]);
            Message {
                id, chat_id: chat_id.to_string(), sender_key: sk, content, seq,
                status: MessageStatus::from_str(&status), created_at,
                is_mine: sk == self.my_pubkey,
            }
        }).collect())
    }

    pub fn add_contact_and_chat(&self, peer_key: &[u8; 32], nickname: &str) -> Result<ChatId> {
        models::insert_contact(&self.db, peer_key, nickname, None)?;
        models::create_chat(&self.db, peer_key)
    }

    pub fn store_outgoing_message(&self, chat_id: &str, text: &str) -> Result<Message> {
        let seq = models::get_next_seq(&self.db, chat_id)?;
        let msg_id = models::insert_message(&self.db, chat_id, &self.my_pubkey, text, seq, "pending")?;
        let msg = Message {
            id: msg_id, chat_id: chat_id.to_string(), sender_key: self.my_pubkey,
            content: text.to_string(), seq, status: MessageStatus::Pending,
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            is_mine: true,
        };
        let _ = self.event_tx.send(ChatEvent::NewMessage { chat_id: chat_id.to_string(), message: msg.clone() });
        Ok(msg)
    }

    pub fn store_incoming_message(&self, chat_id: &str, sender_key: &[u8; 32], text: &str) -> Result<Message> {
        let seq = models::get_next_seq(&self.db, chat_id)?;
        let msg_id = models::insert_message(&self.db, chat_id, sender_key, text, seq, "delivered")?;
        let msg = Message {
            id: msg_id, chat_id: chat_id.to_string(), sender_key: *sender_key,
            content: text.to_string(), seq, status: MessageStatus::Delivered,
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            is_mine: false,
        };
        let _ = self.event_tx.send(ChatEvent::NewMessage { chat_id: chat_id.to_string(), message: msg.clone() });
        Ok(msg)
    }

    pub fn update_status(&self, message_id: &str, status: MessageStatus) -> Result<()> {
        models::update_message_status(&self.db, message_id, status.as_str())?;
        let _ = self.event_tx.send(ChatEvent::MessageStatusChanged { message_id: message_id.to_string(), status });
        Ok(())
    }

    pub fn db(&self) -> &Database { &self.db }

    /// Generate an invite code containing our iroh endpoint info.
    pub fn generate_invite(&self, node: &IrohNode) -> Result<InviteCode> {
        let endpoint_addr = node.endpoint().addr();
        let node_id_bytes = node.node_id().as_bytes().clone();

        // Collect direct addresses
        let addrs: Vec<std::net::SocketAddr> = endpoint_addr
            .ip_addrs()
            .cloned()
            .collect();

        // Get relay URL if available
        let relay_url = endpoint_addr
            .relay_urls()
            .next()
            .map(|url| url.to_string());

        // Expire in 24 hours
        let expires = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400;

        let payload = InvitePayload {
            public_key: self.my_pubkey,
            node_id: node_id_bytes,
            addrs,
            relay_url,
            expires,
        };

        InviteCode::encode(&payload)
    }

    /// Send an encrypted message to a peer via iroh P2P.
    ///
    /// - Stores the message locally as "pending"
    /// - Looks up the peer's public key from contacts using `chat_id`
    /// - Derives a shared secret via X25519 DH
    /// - Encrypts the plaintext and creates a `WireMessage::ChatMessage`
    /// - Sends it over iroh to `peer_node_id`
    /// - Updates status to "sent" on success
    pub async fn send_message(
        &self,
        node: &IrohNode,
        chat_id: &str,
        text: &str,
        peer_node_id: iroh::EndpointId,
    ) -> Result<Message> {
        // 1. Store message locally (as pending)
        let msg = self.store_outgoing_message(chat_id, text)?;

        // 2. Get peer's public key from contacts by finding which chat this is
        //    We look up the chat to get peer_key, then fetch the contact
        let peer_key = self.get_peer_key_for_chat(chat_id)?;

        // 3. Derive shared secret: our X25519 secret key + peer X25519 public key
        let peer_x25519_pub = keypair::x25519_public_from_bytes(&peer_key)?;
        let shared = self.my_x25519_secret.diffie_hellman(&peer_x25519_pub);

        // 4. Derive symmetric key
        let sym_key = e2e::derive_symmetric_key(shared.as_bytes(), &self.my_pubkey, &peer_key);

        // 5. Encrypt the text (use message id as AAD)
        let (ciphertext, nonce) = e2e::encrypt(&sym_key, text.as_bytes(), msg.id.as_bytes())?;

        // 6. Create WireMessage::ChatMessage
        let message_uuid = Uuid::parse_str(&msg.id)
            .map_err(|e| crate::Error::Chat(format!("invalid uuid: {e}")))?;
        let wire_msg = WireMessage::ChatMessage {
            id: message_uuid,
            ciphertext,
            nonce,
            timestamp: msg.created_at as u64,
        };

        // 7. Send via node
        node.send_to_peer(peer_node_id, &wire_msg).await?;

        // 8. Update message status to "sent"
        self.update_status(&msg.id, MessageStatus::Sent)?;

        Ok(msg)
    }

    /// Get the peer's Ed25519 public key for a given chat_id.
    fn get_peer_key_for_chat(&self, chat_id: &str) -> Result<[u8; 32]> {
        let peer_key_bytes: Vec<u8> = self.db.conn()
            .query_row(
                "SELECT peer_key FROM chats WHERE id = ?1",
                rusqlite::params![chat_id],
                |row| row.get(0),
            )
            .map_err(|e| crate::Error::Chat(format!("chat not found: {e}")))?;

        peer_key_bytes
            .try_into()
            .map_err(|_| crate::Error::Chat("invalid peer key length".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::StaticSecret as X25519Secret;

    fn make_x25519_secret(seed: u8) -> X25519Secret {
        // Build from fixed bytes for determinism
        let bytes = [seed; 32];
        X25519Secret::from(bytes)
    }

    fn setup() -> ChatManager {
        let db = Database::open_in_memory().unwrap();
        let secret = make_x25519_secret(0);
        ChatManager::new(db, [0u8; 32], secret)
    }

    #[test]
    fn test_add_contact_and_chat() {
        let mgr = setup();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Alice").unwrap();
        let chats = mgr.get_chats().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].id, chat_id);
        assert_eq!(chats[0].peer_nickname, "Alice");
    }

    #[test]
    fn test_send_and_receive_messages() {
        let mgr = setup();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Bob").unwrap();
        mgr.store_outgoing_message(&chat_id, "Hello Bob!").unwrap();
        mgr.store_incoming_message(&chat_id, &peer, "Hi there!").unwrap();
        let msgs = mgr.get_messages(&chat_id, 10, None).unwrap();
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].is_mine);
        assert_eq!(msgs[0].content, "Hello Bob!");
        assert!(!msgs[1].is_mine);
        assert_eq!(msgs[1].content, "Hi there!");
    }

    #[test]
    fn test_event_broadcast() {
        let mgr = setup();
        let mut rx = mgr.subscribe();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Eve").unwrap();
        mgr.store_outgoing_message(&chat_id, "Test").unwrap();
        let event = rx.try_recv().unwrap();
        match event {
            ChatEvent::NewMessage { message, .. } => assert_eq!(message.content, "Test"),
            _ => panic!("expected NewMessage event"),
        }
    }

    #[test]
    fn test_update_status() {
        let mgr = setup();
        let peer = [1u8; 32];
        let chat_id = mgr.add_contact_and_chat(&peer, "Dave").unwrap();
        let msg = mgr.store_outgoing_message(&chat_id, "Status test").unwrap();
        mgr.update_status(&msg.id, MessageStatus::Delivered).unwrap();
        let msgs = mgr.get_messages(&chat_id, 10, None).unwrap();
        assert_eq!(msgs[0].status, MessageStatus::Delivered);
    }
}
