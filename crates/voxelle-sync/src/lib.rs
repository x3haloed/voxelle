use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use voxelle_core::{
    accept_event, topo_sort_deterministic, EventV1, RoomContext, GOVERNANCE_ROOM_ID,
};
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
    pub sent: usize,
    pub remote_accepted: usize,
    pub remote_already_present: usize,
    pub remote_rejected: usize,
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
    let known_heads = dest.room_heads(room_id)?;
    let (offered, truncated) = missing_events_for_heads(source, room_id, &known_heads, limits)?;

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

/// Selects the oldest causally ordered events that are not ancestors of any
/// head the other store reported. Unknown heads are safe: they simply provide
/// no pruning until a shared event arrives in a later bounded exchange.
pub fn missing_events_for_heads(
    source: &Store,
    room_id: &str,
    known_heads: &[String],
    limits: SyncLimits,
) -> Result<(Vec<EventV1>, bool)> {
    if limits.max_events_per_batch == 0 {
        anyhow::bail!("max_events_per_batch must be positive");
    }

    let events = source
        .room_events(room_id)
        .with_context(|| format!("load source room events for {room_id}"))?;
    let by_id: BTreeMap<_, _> = events
        .iter()
        .map(|event| (event.event_id.as_str(), event))
        .collect();
    let mut known = BTreeSet::new();
    let mut pending: Vec<_> = known_heads.iter().map(String::as_str).collect();
    while let Some(event_id) = pending.pop() {
        let Some(event) = by_id.get(event_id) else {
            continue;
        };
        if !known.insert(event_id) {
            continue;
        }
        pending.extend(event.parents.iter().map(String::as_str));
    }

    let mut missing: Vec<_> = topo_sort_deterministic(&events)
        .into_iter()
        .filter(|event_id| !known.contains(event_id.as_str()))
        .filter_map(|event_id| by_id.get(event_id.as_str()).copied().cloned())
        .collect();
    let truncated = missing.len() > limits.max_events_per_batch;
    missing.truncate(limits.max_events_per_batch);
    Ok((missing, truncated))
}

pub fn accept_offered_events_once(
    dest: &Store,
    offered: &[EventV1],
    context: &RoomContext,
    now_ms: i64,
) -> Result<SyncStats> {
    let mut stats = SyncStats {
        offered: offered.len(),
        ..SyncStats::default()
    };

    let mut accepted_events = dest.room_events(&context.governance_room_id)?;
    if let Some(room_id) = offered
        .iter()
        .map(|event| event.room_id.as_str())
        .find(|room_id| *room_id != context.governance_room_id)
    {
        accepted_events.extend(dest.room_events(room_id)?);
    }

    for event in offered {
        if dest.has_event(&event.event_id)? {
            stats.already_present += 1;
            continue;
        }
        let accepted = accept_event(event, &accepted_events, context, now_ms)
            .map_err(|error| anyhow::anyhow!("event rejected: {error:?}"));
        match accepted.and_then(|accepted| dest.insert_accepted_event(accepted, now_ms)) {
            Ok(true) => {
                stats.accepted += 1;
                accepted_events.push(event.clone());
            }
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
    let mut accepted_events = dest.room_events(&context.governance_room_id)?;
    if event.room_id != context.governance_room_id {
        accepted_events.extend(dest.room_events(&event.room_id)?);
    }
    let accepted = accept_event(event, &accepted_events, context, now_ms)
        .map_err(|e| anyhow::anyhow!("event rejected: {e:?}"))?;
    dest.insert_accepted_event(accepted, now_ms)
}

pub fn merge_stats(total: &mut SyncStats, next: SyncStats) {
    total.offered += next.offered;
    total.accepted += next.accepted;
    total.already_present += next.already_present;
    total.rejected += next.rejected;
    total.sent += next.sent;
    total.remote_accepted += next.remote_accepted;
    total.remote_already_present += next.remote_already_present;
    total.remote_rejected += next.remote_rejected;
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
        let context = RoomContext::new(authority.peer_id);
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
    fn shared_heads_avoid_offering_duplicate_events() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer_id);
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
        assert_eq!(stats.offered, 0);
        assert_eq!(stats.already_present, 0);
        assert_eq!(stats.accepted, 0);
    }

    #[test]
    fn room_event_is_rejected_when_destination_lacks_membership_state() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer_id);
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
        let context = RoomContext::new(authority.peer_id);
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

        let second = sync_room_once(
            &a,
            &b,
            "room:general",
            &context,
            1_300,
            SyncLimits {
                max_events_per_batch: 2,
            },
        )
        .expect("second sync room");
        assert_eq!(second.accepted, 1);
        assert!(!second.truncated);
        assert_eq!(b.room_event_count("room:general").expect("count"), 3);

        let settled = sync_room_once(
            &a,
            &b,
            "room:general",
            &context,
            1_400,
            SyncLimits {
                max_events_per_batch: 2,
            },
        )
        .expect("settled sync room");
        assert_eq!(settled.offered, 0);
        assert!(!settled.truncated);
    }

    #[test]
    fn third_store_receives_forwarded_events_without_central_relay() {
        let authority = PeerIdentity::generate().expect("authority");
        let alice = PeerIdentity::generate().expect("alice");
        let context = RoomContext::new(authority.peer_id);
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
