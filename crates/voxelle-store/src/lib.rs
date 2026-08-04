use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use voxelle_core::{
    compute_heads, derive_identity_state, identity_proof_extends, AcceptedEvent, EventV1,
    IdentityProofV1,
};

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
                PRAGMA busy_timeout = 5000;

                CREATE TABLE IF NOT EXISTS accepted_events (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    room_id TEXT NOT NULL,
                    event_json TEXT NOT NULL,
                    accepted_at_ms INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_accepted_events_room_id
                ON accepted_events(room_id);

                CREATE TABLE IF NOT EXISTS identity_heads (
                    peer_id TEXT PRIMARY KEY NOT NULL,
                    sequence INTEGER NOT NULL,
                    head TEXT NOT NULL,
                    proof_json TEXT NOT NULL
                );
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
        let candidate_proof = &event.delegation.identity_proof;
        let candidate_state = derive_identity_state(candidate_proof)
            .context("derive accepted event identity state")?;
        if candidate_state.peer_id != event.author_peer_id {
            anyhow::bail!("accepted event author does not match identity proof");
        }
        if let Some(known) = self.latest_identity_proof(&event.author_peer_id)? {
            if !identity_proof_extends(&known, candidate_proof)? {
                anyhow::bail!("accepted event carries a stale or forked identity proof");
            }
        }
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
        if changed == 1 {
            let proof_json =
                serde_json::to_string(candidate_proof).context("serialize identity proof")?;
            self.conn
                .execute(
                    r#"
                    INSERT INTO identity_heads (peer_id, sequence, head, proof_json)
                    VALUES (?1, ?2, ?3, ?4)
                    ON CONFLICT(peer_id) DO UPDATE SET
                        sequence = excluded.sequence,
                        head = excluded.head,
                        proof_json = excluded.proof_json
                    WHERE excluded.sequence > identity_heads.sequence
                    "#,
                    params![
                        candidate_state.peer_id,
                        candidate_state.sequence,
                        candidate_state.head,
                        proof_json
                    ],
                )
                .context("advance identity head")?;
        }
        Ok(changed == 1)
    }

    pub fn latest_identity_proof(&self, peer_id: &str) -> Result<Option<IdentityProofV1>> {
        self.conn
            .query_row(
                "SELECT proof_json FROM identity_heads WHERE peer_id = ?1",
                params![peer_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("load identity head")?
            .map(|json| serde_json::from_str(&json).context("parse identity proof"))
            .transpose()
    }

    pub fn has_event(&self, event_id: &str) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM accepted_events WHERE event_id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .context("check event existence")?;
        Ok(count > 0)
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

    pub fn room_event_count(&self, room_id: &str) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM accepted_events WHERE room_id = ?1",
                params![room_id],
                |row| row.get(0),
            )
            .context("count room events")?;
        Ok(count as usize)
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
        create_delegation(identity, 900, 2_000, scopes).expect("delegation")
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
                "peer_id": identity.peer_id,
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
        let context = RoomContext::new(authority.peer_id);
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
    fn recovered_identity_head_rejects_later_events_from_lost_device() {
        let authority = PeerIdentity::generate().expect("authority");
        let original = PeerIdentity::generate_at(900).expect("original");
        let context = RoomContext::new(authority.peer_id);
        let store = Store::open_in_memory().expect("store");

        let join = member_join(&original);
        let accepted_join = accept_event(&join, &[], &context, 1_000).expect("join accepted");
        store
            .insert_accepted_event(accepted_join, 1_000)
            .expect("insert join");

        let lost_device_event = message(&original, 1_300, vec![]);
        let accepted_before_recovery =
            accept_event(&lost_device_event, &[join.clone()], &context, 1_300)
                .expect("old device was valid before recovery");

        let recovered = PeerIdentity::recover(&original.recovery_card(), &original.proof, 1_100)
            .expect("recover");
        let recovered_event = message(&recovered, 1_200, vec![]);
        let accepted_recovered =
            accept_event(&recovered_event, &[join], &context, 1_200).expect("recovered event");
        store
            .insert_accepted_event(accepted_recovered, 1_200)
            .expect("advance identity head");

        let err = store
            .insert_accepted_event(accepted_before_recovery, 1_300)
            .expect_err("lost device rejected after recovery");
        assert!(err.to_string().contains("stale or forked identity proof"));
    }

    #[test]
    fn accepted_events_survive_reopen_and_heads_are_stable() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("voxelle.sqlite3");

        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id);
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
        let context = RoomContext::new(authority.peer_id);
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
