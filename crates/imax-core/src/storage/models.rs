use rusqlite::params;
use crate::storage::db::Database;
use crate::Result;

// ── Row structs ──

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileRow {
    pub id: i64,
    pub seed_phrase: String,
    pub nickname: String,
    pub created_at: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct ChatRow {
    pub id: String,
    pub peer_name: String,
    pub last_message: String,
    pub time: String,
    pub avatar_color: i32,
    pub peer_node_id: Option<Vec<u8>>,
    pub peer_pubkey: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub chat_id: String,
    pub content: String,
    pub is_mine: bool,
    pub time: String,
    pub status: String,
    pub seq: i64,
}

// ── Profiles ──

pub fn create_profile(db: &Database, seed_phrase: &str, nickname: &str) -> Result<i64> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    db.conn()
        .execute(
            "INSERT INTO profiles (seed_phrase, nickname, created_at, is_active) VALUES (?1, ?2, ?3, 0)",
            params![seed_phrase, nickname, now],
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_profile(db: &Database, id: i64) -> Result<Option<ProfileRow>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, seed_phrase, nickname, created_at, is_active FROM profiles WHERE id = ?1")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let result = stmt.query_row(params![id], |row| {
        Ok(ProfileRow {
            id: row.get(0)?,
            seed_phrase: row.get(1)?,
            nickname: row.get(2)?,
            created_at: row.get(3)?,
            is_active: row.get::<_, i32>(4)? != 0,
        })
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::Error::Storage(e.to_string())),
    }
}

pub fn get_all_profiles(db: &Database) -> Result<Vec<ProfileRow>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, seed_phrase, nickname, created_at, is_active FROM profiles ORDER BY created_at ASC")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ProfileRow {
                id: row.get(0)?,
                seed_phrase: row.get(1)?,
                nickname: row.get(2)?,
                created_at: row.get(3)?,
                is_active: row.get::<_, i32>(4)? != 0,
            })
        })
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::Error::Storage(e.to_string()))
}

pub fn get_active_profile(db: &Database) -> Result<Option<ProfileRow>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, seed_phrase, nickname, created_at, is_active FROM profiles WHERE is_active = 1")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let result = stmt.query_row([], |row| {
        Ok(ProfileRow {
            id: row.get(0)?,
            seed_phrase: row.get(1)?,
            nickname: row.get(2)?,
            created_at: row.get(3)?,
            is_active: row.get::<_, i32>(4)? != 0,
        })
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::Error::Storage(e.to_string())),
    }
}

pub fn set_active_profile(db: &Database, profile_id: i64) -> Result<()> {
    db.conn()
        .execute("UPDATE profiles SET is_active = 0", [])
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    db.conn()
        .execute(
            "UPDATE profiles SET is_active = 1 WHERE id = ?1",
            params![profile_id],
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn delete_profile(db: &Database, profile_id: i64) -> Result<()> {
    db.conn()
        .execute("DELETE FROM messages WHERE profile_id = ?1", params![profile_id])
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    db.conn()
        .execute("DELETE FROM chats WHERE profile_id = ?1", params![profile_id])
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    db.conn()
        .execute("DELETE FROM profiles WHERE id = ?1", params![profile_id])
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

// ── Chats (profile-scoped) ──

pub fn upsert_chat(
    db: &Database,
    id: &str,
    profile_id: i64,
    peer_name: &str,
    last_message: &str,
    time: &str,
    avatar_color: i32,
    peer_node_id: Option<&[u8]>,
    peer_pubkey: Option<&[u8]>,
) -> Result<()> {
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO chats (id, profile_id, peer_name, last_message, time, avatar_color, peer_node_id, peer_pubkey) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, profile_id, peer_name, last_message, time, avatar_color, peer_node_id, peer_pubkey],
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_all_chats(db: &Database, profile_id: i64) -> Result<Vec<ChatRow>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, peer_name, last_message, time, avatar_color, peer_node_id, peer_pubkey FROM chats WHERE profile_id = ?1")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let rows = stmt
        .query_map(params![profile_id], |row| {
            Ok(ChatRow {
                id: row.get(0)?,
                peer_name: row.get(1)?,
                last_message: row.get(2)?,
                time: row.get(3)?,
                avatar_color: row.get(4)?,
                peer_node_id: row.get(5)?,
                peer_pubkey: row.get(6)?,
            })
        })
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::Error::Storage(e.to_string()))
}

pub fn update_chat_preview(db: &Database, id: &str, profile_id: i64, last_message: &str) -> Result<()> {
    db.conn()
        .execute(
            "UPDATE chats SET last_message = ?1 WHERE id = ?2 AND profile_id = ?3",
            params![last_message, id, profile_id],
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

// ── Messages (profile-scoped) ──

pub fn insert_message(
    db: &Database,
    id: &str,
    chat_id: &str,
    profile_id: i64,
    content: &str,
    is_mine: bool,
    time: &str,
    status: &str,
) -> Result<()> {
    let seq: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM messages WHERE chat_id = ?1 AND profile_id = ?2",
            params![chat_id, profile_id],
            |r| r.get(0),
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    db.conn()
        .execute(
            "INSERT INTO messages (id, chat_id, profile_id, content, is_mine, time, status, seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, chat_id, profile_id, content, is_mine as i32, time, status, seq],
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_messages_for_chat(db: &Database, chat_id: &str, profile_id: i64) -> Result<Vec<MessageRow>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, chat_id, content, is_mine, time, status, seq FROM messages WHERE chat_id = ?1 AND profile_id = ?2 ORDER BY seq ASC",
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let rows = stmt
        .query_map(params![chat_id, profile_id], |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                content: row.get(2)?,
                is_mine: row.get::<_, i32>(3)? != 0,
                time: row.get(4)?,
                status: row.get(5)?,
                seq: row.get(6)?,
            })
        })
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::Error::Storage(e.to_string()))
}

pub fn update_message_status(db: &Database, message_id: &str, status: &str) -> Result<()> {
    db.conn()
        .execute(
            "UPDATE messages SET status = ?1 WHERE id = ?2",
            params![status, message_id],
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_create_and_get_profile() {
        let db = setup();
        let id = create_profile(&db, "word1 word2 word3", "Max").unwrap();
        let profile = get_profile(&db, id).unwrap().unwrap();
        assert_eq!(profile.seed_phrase, "word1 word2 word3");
        assert_eq!(profile.nickname, "Max");
        assert!(!profile.is_active);
    }

    #[test]
    fn test_set_active_profile() {
        let db = setup();
        let id1 = create_profile(&db, "seed1", "Alice").unwrap();
        let id2 = create_profile(&db, "seed2", "Bob").unwrap();
        set_active_profile(&db, id1).unwrap();
        assert!(get_profile(&db, id1).unwrap().unwrap().is_active);
        assert!(!get_profile(&db, id2).unwrap().unwrap().is_active);
        set_active_profile(&db, id2).unwrap();
        assert!(!get_profile(&db, id1).unwrap().unwrap().is_active);
        assert!(get_profile(&db, id2).unwrap().unwrap().is_active);
    }

    #[test]
    fn test_get_active_profile() {
        let db = setup();
        assert!(get_active_profile(&db).unwrap().is_none());
        let id = create_profile(&db, "seed1", "Alice").unwrap();
        set_active_profile(&db, id).unwrap();
        let active = get_active_profile(&db).unwrap().unwrap();
        assert_eq!(active.nickname, "Alice");
    }

    #[test]
    fn test_upsert_and_get_chats() {
        let db = setup();
        let pid = create_profile(&db, "seed", "User").unwrap();
        upsert_chat(&db, "chat-1", pid, "Alice", "Hello!", "12:00", 2, None, None).unwrap();
        upsert_chat(&db, "chat-2", pid, "Bob", "Hi!", "12:01", 1, None, None).unwrap();
        let chats = get_all_chats(&db, pid).unwrap();
        assert_eq!(chats.len(), 2);
    }

    #[test]
    fn test_chats_isolated_by_profile() {
        let db = setup();
        let p1 = create_profile(&db, "seed1", "Alice").unwrap();
        let p2 = create_profile(&db, "seed2", "Bob").unwrap();
        upsert_chat(&db, "chat-1", p1, "Peer1", "", "", 0, None, None).unwrap();
        upsert_chat(&db, "chat-2", p2, "Peer2", "", "", 0, None, None).unwrap();
        assert_eq!(get_all_chats(&db, p1).unwrap().len(), 1);
        assert_eq!(get_all_chats(&db, p2).unwrap().len(), 1);
    }

    #[test]
    fn test_update_chat_preview() {
        let db = setup();
        let pid = create_profile(&db, "seed", "User").unwrap();
        upsert_chat(&db, "chat-1", pid, "Alice", "Hello!", "12:00", 2, None, None).unwrap();
        update_chat_preview(&db, "chat-1", pid, "Bye!").unwrap();
        let chats = get_all_chats(&db, pid).unwrap();
        assert_eq!(chats[0].last_message, "Bye!");
    }

    #[test]
    fn test_insert_and_get_messages() {
        let db = setup();
        let pid = create_profile(&db, "seed", "User").unwrap();
        upsert_chat(&db, "chat-1", pid, "Alice", "", "", 0, None, None).unwrap();
        insert_message(&db, "m1", "chat-1", pid, "Hello!", true, "12:00", "sent").unwrap();
        insert_message(&db, "m2", "chat-1", pid, "Hi back!", false, "12:01", "sent").unwrap();
        let msgs = get_messages_for_chat(&db, "chat-1", pid).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "Hello!");
        assert!(msgs[0].is_mine);
        assert_eq!(msgs[1].content, "Hi back!");
        assert!(!msgs[1].is_mine);
        assert_eq!(msgs[0].seq, 1);
        assert_eq!(msgs[1].seq, 2);
    }

    #[test]
    fn test_delete_profile_cascades() {
        let db = setup();
        let pid = create_profile(&db, "seed", "User").unwrap();
        upsert_chat(&db, "chat-1", pid, "Alice", "", "", 0, None, None).unwrap();
        insert_message(&db, "m1", "chat-1", pid, "Hello!", true, "12:00", "sent").unwrap();
        delete_profile(&db, pid).unwrap();
        assert!(get_profile(&db, pid).unwrap().is_none());
        assert_eq!(get_all_chats(&db, pid).unwrap().len(), 0);
        assert_eq!(get_messages_for_chat(&db, "chat-1", pid).unwrap().len(), 0);
    }
}
