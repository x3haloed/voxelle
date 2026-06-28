use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use voxelle_core::{compute_heads, AcceptedEvent, EventV1};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("open SQLite store")?;
        let store = Self { conn };
        store.init()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory SQLite store")?;
        let store = Self { conn };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;

                CREATE TABLE IF NOT EXISTS accepted_events (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    room_id TEXT NOT NULL,
                    event_json TEXT NOT NULL,
                    accepted_at_ms INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_accepted_events_room_id
                ON accepted_events(room_id);
                "#,
            )
            .context("initialize store schema")?;
        Ok(())
    }

    pub fn insert_accepted_event(
        &self,
        accepted: AcceptedEvent<'_>,
        accepted_at_ms: i64,
    ) -> Result<bool> {
        let event = accepted.event();
        let event_json = serde_json::to_string(event).context("serialize event")?;
        let changed = self
            .conn
            .execute(
                r#"
                INSERT OR IGNORE INTO accepted_events
                    (event_id, room_id, event_json, accepted_at_ms)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![event.event_id, event.room_id, event_json, accepted_at_ms],
            )
            .context("insert accepted event")?;
        Ok(changed == 1)
    }

    pub fn get_event(&self, event_id: &str) -> Result<Option<EventV1>> {
        self.conn
            .query_row(
                "SELECT event_json FROM accepted_events WHERE event_id = ?1",
                params![event_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("load event")?
            .map(|json| serde_json::from_str(&json).context("parse stored event"))
            .transpose()
    }

    pub fn room_events(&self, room_id: &str) -> Result<Vec<EventV1>> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT event_json
                FROM accepted_events
                WHERE room_id = ?1
                ORDER BY accepted_at_ms ASC, event_id ASC
                "#,
            )
            .context("prepare room event query")?;

        let rows = stmt
            .query_map(params![room_id], |row| row.get::<_, String>(0))
            .context("query room events")?;

        let mut events = Vec::new();
        for row in rows {
            let json = row.context("read room event row")?;
            events.push(serde_json::from_str(&json).context("parse stored room event")?);
        }
        Ok(events)
    }

    pub fn room_heads(&self, room_id: &str) -> Result<Vec<String>> {
        Ok(compute_heads(&self.room_events(room_id)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;
    use voxelle_core::{
        accept_event, create_delegation, create_event, PeerIdentity, RoomContext,
        GOVERNANCE_ROOM_ID,
    };

    fn delegation_for(
        identity: &PeerIdentity,
        scopes: Vec<String>,
    ) -> voxelle_core::DelegationCertV1 {
        create_delegation(&identity.peer, &identity.device, 900, 2_000, scopes).expect("delegation")
    }

    fn member_join(identity: &PeerIdentity) -> EventV1 {
        create_event(
            identity,
            delegation_for(identity, vec!["room:join".to_string()]),
            GOVERNANCE_ROOM_ID,
            1_000,
            "MEMBER_JOIN",
            vec![],
            json!({
                "peer_id": identity.peer.id,
                "peer_pub": identity.peer.spki_b64,
            }),
        )
        .expect("member join")
    }

    fn message(identity: &PeerIdentity, created_ms: i64, parents: Vec<String>) -> EventV1 {
        create_event(
            identity,
            delegation_for(identity, vec!["room:post".to_string()]),
            "room:general",
            created_ms,
            "MSG_POST",
            parents,
            json!({ "text": "hello" }),
        )
        .expect("message")
    }

    #[test]
    fn accepted_event_insert_is_idempotent() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id);
        let join = member_join(&member);
        let accepted = accept_event(&join, &[], &context, 1_000).expect("accepted");

        let store = Store::open_in_memory().expect("store");
        assert!(store
            .insert_accepted_event(accepted, 1_000)
            .expect("insert"));
        let accepted_again = accept_event(&join, &[], &context, 1_000).expect("accepted");
        assert!(!store
            .insert_accepted_event(accepted_again, 1_001)
            .expect("idempotent insert"));
    }

    #[test]
    fn accepted_events_survive_reopen_and_heads_are_stable() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("voxelle.sqlite3");

        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id);
        let join = member_join(&member);
        let msg = message(&member, 1_100, vec![]);

        {
            let store = Store::open(&path).expect("store");
            let accepted_join = accept_event(&join, &[], &context, 1_000).expect("join accepted");
            store
                .insert_accepted_event(accepted_join, 1_000)
                .expect("insert join");
            let accepted_msg =
                accept_event(&msg, &[join.clone()], &context, 1_100).expect("msg accepted");
            store
                .insert_accepted_event(accepted_msg, 1_100)
                .expect("insert msg");
        }

        let reopened = Store::open(&path).expect("reopen");
        assert_eq!(
            reopened
                .get_event(&msg.event_id)
                .expect("load event")
                .expect("present")
                .event_id,
            msg.event_id
        );
        assert_eq!(
            reopened.room_events("room:general").expect("events").len(),
            1
        );
        assert_eq!(
            reopened.room_heads("room:general").expect("heads"),
            vec![msg.event_id]
        );
    }

    #[test]
    fn dependent_room_heads_ignore_known_parents() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id);
        let join = member_join(&member);
        let root = message(&member, 1_100, vec![]);
        let child = message(&member, 1_200, vec![root.event_id.clone()]);
        let store = Store::open_in_memory().expect("store");

        let accepted_join = accept_event(&join, &[], &context, 1_000).expect("join accepted");
        store
            .insert_accepted_event(accepted_join, 1_000)
            .expect("insert join");
        let accepted_root =
            accept_event(&root, &[join.clone()], &context, 1_100).expect("root accepted");
        store
            .insert_accepted_event(accepted_root, 1_100)
            .expect("insert root");
        let accepted_child =
            accept_event(&child, &[join], &context, 1_200).expect("child accepted");
        store
            .insert_accepted_event(accepted_child, 1_200)
            .expect("insert child");

        assert_eq!(
            store.room_heads("room:general").expect("heads"),
            vec![child.event_id]
        );
    }
}
