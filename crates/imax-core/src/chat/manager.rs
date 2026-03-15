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
    profile_id: i64,
}

impl ChatManager {
    pub fn new(db: Database, my_pubkey: [u8; 32], my_x25519_secret: X25519Secret, profile_id: i64) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self { db, my_pubkey, my_x25519_secret, event_tx, profile_id }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent> {
        self.event_tx.subscribe()
    }

    pub fn get_chats(&self) -> Result<Vec<ChatPreview>> {
        let rows = models::get_all_chats(&self.db, self.profile_id)?;
        Ok(rows.into_iter().map(|r| ChatPreview {
            id: r.id,
            peer_key: [0u8; 32],
            peer_nickname: r.peer_name,
            last_message_text: if r.last_message.is_empty() { None } else { Some(r.last_message) },
            last_message_time: None,
            unread_count: 0,
            is_online: false,
        }).collect())
    }

    pub fn get_messages(&self, chat_id: &str, _limit: usize, _before_seq: Option<i64>) -> Result<Vec<Message>> {
        let rows = models::get_messages_for_chat(&self.db, chat_id, self.profile_id)?;
        Ok(rows.into_iter().map(|r| Message {
            id: r.id,
            chat_id: r.chat_id,
            sender_key: if r.is_mine { self.my_pubkey } else { [0u8; 32] },
            content: r.content,
            seq: r.seq,
            status: MessageStatus::from_str(&r.status),
            created_at: 0,
            is_mine: r.is_mine,
        }).collect())
    }

    pub fn add_contact_and_chat(&self, _peer_key: &[u8; 32], nickname: &str) -> Result<ChatId> {
        let id = Uuid::new_v4().to_string();
        models::upsert_chat(&self.db, &id, self.profile_id, nickname, "", "", 0)?;
        Ok(id)
    }

    pub fn store_outgoing_message(&self, chat_id: &str, text: &str) -> Result<Message> {
        let id = Uuid::new_v4().to_string();
        models::insert_message(&self.db, &id, chat_id, self.profile_id, text, true, "", "pending")?;
        let msg = Message {
            id: id.clone(), chat_id: chat_id.to_string(), sender_key: self.my_pubkey,
            content: text.to_string(), seq: 0, status: MessageStatus::Pending,
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            is_mine: true,
        };
        let _ = self.event_tx.send(ChatEvent::NewMessage { chat_id: chat_id.to_string(), message: msg.clone() });
        Ok(msg)
    }

    pub fn store_incoming_message(&self, chat_id: &str, sender_key: &[u8; 32], text: &str) -> Result<Message> {
        let id = Uuid::new_v4().to_string();
        models::insert_message(&self.db, &id, chat_id, self.profile_id, text, false, "", "delivered")?;
        let msg = Message {
            id: id.clone(), chat_id: chat_id.to_string(), sender_key: *sender_key,
            content: text.to_string(), seq: 0, status: MessageStatus::Delivered,
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            is_mine: false,
        };
        let _ = self.event_tx.send(ChatEvent::NewMessage { chat_id: chat_id.to_string(), message: msg.clone() });
        Ok(msg)
    }

    pub fn update_status(&self, message_id: &str, status: MessageStatus) -> Result<()> {
        self.db.conn().execute(
            "UPDATE messages SET status = ?1 WHERE id = ?2",
            rusqlite::params![status.as_str(), message_id],
        ).map_err(|e| crate::Error::Storage(e.to_string()))?;
        let _ = self.event_tx.send(ChatEvent::MessageStatusChanged { message_id: message_id.to_string(), status });
        Ok(())
    }

    pub fn db(&self) -> &Database { &self.db }

    /// Generate an invite code containing our iroh endpoint info.
    pub fn generate_invite(&self, node: &IrohNode) -> Result<InviteCode> {
        let endpoint_addr = node.endpoint().addr();
        let node_id_bytes = node.node_id().as_bytes().clone();

        let addrs: Vec<std::net::SocketAddr> = endpoint_addr
            .ip_addrs()
            .cloned()
            .collect();

        let relay_url = endpoint_addr
            .relay_urls()
            .next()
            .map(|url| url.to_string());

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
    pub async fn send_message(
        &self,
        node: &IrohNode,
        chat_id: &str,
        text: &str,
        peer_node_id: iroh::EndpointId,
        peer_key: &[u8; 32],
    ) -> Result<Message> {
        let msg = self.store_outgoing_message(chat_id, text)?;

        let peer_x25519_pub = keypair::x25519_public_from_bytes(peer_key)?;
        let shared = self.my_x25519_secret.diffie_hellman(&peer_x25519_pub);

        let sym_key = e2e::derive_symmetric_key(shared.as_bytes(), &self.my_pubkey, peer_key);

        let (ciphertext, nonce) = e2e::encrypt(&sym_key, text.as_bytes(), msg.id.as_bytes())?;

        let message_uuid = Uuid::parse_str(&msg.id)
            .map_err(|e| crate::Error::Chat(format!("invalid uuid: {e}")))?;
        let wire_msg = WireMessage::ChatMessage {
            id: message_uuid,
            ciphertext,
            nonce,
            timestamp: msg.created_at as u64,
        };

        node.send_to_peer(peer_node_id, &wire_msg).await?;

        self.update_status(&msg.id, MessageStatus::Sent)?;

        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::StaticSecret as X25519Secret;

    fn make_x25519_secret(seed: u8) -> X25519Secret {
        let bytes = [seed; 32];
        X25519Secret::from(bytes)
    }

    fn setup() -> ChatManager {
        let db = Database::open_in_memory().unwrap();
        let profile_id = models::create_profile(&db, "test-seed", "Test").unwrap();
        let secret = make_x25519_secret(0);
        ChatManager::new(db, [0u8; 32], secret, profile_id)
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
