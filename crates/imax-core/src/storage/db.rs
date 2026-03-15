use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use crate::Result;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let mut conn = Connection::open(path)
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        migrations()
            .to_latest(&mut conn)
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        migrations()
            .to_latest(&mut conn)
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE identity (
                id            INTEGER PRIMARY KEY CHECK (id = 1),
                seed_encrypted BLOB NOT NULL,
                seed_nonce    BLOB NOT NULL,
                public_key    BLOB NOT NULL,
                nickname      TEXT NOT NULL,
                created_at    INTEGER NOT NULL
            );
            CREATE TABLE contacts (
                public_key    BLOB PRIMARY KEY,
                nickname      TEXT NOT NULL,
                node_id       BLOB,
                added_at      INTEGER NOT NULL,
                last_seen     INTEGER
            );
            CREATE TABLE chats (
                id            TEXT PRIMARY KEY,
                peer_key      BLOB NOT NULL UNIQUE,
                created_at    INTEGER NOT NULL,
                last_message  TEXT,
                unread_count  INTEGER DEFAULT 0
            );
            CREATE TABLE messages (
                id            TEXT PRIMARY KEY,
                chat_id       TEXT NOT NULL,
                sender_key    BLOB NOT NULL,
                content       TEXT NOT NULL,
                seq           INTEGER NOT NULL,
                status        TEXT NOT NULL CHECK (status IN ('pending','sent','delivered','read')),
                created_at    INTEGER NOT NULL,
                received_at   INTEGER
            );
            CREATE TABLE pending_invites (
                code          TEXT PRIMARY KEY,
                created_at    INTEGER NOT NULL,
                expires_at    INTEGER
            );
            CREATE INDEX idx_messages_chat ON messages(chat_id, created_at);
            CREATE INDEX idx_messages_status ON messages(status) WHERE status = 'pending';"
        ),
        M::up(
            "DROP TABLE IF EXISTS pending_invites;
            DROP TABLE IF EXISTS messages;
            DROP TABLE IF EXISTS contacts;
            DROP TABLE IF EXISTS chats;
            DROP TABLE IF EXISTS identity;

            CREATE TABLE identity (
                id           INTEGER PRIMARY KEY CHECK (id = 1),
                seed_phrase  TEXT NOT NULL,
                nickname     TEXT NOT NULL,
                created_at   INTEGER NOT NULL
            );

            CREATE TABLE chats (
                id            TEXT PRIMARY KEY,
                peer_name     TEXT NOT NULL DEFAULT '',
                last_message  TEXT NOT NULL DEFAULT '',
                time          TEXT NOT NULL DEFAULT '',
                avatar_color  INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE messages (
                id        TEXT PRIMARY KEY,
                chat_id   TEXT NOT NULL,
                content   TEXT NOT NULL,
                is_mine   INTEGER NOT NULL DEFAULT 0,
                time      TEXT NOT NULL DEFAULT '',
                status    TEXT NOT NULL DEFAULT 'sent',
                seq       INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX idx_messages_chat_v2 ON messages(chat_id, seq);"
        ),
        // V3: Multi-profile support
        M::up(
            "DROP TABLE IF EXISTS messages;
            DROP TABLE IF EXISTS chats;
            DROP TABLE IF EXISTS identity;

            CREATE TABLE profiles (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                seed_phrase TEXT NOT NULL,
                nickname    TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                is_active   INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE chats (
                id           TEXT NOT NULL,
                profile_id   INTEGER NOT NULL,
                peer_name    TEXT NOT NULL DEFAULT '',
                last_message TEXT NOT NULL DEFAULT '',
                time         TEXT NOT NULL DEFAULT '',
                avatar_color INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (id, profile_id),
                FOREIGN KEY (profile_id) REFERENCES profiles(id)
            );

            CREATE TABLE messages (
                id         TEXT PRIMARY KEY,
                chat_id    TEXT NOT NULL,
                profile_id INTEGER NOT NULL,
                content    TEXT NOT NULL,
                is_mine    INTEGER NOT NULL DEFAULT 0,
                time       TEXT NOT NULL DEFAULT '',
                status     TEXT NOT NULL DEFAULT 'sent',
                seq        INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (profile_id) REFERENCES profiles(id)
            );

            CREATE INDEX idx_messages_chat_v3 ON messages(chat_id, profile_id, seq);
            CREATE INDEX idx_chats_profile ON chats(profile_id);"
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory().unwrap();
        let count: i32 = db.conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('profiles','chats','messages')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }
}
