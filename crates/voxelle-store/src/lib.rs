use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use voxelle_core::{
    compute_heads, derive_identity_state, identity_proof_extends, AcceptedEvent, EventV1,
    IdentityProofV1,
};

pub struct Store {
    conn: Connection,
}

pub const MAX_RESIDENT_OBSERVATION_CONSUMERS: usize = 64;
pub const MAX_RESIDENT_OBSERVATION_ROOMS_PER_CONSUMER: usize = 512;
const MAX_CONSUMER_ID_BYTES: usize = 128;
const MAX_ROOM_ID_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct SequencedEvent {
    pub local_fact_sequence: u64,
    pub event: EventV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidentObservationStart {
    FromBeginning,
    FromNow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentObservationConsumer {
    pub consumer_id: String,
    pub start: ResidentObservationStart,
    pub start_fact_sequence: u64,
    pub created_ms: i64,
    pub updated_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentObservationCheckpoint {
    pub consumer_id: String,
    pub room_id: String,
    pub committed_fact_sequence: u64,
    pub updated_ms: i64,
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
                    local_fact_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_id TEXT UNIQUE NOT NULL,
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

                CREATE TABLE IF NOT EXISTS local_state (
                    key TEXT PRIMARY KEY NOT NULL,
                    value_json TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS resident_observation_consumers (
                    consumer_id TEXT PRIMARY KEY NOT NULL,
                    start_policy TEXT NOT NULL,
                    start_fact_sequence INTEGER NOT NULL,
                    created_ms INTEGER NOT NULL,
                    updated_ms INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS resident_observation_checkpoints (
                    consumer_id TEXT NOT NULL,
                    room_id TEXT NOT NULL,
                    committed_fact_sequence INTEGER NOT NULL,
                    updated_ms INTEGER NOT NULL,
                    PRIMARY KEY (consumer_id, room_id),
                    FOREIGN KEY (consumer_id)
                        REFERENCES resident_observation_consumers(consumer_id)
                        ON DELETE CASCADE
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
        let transaction = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .context("start accepted-event transaction")?;
        let known = transaction
            .query_row(
                "SELECT proof_json FROM identity_heads WHERE peer_id = ?1",
                params![event.author_peer_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("load identity head in transaction")?
            .map(|json| serde_json::from_str(&json).context("parse identity proof"))
            .transpose()?;
        if let Some(known) = known {
            if !identity_proof_extends(&known, candidate_proof)? {
                anyhow::bail!("accepted event carries a stale or forked identity proof");
            }
        }
        let already_present = transaction
            .query_row(
                "SELECT 1 FROM accepted_events WHERE event_id = ?1",
                params![event.event_id],
                |_| Ok(()),
            )
            .optional()
            .context("check accepted event in transaction")?
            .is_some();
        let changed = if already_present {
            false
        } else {
            let event_json = serde_json::to_string(event).context("serialize event")?;
            transaction
                .execute(
                    r#"
                INSERT INTO accepted_events
                    (event_id, room_id, event_json, accepted_at_ms)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                    params![event.event_id, event.room_id, event_json, accepted_at_ms],
                )
                .context("insert accepted event")?;
            true
        };
        let proof_json =
            serde_json::to_string(candidate_proof).context("serialize identity proof")?;
        transaction
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
        transaction
            .commit()
            .context("commit accepted event and identity head")?;
        Ok(changed)
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

    pub fn local_fact_high_water(&self) -> Result<u64> {
        let sequence: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(local_fact_sequence), 0) FROM accepted_events",
                [],
                |row| row.get(0),
            )
            .context("read local fact high water")?;
        sequence
            .try_into()
            .context("local fact sequence is negative")
    }

    pub fn event_local_fact_sequence(&self, event_id: &str) -> Result<Option<u64>> {
        self.conn
            .query_row(
                "SELECT local_fact_sequence FROM accepted_events WHERE event_id = ?1",
                params![event_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("read event local fact sequence")?
            .map(|sequence| {
                sequence
                    .try_into()
                    .context("local fact sequence is negative")
            })
            .transpose()
    }

    pub fn room_events_with_sequence(&self, room_id: &str) -> Result<Vec<SequencedEvent>> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT local_fact_sequence, event_json
                FROM accepted_events
                WHERE room_id = ?1
                ORDER BY local_fact_sequence ASC
                "#,
            )
            .context("prepare sequenced room event query")?;
        let rows = stmt
            .query_map(params![room_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .context("query sequenced room events")?;
        let mut events = Vec::new();
        for row in rows {
            let (sequence, json) = row.context("read sequenced room event row")?;
            events.push(SequencedEvent {
                local_fact_sequence: sequence
                    .try_into()
                    .context("local fact sequence is negative")?,
                event: serde_json::from_str(&json).context("parse stored room event")?,
            });
        }
        Ok(events)
    }

    pub fn open_resident_observation_consumer(
        &self,
        consumer_id: &str,
        start: ResidentObservationStart,
        now_ms: i64,
    ) -> Result<ResidentObservationConsumer> {
        validate_consumer_id(consumer_id)?;
        let transaction = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .context("start resident observation open transaction")?;
        if let Some(existing) = load_resident_observation_consumer(&transaction, consumer_id)? {
            if existing.start != start {
                anyhow::bail!(
                    "resident observation consumer already uses a different start policy"
                );
            }
            transaction
                .commit()
                .context("finish resident observation lookup")?;
            return Ok(existing);
        }
        let count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM resident_observation_consumers",
                [],
                |row| row.get(0),
            )
            .context("count resident observation consumers")?;
        if usize::try_from(count).unwrap_or(usize::MAX) >= MAX_RESIDENT_OBSERVATION_CONSUMERS {
            anyhow::bail!("resident observation consumer limit reached");
        }
        let high_water = local_fact_high_water_in(&transaction)?;
        let start_fact_sequence = match start {
            ResidentObservationStart::FromBeginning => 0,
            ResidentObservationStart::FromNow => high_water,
        };
        transaction
            .execute(
                r#"
                INSERT INTO resident_observation_consumers
                    (consumer_id, start_policy, start_fact_sequence, created_ms, updated_ms)
                VALUES (?1, ?2, ?3, ?4, ?4)
                "#,
                params![
                    consumer_id,
                    resident_start_name(start),
                    u64_to_i64(start_fact_sequence)?,
                    now_ms
                ],
            )
            .context("insert resident observation consumer")?;
        transaction
            .commit()
            .context("commit resident observation consumer")?;
        Ok(ResidentObservationConsumer {
            consumer_id: consumer_id.to_string(),
            start,
            start_fact_sequence,
            created_ms: now_ms,
            updated_ms: now_ms,
        })
    }

    pub fn resident_observation_consumer(
        &self,
        consumer_id: &str,
    ) -> Result<Option<ResidentObservationConsumer>> {
        validate_consumer_id(consumer_id)?;
        load_resident_observation_consumer(&self.conn, consumer_id)
    }

    pub fn resident_observation_checkpoint(
        &self,
        consumer_id: &str,
        room_id: &str,
    ) -> Result<Option<ResidentObservationCheckpoint>> {
        validate_consumer_id(consumer_id)?;
        validate_room_id(room_id)?;
        self.conn
            .query_row(
                r#"
                SELECT committed_fact_sequence, updated_ms
                FROM resident_observation_checkpoints
                WHERE consumer_id = ?1 AND room_id = ?2
                "#,
                params![consumer_id, room_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .context("load resident observation checkpoint")?
            .map(|(sequence, updated_ms)| {
                Ok(ResidentObservationCheckpoint {
                    consumer_id: consumer_id.to_string(),
                    room_id: room_id.to_string(),
                    committed_fact_sequence: sequence
                        .try_into()
                        .context("resident checkpoint sequence is negative")?,
                    updated_ms,
                })
            })
            .transpose()
    }

    pub fn effective_resident_observation_sequence(
        &self,
        consumer_id: &str,
        room_id: &str,
    ) -> Result<u64> {
        if let Some(checkpoint) = self.resident_observation_checkpoint(consumer_id, room_id)? {
            return Ok(checkpoint.committed_fact_sequence);
        }
        self.resident_observation_consumer(consumer_id)?
            .map(|consumer| consumer.start_fact_sequence)
            .ok_or_else(|| anyhow::anyhow!("resident observation consumer is not open"))
    }

    pub fn commit_resident_observation(
        &self,
        consumer_id: &str,
        room_id: &str,
        through_fact_sequence: u64,
        now_ms: i64,
    ) -> Result<ResidentObservationCheckpoint> {
        validate_consumer_id(consumer_id)?;
        validate_room_id(room_id)?;
        let transaction = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .context("start resident observation commit transaction")?;
        let consumer = load_resident_observation_consumer(&transaction, consumer_id)?
            .ok_or_else(|| anyhow::anyhow!("resident observation consumer is not open"))?;
        let high_water = local_fact_high_water_in(&transaction)?;
        if through_fact_sequence > high_water {
            anyhow::bail!("resident observation checkpoint exceeds local fact high water");
        }
        let existing: Option<i64> = transaction
            .query_row(
                r#"
                SELECT committed_fact_sequence
                FROM resident_observation_checkpoints
                WHERE consumer_id = ?1 AND room_id = ?2
                "#,
                params![consumer_id, room_id],
                |row| row.get(0),
            )
            .optional()
            .context("load current resident observation checkpoint")?;
        let current = existing
            .map(|value| {
                value
                    .try_into()
                    .context("resident checkpoint sequence is negative")
            })
            .transpose()?
            .unwrap_or(consumer.start_fact_sequence);
        if through_fact_sequence < current {
            anyhow::bail!("resident observation checkpoint cannot move backwards");
        }
        if existing.is_none() {
            let count: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM resident_observation_checkpoints WHERE consumer_id = ?1",
                    params![consumer_id],
                    |row| row.get(0),
                )
                .context("count resident observation rooms")?;
            if usize::try_from(count).unwrap_or(usize::MAX)
                >= MAX_RESIDENT_OBSERVATION_ROOMS_PER_CONSUMER
            {
                anyhow::bail!("resident observation room limit reached");
            }
        }
        transaction
            .execute(
                r#"
                INSERT INTO resident_observation_checkpoints
                    (consumer_id, room_id, committed_fact_sequence, updated_ms)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(consumer_id, room_id) DO UPDATE SET
                    committed_fact_sequence = excluded.committed_fact_sequence,
                    updated_ms = excluded.updated_ms
                "#,
                params![
                    consumer_id,
                    room_id,
                    u64_to_i64(through_fact_sequence)?,
                    now_ms
                ],
            )
            .context("store resident observation checkpoint")?;
        transaction
            .execute(
                "UPDATE resident_observation_consumers SET updated_ms = ?2 WHERE consumer_id = ?1",
                params![consumer_id, now_ms],
            )
            .context("touch resident observation consumer")?;
        transaction
            .commit()
            .context("commit resident observation checkpoint")?;
        Ok(ResidentObservationCheckpoint {
            consumer_id: consumer_id.to_string(),
            room_id: room_id.to_string(),
            committed_fact_sequence: through_fact_sequence,
            updated_ms: now_ms,
        })
    }

    pub fn release_resident_observation_consumer(&self, consumer_id: &str) -> Result<bool> {
        validate_consumer_id(consumer_id)?;
        Ok(self
            .conn
            .execute(
                "DELETE FROM resident_observation_consumers WHERE consumer_id = ?1",
                params![consumer_id],
            )
            .context("release resident observation consumer")?
            == 1)
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

    pub fn local_state<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        self.conn
            .query_row(
                "SELECT value_json FROM local_state WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .with_context(|| format!("load local state {key}"))?
            .map(|json| {
                serde_json::from_str(&json).with_context(|| format!("parse local state {key}"))
            })
            .transpose()
    }

    pub fn put_local_state<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json =
            serde_json::to_string(value).with_context(|| format!("serialize local state {key}"))?;
        self.conn
            .execute(
                r#"
                INSERT INTO local_state (key, value_json)
                VALUES (?1, ?2)
                ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json
                "#,
                params![key, json],
            )
            .with_context(|| format!("store local state {key}"))?;
        Ok(())
    }
}

fn validate_consumer_id(consumer_id: &str) -> Result<()> {
    if consumer_id.is_empty()
        || consumer_id.len() > MAX_CONSUMER_ID_BYTES
        || !consumer_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        anyhow::bail!(
            "consumer_id must use 1-{MAX_CONSUMER_ID_BYTES} ASCII letters, digits, '-', '_', '.', or ':'"
        );
    }
    Ok(())
}

fn validate_room_id(room_id: &str) -> Result<()> {
    if room_id.is_empty() || room_id.len() > MAX_ROOM_ID_BYTES {
        anyhow::bail!("room_id must use 1-{MAX_ROOM_ID_BYTES} bytes");
    }
    Ok(())
}

fn resident_start_name(start: ResidentObservationStart) -> &'static str {
    match start {
        ResidentObservationStart::FromBeginning => "from_beginning",
        ResidentObservationStart::FromNow => "from_now",
    }
}

fn parse_resident_start(value: &str) -> Result<ResidentObservationStart> {
    match value {
        "from_beginning" => Ok(ResidentObservationStart::FromBeginning),
        "from_now" => Ok(ResidentObservationStart::FromNow),
        _ => anyhow::bail!("unsupported resident observation start policy {value}"),
    }
}

fn u64_to_i64(value: u64) -> Result<i64> {
    value
        .try_into()
        .context("local fact sequence exceeds SQLite integer range")
}

fn local_fact_high_water_in(conn: &Connection) -> Result<u64> {
    let sequence: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(local_fact_sequence), 0) FROM accepted_events",
            [],
            |row| row.get(0),
        )
        .context("read local fact high water")?;
    sequence
        .try_into()
        .context("local fact sequence is negative")
}

fn load_resident_observation_consumer(
    conn: &Connection,
    consumer_id: &str,
) -> Result<Option<ResidentObservationConsumer>> {
    conn.query_row(
        r#"
        SELECT start_policy, start_fact_sequence, created_ms, updated_ms
        FROM resident_observation_consumers
        WHERE consumer_id = ?1
        "#,
        params![consumer_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .optional()
    .context("load resident observation consumer")?
    .map(|(start, start_fact_sequence, created_ms, updated_ms)| {
        Ok(ResidentObservationConsumer {
            consumer_id: consumer_id.to_string(),
            start: parse_resident_start(&start)?,
            start_fact_sequence: start_fact_sequence
                .try_into()
                .context("resident start sequence is negative")?,
            created_ms,
            updated_ms,
        })
    })
    .transpose()
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
                "encryption_pub": identity.encryption_public_b64(),
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
        store
            .conn
            .execute("DELETE FROM identity_heads", [])
            .expect("simulate interrupted legacy state");
        let accepted_again = accept_event(&join, &[], &context, 1_000).expect("accepted");
        assert!(!store
            .insert_accepted_event(accepted_again, 1_001)
            .expect("idempotent insert"));
        assert!(store
            .latest_identity_proof(&member.peer_id)
            .expect("repaired head")
            .is_some());
        assert_eq!(store.local_fact_high_water().expect("high water"), 1);
        assert_eq!(
            store
                .event_local_fact_sequence(&join.event_id)
                .expect("event sequence"),
            Some(1)
        );
    }

    #[test]
    fn local_fact_sequences_advance_only_for_first_admission_and_survive_reopen() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("voxelle.sqlite3");
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id);
        let join = member_join(&member);
        let first = message(&member, 1_100, vec![]);
        let second = message(&member, 1_200, vec![first.event_id.clone()]);

        {
            let store = Store::open(&path).expect("store");
            store
                .insert_accepted_event(
                    accept_event(&join, &[], &context, 1_000).expect("join accepted"),
                    1_000,
                )
                .expect("insert join");
            store
                .insert_accepted_event(
                    accept_event(&first, std::slice::from_ref(&join), &context, 1_100)
                        .expect("first accepted"),
                    1_100,
                )
                .expect("insert first");
            assert!(!store
                .insert_accepted_event(
                    accept_event(&first, std::slice::from_ref(&join), &context, 1_100)
                        .expect("duplicate accepted"),
                    9_999,
                )
                .expect("duplicate insert"));
            store
                .insert_accepted_event(
                    accept_event(&second, &[join.clone(), first.clone()], &context, 1_200)
                        .expect("second accepted"),
                    1_200,
                )
                .expect("insert second");
            assert_eq!(store.local_fact_high_water().expect("high water"), 3);
        }

        let reopened = Store::open(&path).expect("reopen");
        let sequenced = reopened
            .room_events_with_sequence("room:general")
            .expect("sequenced events");
        assert_eq!(
            sequenced
                .iter()
                .map(|event| (event.local_fact_sequence, event.event.event_id.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, first.event_id.as_str()), (3, second.event_id.as_str())]
        );
        assert_eq!(reopened.local_fact_high_water().expect("high water"), 3);
    }

    #[test]
    fn resident_observation_consumers_are_independent_monotonic_and_durable() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("voxelle.sqlite3");
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id);
        let join = member_join(&member);

        {
            let store = Store::open(&path).expect("store");
            store
                .insert_accepted_event(
                    accept_event(&join, &[], &context, 1_000).expect("join accepted"),
                    1_000,
                )
                .expect("insert join");
            let from_now = store
                .open_resident_observation_consumer(
                    "watch:alpha",
                    ResidentObservationStart::FromNow,
                    1_100,
                )
                .expect("open from now");
            assert_eq!(from_now.start_fact_sequence, 1);
            let from_beginning = store
                .open_resident_observation_consumer(
                    "watch:beta",
                    ResidentObservationStart::FromBeginning,
                    1_100,
                )
                .expect("open from beginning");
            assert_eq!(from_beginning.start_fact_sequence, 0);
            assert_eq!(
                store
                    .effective_resident_observation_sequence("watch:alpha", "room:general")
                    .expect("alpha effective sequence"),
                1
            );
            assert_eq!(
                store
                    .effective_resident_observation_sequence("watch:beta", "room:general")
                    .expect("beta effective sequence"),
                0
            );
            store
                .commit_resident_observation("watch:beta", "room:general", 1, 1_200)
                .expect("commit beta");
            store
                .commit_resident_observation("watch:beta", "room:general", 1, 1_300)
                .expect("idempotent beta commit");
            assert!(store
                .commit_resident_observation("watch:beta", "room:general", 0, 1_400)
                .expect_err("reject regression")
                .to_string()
                .contains("cannot move backwards"));
            assert!(store
                .commit_resident_observation("watch:beta", "room:general", 2, 1_400)
                .expect_err("reject future checkpoint")
                .to_string()
                .contains("exceeds local fact high water"));
        }

        let reopened = Store::open(&path).expect("reopen");
        assert_eq!(
            reopened
                .resident_observation_checkpoint("watch:beta", "room:general")
                .expect("beta checkpoint")
                .expect("beta checkpoint present")
                .committed_fact_sequence,
            1
        );
        assert!(reopened
            .resident_observation_checkpoint("watch:alpha", "room:general")
            .expect("alpha checkpoint")
            .is_none());
        assert!(reopened
            .open_resident_observation_consumer(
                "watch:alpha",
                ResidentObservationStart::FromBeginning,
                2_000,
            )
            .expect_err("start policy is immutable")
            .to_string()
            .contains("different start policy"));
        assert!(reopened
            .release_resident_observation_consumer("watch:beta")
            .expect("release beta"));
        assert!(reopened
            .resident_observation_checkpoint("watch:beta", "room:general")
            .expect("released checkpoint")
            .is_none());
        assert!(!reopened
            .release_resident_observation_consumer("watch:beta")
            .expect("release beta again"));
    }

    #[test]
    fn resident_observation_consumer_ids_and_count_are_bounded() {
        let store = Store::open_in_memory().expect("store");
        assert!(store
            .open_resident_observation_consumer(
                "contains whitespace",
                ResidentObservationStart::FromBeginning,
                1_000,
            )
            .expect_err("invalid consumer id")
            .to_string()
            .contains("consumer_id"));
        for index in 0..MAX_RESIDENT_OBSERVATION_CONSUMERS {
            store
                .open_resident_observation_consumer(
                    &format!("resident-{index}"),
                    ResidentObservationStart::FromBeginning,
                    1_000,
                )
                .expect("open bounded consumer");
        }
        assert!(store
            .open_resident_observation_consumer(
                "resident-over-limit",
                ResidentObservationStart::FromBeginning,
                1_000,
            )
            .expect_err("consumer bound")
            .to_string()
            .contains("limit reached"));
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
        let accepted_before_recovery = accept_event(
            &lost_device_event,
            std::slice::from_ref(&join),
            &context,
            1_300,
        )
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
            let accepted_msg = accept_event(&msg, std::slice::from_ref(&join), &context, 1_100)
                .expect("msg accepted");
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
        let accepted_root = accept_event(&root, std::slice::from_ref(&join), &context, 1_100)
            .expect("root accepted");
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

    #[test]
    fn typed_local_state_survives_reopen_and_replacement() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("voxelle.sqlite3");
        {
            let store = Store::open(&path).expect("store");
            store
                .put_local_state("ui.preferences", &serde_json::json!({"width": 360}))
                .expect("insert local state");
            store
                .put_local_state("ui.preferences", &serde_json::json!({"width": 420}))
                .expect("replace local state");
        }

        let reopened = Store::open(&path).expect("reopen");
        let value: serde_json::Value = reopened
            .local_state("ui.preferences")
            .expect("load local state")
            .expect("present");
        assert_eq!(value, serde_json::json!({"width": 420}));
        assert!(reopened
            .local_state::<serde_json::Value>("missing")
            .expect("load missing")
            .is_none());
    }
}
