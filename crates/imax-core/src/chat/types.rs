use serde::{Deserialize, Serialize};

pub type ChatId = String;
pub type MessageId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPreview {
    pub id: ChatId,
    pub peer_key: [u8; 32],
    pub peer_nickname: String,
    pub last_message_text: Option<String>,
    pub last_message_time: Option<i64>,
    pub unread_count: i32,
    pub is_online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub sender_key: [u8; 32],
    pub content: String,
    pub seq: i64,
    pub status: MessageStatus,
    pub created_at: i64,
    pub is_mine: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Read,
}

impl MessageStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "sent" => Self::Sent,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatEvent {
    NewMessage { chat_id: ChatId, message: Message },
    MessageStatusChanged { message_id: MessageId, status: MessageStatus },
    PeerOnline { public_key: [u8; 32] },
    PeerOffline { public_key: [u8; 32] },
    InviteAccepted { chat_id: ChatId, peer_nickname: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_status_roundtrip() {
        for status in [MessageStatus::Pending, MessageStatus::Sent, MessageStatus::Delivered, MessageStatus::Read] {
            assert_eq!(MessageStatus::from_str(status.as_str()), status);
        }
    }
}
