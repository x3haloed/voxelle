use anyhow::{Context, Result};
use voxelle_core::{accept_event, EventV1, RoomContext, GOVERNANCE_ROOM_ID};
use voxelle_store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncLimits {
    pub max_events_per_batch: usize,
}

impl Default for SyncLimits {
    fn default() -> Self {
        Self {
            max_events_per_batch: 64,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub offered: usize,
    pub accepted: usize,
    pub already_present: usize,
    pub rejected: usize,
    pub truncated: bool,
}

pub fn sync_room_once(
    source: &Store,
    dest: &Store,
    room_id: &str,
    context: &RoomContext,
    now_ms: i64,
    limits: SyncLimits,
) -> Result<SyncStats> {
    let mut offered = source
        .room_events(room_id)
        .with_context(|| format!("load source room events for {room_id}"))?;
    offered.sort_by(|a, b| {
        a.created_ms
            .cmp(&b.created_ms)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    let truncated = offered.len() > limits.max_events_per_batch;
    offered.truncate(limits.max_events_per_batch);

    let mut stats = SyncStats {
        offered: offered.len(),
        truncated,
        ..SyncStats::default()
    };

    for event in offered {
        if dest.has_event(&event.event_id)? {
            stats.already_present += 1;
            continue;
        }
        match insert_after_acceptance(dest, &event, context, now_ms) {
            Ok(true) => stats.accepted += 1,
            Ok(false) => stats.already_present += 1,
            Err(_) => stats.rejected += 1,
        }
    }

    Ok(stats)
}

pub fn sync_rooms_once(
    source: &Store,
    dest: &Store,
    room_ids: &[&str],
    context: &RoomContext,
    now_ms: i64,
    limits: SyncLimits,
) -> Result<SyncStats> {
    let mut total = SyncStats::default();

    let governance = sync_room_once(source, dest, GOVERNANCE_ROOM_ID, context, now_ms, limits)?;
    merge_stats(&mut total, governance);

    for room_id in room_ids {
        if *room_id == GOVERNANCE_ROOM_ID {
            continue;
        }
        let stats = sync_room_once(source, dest, room_id, context, now_ms, limits)?;
        merge_stats(&mut total, stats);
    }

    Ok(total)
}

fn insert_after_acceptance(
    dest: &Store,
    event: &EventV1,
    context: &RoomContext,
    now_ms: i64,
) -> Result<bool> {
    let governance_events = dest.room_events(GOVERNANCE_ROOM_ID)?;
    let accepted = accept_event(event, &governance_events, context, now_ms)
        .map_err(|e| anyhow::anyhow!("event rejected: {e:?}"))?;
    dest.insert_accepted_event(accepted, now_ms)
}

fn merge_stats(total: &mut SyncStats, next: SyncStats) {
    total.offered += next.offered;
    total.accepted += next.accepted;
    total.already_present += next.already_present;
    total.rejected += next.rejected;
    total.truncated |= next.truncated;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use voxelle_core::{
        create_delegation, create_event, PeerIdentity, RoomContext, GOVERNANCE_ROOM_ID,
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

    fn message(
        identity: &PeerIdentity,
        created_ms: i64,
        parents: Vec<String>,
        text: &str,
    ) -> EventV1 {
        create_event(
            identity,
            delegation_for(identity, vec!["room:post".to_string()]),
            "room:general",
            created_ms,
            "MSG_POST",
            parents,
            json!({ "text": text }),
        )
        .expect("message")
    }

    fn insert_seed(store: &Store, event: &EventV1, context: &RoomContext, now_ms: i64) {
        let governance = store.room_events(GOVERNANCE_ROOM_ID).expect("governance");
        let accepted = accept_event(event, &governance, context, now_ms).expect("accepted");
        store
            .insert_accepted_event(accepted, now_ms)
            .expect("insert");
    }

    #[test]
    fn two_stores_converge_from_missing_events() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer.id);
        let a = Store::open_in_memory().expect("store a");
        let b = Store::open_in_memory().expect("store b");

        let join = member_join(&alice);
        let msg = message(&alice, 1_100, vec![], "hello");
        insert_seed(&a, &join, &context, 1_000);
        insert_seed(&a, &msg, &context, 1_100);

        let stats = sync_rooms_once(
            &a,
            &b,
            &["room:general"],
            &context,
            1_200,
            SyncLimits::default(),
        )
        .expect("sync");

        assert_eq!(stats.accepted, 2);
        assert_eq!(
            b.room_event_count(GOVERNANCE_ROOM_ID).expect("gov count"),
            1
        );
        assert_eq!(b.room_event_count("room:general").expect("room count"), 1);
        assert_eq!(
            a.room_heads("room:general").unwrap(),
            b.room_heads("room:general").unwrap()
        );
    }

    #[test]
    fn duplicate_events_are_ignored() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer.id);
        let a = Store::open_in_memory().expect("store a");
        let b = Store::open_in_memory().expect("store b");
        let join = member_join(&alice);
        insert_seed(&a, &join, &context, 1_000);
        insert_seed(&b, &join, &context, 1_000);

        let stats = sync_room_once(
            &a,
            &b,
            GOVERNANCE_ROOM_ID,
            &context,
            1_100,
            SyncLimits::default(),
        )
        .expect("sync");
        assert_eq!(stats.already_present, 1);
        assert_eq!(stats.accepted, 0);
    }

    #[test]
    fn room_event_is_rejected_when_destination_lacks_membership_state() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer.id);
        let a = Store::open_in_memory().expect("store a");
        let b = Store::open_in_memory().expect("store b");
        let join = member_join(&alice);
        let event = message(&alice, 1_100, vec![], "not before join");
        insert_seed(&a, &join, &context, 1_000);
        insert_seed(&a, &event, &context, 1_100);

        // Intentionally sync the room without syncing governance first. Destination validation
        // should reject the message because Alice is not yet a member in destination state.
        let stats = sync_room_once(
            &a,
            &b,
            "room:general",
            &context,
            1_100,
            SyncLimits::default(),
        )
        .expect("sync");
        assert_eq!(stats.accepted, 0);
        assert_eq!(stats.rejected, 1);
        assert_eq!(b.room_event_count("room:general").expect("count"), 0);
    }

    #[test]
    fn batch_limits_are_enforced() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer.id);
        let a = Store::open_in_memory().expect("store a");
        let b = Store::open_in_memory().expect("store b");
        let join = member_join(&alice);
        insert_seed(&a, &join, &context, 1_000);
        for i in 0..3 {
            let msg = message(&alice, 1_100 + i, vec![], &format!("msg-{i}"));
            insert_seed(&a, &msg, &context, 1_100 + i);
        }
        sync_room_once(
            &a,
            &b,
            GOVERNANCE_ROOM_ID,
            &context,
            1_200,
            SyncLimits::default(),
        )
        .expect("sync governance");

        let stats = sync_room_once(
            &a,
            &b,
            "room:general",
            &context,
            1_200,
            SyncLimits {
                max_events_per_batch: 2,
            },
        )
        .expect("sync room");
        assert_eq!(stats.offered, 2);
        assert_eq!(stats.accepted, 2);
        assert!(stats.truncated);
        assert_eq!(b.room_event_count("room:general").expect("count"), 2);
    }

    #[test]
    fn third_store_receives_forwarded_events_without_central_relay() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer.id);
        let a = Store::open_in_memory().expect("store a");
        let b = Store::open_in_memory().expect("store b");
        let c = Store::open_in_memory().expect("store c");
        let join = member_join(&alice);
        let msg = message(&alice, 1_100, vec![], "hello through bob");

        insert_seed(&a, &join, &context, 1_000);
        insert_seed(&a, &msg, &context, 1_100);

        sync_rooms_once(
            &a,
            &b,
            &["room:general"],
            &context,
            1_200,
            SyncLimits::default(),
        )
        .expect("a to b");
        sync_rooms_once(
            &b,
            &c,
            &["room:general"],
            &context,
            1_300,
            SyncLimits::default(),
        )
        .expect("b to c");

        assert!(c.has_event(&join.event_id).expect("has join"));
        assert!(c.has_event(&msg.event_id).expect("has msg"));
        assert_eq!(
            c.room_heads("room:general").expect("heads"),
            vec![msg.event_id]
        );
    }
}
