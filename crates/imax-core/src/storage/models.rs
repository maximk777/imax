use rusqlite::params;
use uuid::Uuid;
use crate::storage::db::Database;
use crate::Result;

pub fn save_identity(db: &Database, seed_encrypted: &[u8], seed_nonce: &[u8], public_key: &[u8; 32], nickname: &str) -> Result<()> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT OR REPLACE INTO identity (id, seed_encrypted, seed_nonce, public_key, nickname, created_at) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
        params![seed_encrypted, seed_nonce, public_key.as_slice(), nickname, now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_identity(db: &Database) -> Result<Option<(Vec<u8>, Vec<u8>, Vec<u8>, String)>> {
    let mut stmt = db.conn().prepare("SELECT seed_encrypted, seed_nonce, public_key, nickname FROM identity WHERE id = 1")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let result = stmt.query_row([], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, Vec<u8>>(2)?, row.get::<_, String>(3)?))
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::Error::Storage(e.to_string())),
    }
}

pub fn insert_contact(db: &Database, public_key: &[u8; 32], nickname: &str, node_id: Option<&[u8]>) -> Result<()> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT OR REPLACE INTO contacts (public_key, nickname, node_id, added_at) VALUES (?1, ?2, ?3, ?4)",
        params![public_key.as_slice(), nickname, node_id, now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_contact(db: &Database, public_key: &[u8; 32]) -> Result<Option<(Vec<u8>, String, Option<Vec<u8>>)>> {
    let mut stmt = db.conn().prepare("SELECT public_key, nickname, node_id FROM contacts WHERE public_key = ?1")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let result = stmt.query_row(params![public_key.as_slice()], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<Vec<u8>>>(2)?))
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::Error::Storage(e.to_string())),
    }
}

pub fn create_chat(db: &Database, peer_key: &[u8; 32]) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT INTO chats (id, peer_key, created_at) VALUES (?1, ?2, ?3)",
        params![id, peer_key.as_slice(), now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(id)
}

pub fn get_chats(db: &Database) -> Result<Vec<(String, Vec<u8>, i64, Option<String>, i32)>> {
    let mut stmt = db.conn().prepare("SELECT id, peer_key, created_at, last_message, unread_count FROM chats ORDER BY created_at DESC")
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, i64>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, i32>(4)?))
    }).map_err(|e| crate::Error::Storage(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| crate::Error::Storage(e.to_string()))
}

pub fn insert_message(db: &Database, chat_id: &str, sender_key: &[u8; 32], content: &str, seq: i64, status: &str) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    db.conn().execute(
        "INSERT INTO messages (id, chat_id, sender_key, content, seq, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, chat_id, sender_key.as_slice(), content, seq, status, now],
    ).map_err(|e| crate::Error::Storage(e.to_string()))?;
    db.conn().execute("UPDATE chats SET last_message = ?1 WHERE id = ?2", params![id, chat_id])
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(id)
}

pub fn get_messages(db: &Database, chat_id: &str, limit: usize, before_seq: Option<i64>) -> Result<Vec<(String, Vec<u8>, String, i64, String, i64)>> {
    match before_seq {
        Some(seq) => {
            let mut stmt = db.conn().prepare("SELECT id, sender_key, content, seq, status, created_at FROM messages WHERE chat_id = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3")
                .map_err(|e| crate::Error::Storage(e.to_string()))?;
            let rows = stmt.query_map(params![chat_id, seq, limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
            }).map_err(|e| crate::Error::Storage(e.to_string()))?;
            let mut result = rows.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| crate::Error::Storage(e.to_string()))?;
            result.reverse();
            Ok(result)
        }
        None => {
            let mut stmt = db.conn().prepare("SELECT id, sender_key, content, seq, status, created_at FROM messages WHERE chat_id = ?1 ORDER BY seq ASC LIMIT ?2")
                .map_err(|e| crate::Error::Storage(e.to_string()))?;
            let rows = stmt.query_map(params![chat_id, limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
            }).map_err(|e| crate::Error::Storage(e.to_string()))?;
            rows.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| crate::Error::Storage(e.to_string()))
        }
    }
}

pub fn update_message_status(db: &Database, message_id: &str, status: &str) -> Result<()> {
    db.conn().execute("UPDATE messages SET status = ?1 WHERE id = ?2", params![status, message_id])
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_next_seq(db: &Database, chat_id: &str) -> Result<i64> {
    let max_seq: Option<i64> = db.conn()
        .query_row("SELECT MAX(seq) FROM messages WHERE chat_id = ?1", params![chat_id], |r| r.get(0))
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(max_seq.unwrap_or(0) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database { Database::open_in_memory().unwrap() }

    #[test]
    fn test_save_and_get_identity() {
        let db = setup();
        save_identity(&db, b"encrypted_seed", b"nonce_bytes_here", &[1u8; 32], "Max").unwrap();
        let identity = get_identity(&db).unwrap().unwrap();
        assert_eq!(identity.3, "Max");
        assert_eq!(identity.2, vec![1u8; 32]);
    }

    #[test]
    fn test_insert_and_get_contact() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let contact = get_contact(&db, &pk).unwrap().unwrap();
        assert_eq!(contact.1, "Alice");
    }

    #[test]
    fn test_create_and_get_chats() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        let chats = get_chats(&db).unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].0, chat_id);
    }

    #[test]
    fn test_insert_and_get_messages() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        insert_message(&db, &chat_id, &pk, "Hello!", 1, "sent").unwrap();
        insert_message(&db, &chat_id, &pk, "World!", 2, "sent").unwrap();
        let msgs = get_messages(&db, &chat_id, 10, None).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].2, "Hello!");
        assert_eq!(msgs[1].2, "World!");
    }

    #[test]
    fn test_update_message_status() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        let msg_id = insert_message(&db, &chat_id, &pk, "Hi", 1, "pending").unwrap();
        update_message_status(&db, &msg_id, "delivered").unwrap();
        let msgs = get_messages(&db, &chat_id, 10, None).unwrap();
        assert_eq!(msgs[0].4, "delivered");
    }

    #[test]
    fn test_get_next_seq() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        assert_eq!(get_next_seq(&db, &chat_id).unwrap(), 1);
        insert_message(&db, &chat_id, &pk, "Hi", 1, "sent").unwrap();
        assert_eq!(get_next_seq(&db, &chat_id).unwrap(), 2);
    }

    #[test]
    fn test_get_messages_pagination() {
        let db = setup();
        let pk = [1u8; 32];
        insert_contact(&db, &pk, "Alice", None).unwrap();
        let chat_id = create_chat(&db, &pk).unwrap();
        for i in 1..=5 {
            insert_message(&db, &chat_id, &pk, &format!("msg {i}"), i, "sent").unwrap();
        }
        let msgs = get_messages(&db, &chat_id, 2, Some(4)).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].2, "msg 2");
        assert_eq!(msgs[1].2, "msg 3");
    }
}
