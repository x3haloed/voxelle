use anyhow::{anyhow, Context, Result};
use base64::Engine;
use ed25519_dalek::pkcs8::EncodePublicKey;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spki::der::Decode;
use spki::SubjectPublicKeyInfoRef;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};

const OID_ED25519: spki::ObjectIdentifier = spki::ObjectIdentifier::new_unwrap("1.3.101.112");
pub const GOVERNANCE_ROOM_ID: &str = "governance";

#[derive(Debug, Clone)]
pub struct Keypair {
    signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub spki_der: Vec<u8>,
    pub spki_b64: String,
    pub id: String,
}

impl Keypair {
    pub fn generate() -> Result<Self> {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        Self::from_signing_key(signing_key)
    }

    pub fn from_signing_key(signing_key: SigningKey) -> Result<Self> {
        let verifying_key = signing_key.verifying_key();
        let spki_der = verifying_key
            .to_public_key_der()
            .context("encode Ed25519 public key as SPKI DER")?
            .as_bytes()
            .to_vec();
        let spki_b64 = base64::engine::general_purpose::STANDARD.encode(&spki_der);
        let id = id_from_spki_der(&spki_der)?;
        Ok(Self {
            signing_key,
            verifying_key,
            spki_der,
            spki_b64,
            id,
        })
    }

    pub fn sign(&self, bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.signing_key.sign(bytes).to_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub peer: Keypair,
    pub device: Keypair,
}

impl PeerIdentity {
    pub fn generate() -> Result<Self> {
        Ok(Self {
            peer: Keypair::generate()?,
            device: Keypair::generate()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegationCertV1 {
    pub v: u8,
    pub peer_id: String,
    pub peer_pub: String,
    pub device_id: String,
    pub device_pub: String,
    pub not_before_ms: i64,
    pub expires_ms: i64,
    pub scopes: Vec<String>,
    pub sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventV1 {
    pub v: u8,
    pub room_id: String,
    pub event_id: String,
    pub author_peer_id: String,
    pub author_device_id: String,
    pub author_device_pub: String,
    pub delegation: DelegationCertV1,
    pub created_ms: i64,
    pub kind: String,
    pub parents: Vec<String>,
    pub body: serde_json::Value,
    pub sig: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomContext {
    pub authority_peer_id: String,
    pub governance_room_id: String,
}

impl RoomContext {
    pub fn new(authority_peer_id: impl Into<String>) -> Self {
        Self {
            authority_peer_id: authority_peer_id.into(),
            governance_room_id: GOVERNANCE_ROOM_ID.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GovernanceState {
    pub members: HashSet<String>,
    pub banned: HashSet<String>,
    pub revoked_devices: HashSet<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptError {
    Invalid(String),
    NotMember,
    Banned,
    DeviceRevoked,
    NotAuthorized,
    InvalidGovernanceBody(String),
}

pub type AcceptResult<T> = std::result::Result<T, AcceptError>;

#[derive(Debug, Clone, Copy)]
pub struct AcceptedEvent<'a> {
    event: &'a EventV1,
}

impl<'a> AcceptedEvent<'a> {
    pub fn event(self) -> &'a EventV1 {
        self.event
    }
}

pub fn create_delegation(
    peer: &Keypair,
    device: &Keypair,
    not_before_ms: i64,
    expires_ms: i64,
    scopes: Vec<String>,
) -> Result<DelegationCertV1> {
    let unsigned = DelegationUnsigned {
        v: 1,
        peer_id: peer.id.clone(),
        peer_pub: peer.spki_b64.clone(),
        device_id: device.id.clone(),
        device_pub: device.spki_b64.clone(),
        not_before_ms,
        expires_ms,
        scopes,
    };
    let sig_input = delegation_signature_input(&unsigned)?;
    Ok(DelegationCertV1 {
        v: unsigned.v,
        peer_id: unsigned.peer_id,
        peer_pub: unsigned.peer_pub,
        device_id: unsigned.device_id,
        device_pub: unsigned.device_pub,
        not_before_ms: unsigned.not_before_ms,
        expires_ms: unsigned.expires_ms,
        scopes: unsigned.scopes,
        sig: peer.sign(&sig_input),
    })
}

pub fn validate_delegation_at(
    delegation: &DelegationCertV1,
    expected_peer_id: &str,
    expected_device_id: &str,
    required_scope: &str,
    now_ms: i64,
) -> Result<()> {
    if delegation.v != 1 {
        return Err(anyhow!("delegation.v must be 1"));
    }
    if delegation.peer_id != expected_peer_id {
        return Err(anyhow!("delegation peer_id mismatch"));
    }
    if delegation.device_id != expected_device_id {
        return Err(anyhow!("delegation device_id mismatch"));
    }
    if now_ms < delegation.not_before_ms {
        return Err(anyhow!("delegation not yet valid"));
    }
    if now_ms > delegation.expires_ms {
        return Err(anyhow!("delegation expired"));
    }
    if !delegation.scopes.iter().any(|s| s == required_scope) {
        return Err(anyhow!(
            "delegation missing required scope: {required_scope}"
        ));
    }

    let peer_spki = b64_decode(&delegation.peer_pub).context("decode peer_pub")?;
    let device_spki = b64_decode(&delegation.device_pub).context("decode device_pub")?;
    if id_from_spki_der(&peer_spki)? != delegation.peer_id {
        return Err(anyhow!("delegation peer_id does not match peer_pub"));
    }
    if id_from_spki_der(&device_spki)? != delegation.device_id {
        return Err(anyhow!("delegation device_id does not match device_pub"));
    }

    let unsigned = DelegationUnsigned {
        v: delegation.v,
        peer_id: delegation.peer_id.clone(),
        peer_pub: delegation.peer_pub.clone(),
        device_id: delegation.device_id.clone(),
        device_pub: delegation.device_pub.clone(),
        not_before_ms: delegation.not_before_ms,
        expires_ms: delegation.expires_ms,
        scopes: delegation.scopes.clone(),
    };
    verify_signature(
        &ed25519_public_key_from_spki_der(&peer_spki)?,
        &delegation_signature_input(&unsigned)?,
        &delegation.sig,
    )
    .context("delegation signature invalid")
}

pub fn create_event(
    identity: &PeerIdentity,
    delegation: DelegationCertV1,
    room_id: impl Into<String>,
    created_ms: i64,
    kind: impl Into<String>,
    parents: Vec<String>,
    body: serde_json::Value,
) -> Result<EventV1> {
    let mut parents = parents;
    parents.sort();
    parents.dedup();

    let unsigned = EventUnsigned {
        v: 1,
        room_id: room_id.into(),
        author_peer_id: identity.peer.id.clone(),
        author_device_id: identity.device.id.clone(),
        author_device_pub: identity.device.spki_b64.clone(),
        delegation_sig: delegation.sig.clone(),
        created_ms,
        kind: kind.into(),
        parents,
        body,
    };
    let sig_input = event_signature_input(&unsigned)?;
    let event_id = event_id_from_signature_input(&sig_input);
    Ok(EventV1 {
        v: unsigned.v,
        room_id: unsigned.room_id,
        event_id,
        author_peer_id: unsigned.author_peer_id,
        author_device_id: unsigned.author_device_id,
        author_device_pub: unsigned.author_device_pub,
        delegation,
        created_ms: unsigned.created_ms,
        kind: unsigned.kind,
        parents: unsigned.parents,
        body: unsigned.body,
        sig: identity.device.sign(&sig_input),
    })
}

pub fn validate_event_at(event: &EventV1, required_scope: &str, now_ms: i64) -> Result<()> {
    if event.v != 1 {
        return Err(anyhow!("event.v must be 1"));
    }
    validate_delegation_at(
        &event.delegation,
        &event.author_peer_id,
        &event.author_device_id,
        required_scope,
        now_ms,
    )?;

    let device_spki = b64_decode(&event.author_device_pub).context("decode author_device_pub")?;
    if id_from_spki_der(&device_spki)? != event.author_device_id {
        return Err(anyhow!(
            "event author_device_id does not match author_device_pub"
        ));
    }
    if event.delegation.device_pub != event.author_device_pub {
        return Err(anyhow!("event author_device_pub does not match delegation"));
    }

    let mut parents = event.parents.clone();
    parents.sort();
    parents.dedup();
    if parents != event.parents {
        return Err(anyhow!("event parents are not canonical"));
    }

    let unsigned = EventUnsigned {
        v: event.v,
        room_id: event.room_id.clone(),
        author_peer_id: event.author_peer_id.clone(),
        author_device_id: event.author_device_id.clone(),
        author_device_pub: event.author_device_pub.clone(),
        delegation_sig: event.delegation.sig.clone(),
        created_ms: event.created_ms,
        kind: event.kind.clone(),
        parents: event.parents.clone(),
        body: event.body.clone(),
    };
    let sig_input = event_signature_input(&unsigned)?;
    let expected_event_id = event_id_from_signature_input(&sig_input);
    if event.event_id != expected_event_id {
        return Err(anyhow!("event_id mismatch"));
    }
    verify_signature(
        &ed25519_public_key_from_spki_der(&device_spki)?,
        &sig_input,
        &event.sig,
    )
    .context("event signature invalid")
}

pub fn accept_event<'a>(
    event: &'a EventV1,
    accepted_room_events: &[EventV1],
    context: &RoomContext,
    now_ms: i64,
) -> AcceptResult<AcceptedEvent<'a>> {
    let required_scope = required_scope_for_kind(&event.kind);
    validate_event_at(event, required_scope, now_ms)
        .map_err(|e| AcceptError::Invalid(e.to_string()))?;

    let governance_events: Vec<EventV1> = accepted_room_events
        .iter()
        .filter(|e| e.room_id == context.governance_room_id)
        .cloned()
        .collect();
    let state = derive_governance_state(&governance_events, context, now_ms);

    if state
        .revoked_devices
        .contains(&(event.author_peer_id.clone(), event.author_device_id.clone()))
    {
        return Err(AcceptError::DeviceRevoked);
    }

    if event.room_id == context.governance_room_id {
        accept_governance_event(event, &state, context)
    } else {
        if state.banned.contains(&event.author_peer_id) {
            return Err(AcceptError::Banned);
        }
        if !state.members.contains(&event.author_peer_id) {
            return Err(AcceptError::NotMember);
        }
        Ok(AcceptedEvent { event })
    }
}

pub fn derive_governance_state(
    governance_events: &[EventV1],
    context: &RoomContext,
    now_ms: i64,
) -> GovernanceState {
    let mut state = GovernanceState::default();
    let by_id: BTreeMap<String, &EventV1> = governance_events
        .iter()
        .map(|event| (event.event_id.clone(), event))
        .collect();

    for id in topo_sort_deterministic(governance_events) {
        let Some(event) = by_id.get(&id).copied() else {
            continue;
        };
        let required_scope = required_scope_for_kind(&event.kind);
        if validate_event_at(event, required_scope, now_ms).is_err() {
            continue;
        }
        if event.room_id != context.governance_room_id {
            continue;
        }

        match event.kind.as_str() {
            "MEMBER_JOIN" => {
                if member_join_body_matches_author(event)
                    && !state.banned.contains(&event.author_peer_id)
                {
                    state.members.insert(event.author_peer_id.clone());
                }
            }
            "MEMBER_BAN" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let Some(peer_id) = string_body_field(event, "peer_id") {
                    state.banned.insert(peer_id.clone());
                    state.members.remove(&peer_id);
                }
            }
            "MEMBER_UNBAN" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let Some(peer_id) = string_body_field(event, "peer_id") {
                    state.banned.remove(&peer_id);
                }
            }
            "DEVICE_REVOKE" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let (Some(peer_id), Some(device_id)) = (
                    string_body_field(event, "peer_id"),
                    string_body_field(event, "device_id"),
                ) {
                    state.revoked_devices.insert((peer_id, device_id));
                }
            }
            _ => {}
        }
    }

    state
}

fn accept_governance_event<'a>(
    event: &'a EventV1,
    state: &GovernanceState,
    context: &RoomContext,
) -> AcceptResult<AcceptedEvent<'a>> {
    match event.kind.as_str() {
        "MEMBER_JOIN" => {
            if state.banned.contains(&event.author_peer_id) {
                return Err(AcceptError::Banned);
            }
            if !member_join_body_matches_author(event) {
                return Err(AcceptError::InvalidGovernanceBody(
                    "MEMBER_JOIN body must match author peer".to_string(),
                ));
            }
            Ok(AcceptedEvent { event })
        }
        "MEMBER_BAN" | "MEMBER_UNBAN" | "DEVICE_REVOKE" => {
            if event.author_peer_id != context.authority_peer_id {
                return Err(AcceptError::NotAuthorized);
            }
            Ok(AcceptedEvent { event })
        }
        _ => {
            if event.author_peer_id != context.authority_peer_id {
                return Err(AcceptError::NotAuthorized);
            }
            Ok(AcceptedEvent { event })
        }
    }
}

fn required_scope_for_kind(kind: &str) -> &'static str {
    match kind {
        "MEMBER_JOIN" => "room:join",
        "MEMBER_BAN" | "MEMBER_UNBAN" | "DEVICE_REVOKE" => "room:governance",
        k if k.starts_with("MSG_") || k.starts_with("REACTION_") || k.starts_with("PIN_") => {
            "room:post"
        }
        // Unknown kinds must not bypass membership; post is the least privileged default.
        _ => "room:post",
    }
}

fn member_join_body_matches_author(event: &EventV1) -> bool {
    string_body_field(event, "peer_id").as_deref() == Some(event.author_peer_id.as_str())
        && string_body_field(event, "peer_pub").as_deref()
            == Some(event.delegation.peer_pub.as_str())
}

fn string_body_field(event: &EventV1, field: &str) -> Option<String> {
    event.body.get(field)?.as_str().map(ToOwned::to_owned)
}

pub fn compute_heads(events: &[EventV1]) -> Vec<String> {
    let ids: BTreeSet<String> = events.iter().map(|e| e.event_id.clone()).collect();
    let mut non_heads = BTreeSet::new();
    for event in events {
        for parent in &event.parents {
            if ids.contains(parent) {
                non_heads.insert(parent.clone());
            }
        }
    }
    ids.difference(&non_heads).cloned().collect()
}

pub fn topo_sort_deterministic(events: &[EventV1]) -> Vec<String> {
    let by_id: BTreeMap<String, &EventV1> = events
        .iter()
        .map(|event| (event.event_id.clone(), event))
        .collect();
    let mut children: BTreeMap<String, BTreeSet<String>> = by_id
        .keys()
        .map(|id| (id.clone(), BTreeSet::<String>::new()))
        .collect();
    let mut indegree: HashMap<String, usize> =
        by_id.keys().map(|id| (id.clone(), 0usize)).collect();

    for event in by_id.values() {
        for parent in &event.parents {
            if by_id.contains_key(parent) {
                children
                    .entry(parent.clone())
                    .or_default()
                    .insert(event.event_id.clone());
                *indegree.entry(event.event_id.clone()).or_default() += 1;
            }
        }
    }

    let mut ready: Vec<String> = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then(|| id.clone()))
        .collect();
    let mut out = Vec::with_capacity(by_id.len());

    while !ready.is_empty() {
        ready.sort_by(|a, b| compare_events(&by_id, a, b));
        let id = ready.remove(0);
        out.push(id.clone());
        if let Some(kids) = children.get(&id) {
            for kid in kids {
                if let Some(degree) = indegree.get_mut(kid) {
                    *degree -= 1;
                    if *degree == 0 {
                        ready.push(kid.clone());
                    }
                }
            }
        }
    }

    if out.len() != by_id.len() {
        let emitted: BTreeSet<_> = out.iter().cloned().collect();
        let mut remaining: Vec<String> = by_id
            .keys()
            .filter(|id| !emitted.contains(*id))
            .cloned()
            .collect();
        remaining.sort_by(|a, b| compare_events(&by_id, a, b));
        out.extend(remaining);
    }

    out
}

fn compare_events(by_id: &BTreeMap<String, &EventV1>, a: &str, b: &str) -> Ordering {
    let ta = by_id.get(a).map(|e| e.created_ms).unwrap_or_default();
    let tb = by_id.get(b).map(|e| e.created_ms).unwrap_or_default();
    ta.cmp(&tb).then_with(|| a.cmp(b))
}

#[derive(Debug)]
struct DelegationUnsigned {
    v: u8,
    peer_id: String,
    peer_pub: String,
    device_id: String,
    device_pub: String,
    not_before_ms: i64,
    expires_ms: i64,
    scopes: Vec<String>,
}

#[derive(Debug)]
struct EventUnsigned {
    v: u8,
    room_id: String,
    author_peer_id: String,
    author_device_id: String,
    author_device_pub: String,
    delegation_sig: String,
    created_ms: i64,
    kind: String,
    parents: Vec<String>,
    body: serde_json::Value,
}

fn delegation_signature_input(unsigned: &DelegationUnsigned) -> Result<Vec<u8>> {
    let mut w = NetstringWriter::new(Vec::new());
    w.write_prefix("voxelle/delegation/v1\n")?;
    w.write_int(unsigned.v.into())?;
    w.write_str(&unsigned.peer_id)?;
    w.write_str(&unsigned.peer_pub)?;
    w.write_str(&unsigned.device_id)?;
    w.write_str(&unsigned.device_pub)?;
    w.write_int(unsigned.not_before_ms)?;
    w.write_int(unsigned.expires_ms)?;
    w.write_count(unsigned.scopes.len())?;
    for scope in &unsigned.scopes {
        w.write_str(scope)?;
    }
    Ok(w.into_inner())
}

fn event_signature_input(unsigned: &EventUnsigned) -> Result<Vec<u8>> {
    let mut w = NetstringWriter::new(Vec::new());
    w.write_prefix("voxelle/event/v1\n")?;
    w.write_int(unsigned.v.into())?;
    w.write_str(&unsigned.room_id)?;
    w.write_str(&unsigned.author_peer_id)?;
    w.write_str(&unsigned.author_device_id)?;
    w.write_str(&unsigned.author_device_pub)?;
    w.write_str(&unsigned.delegation_sig)?;
    w.write_int(unsigned.created_ms)?;
    w.write_str(&unsigned.kind)?;
    w.write_count(unsigned.parents.len())?;
    for parent in &unsigned.parents {
        w.write_str(parent)?;
    }
    w.write_bytes(&jcs_bytes(&unsigned.body)?)?;
    Ok(w.into_inner())
}

fn event_id_from_signature_input(bytes: &[u8]) -> String {
    format!("e:{}", base64url_sha256(bytes))
}

pub fn id_from_spki_der(spki_der: &[u8]) -> Result<String> {
    if !is_ed25519_spki(spki_der) {
        return Err(anyhow!("SPKI is not Ed25519"));
    }
    Ok(format!("ed25519:{}", base64url_sha256(spki_der)))
}

pub fn is_ed25519_spki(spki_der: &[u8]) -> bool {
    let Ok(spki) = SubjectPublicKeyInfoRef::from_der(spki_der) else {
        return false;
    };
    spki.algorithm.oid == OID_ED25519
}

pub fn ed25519_public_key_from_spki_der(spki_der: &[u8]) -> Result<VerifyingKey> {
    let spki =
        SubjectPublicKeyInfoRef::from_der(spki_der).context("parse SPKI SubjectPublicKeyInfo")?;
    if spki.algorithm.oid != OID_ED25519 {
        return Err(anyhow!("SPKI algorithm OID is not Ed25519"));
    }
    let pk_bytes = spki
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| anyhow!("SPKI subject_public_key missing"))?;
    let pk: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| anyhow!("Ed25519 public key must be 32 bytes"))?;
    Ok(VerifyingKey::from_bytes(&pk)?)
}

pub fn jcs_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    Ok(serde_jcs::to_string(value)
        .context("serialize to JCS")?
        .into_bytes())
}

fn verify_signature(verifying_key: &VerifyingKey, message: &[u8], sig_b64: &str) -> Result<()> {
    let sig_bytes = b64_decode(sig_b64).context("decode signature")?;
    let sig = Signature::try_from(sig_bytes.as_slice()).context("parse signature")?;
    verifying_key.verify(message, &sig)?;
    Ok(())
}

fn b64_decode(s: &str) -> Result<Vec<u8>> {
    Ok(base64::engine::general_purpose::STANDARD.decode(s)?)
}

fn base64url_sha256(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(bytes))
}

struct NetstringWriter<W: Write> {
    inner: W,
}

impl<W: Write> NetstringWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner }
    }

    fn write_prefix(&mut self, prefix: &str) -> io::Result<()> {
        self.inner.write_all(prefix.as_bytes())
    }

    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write_bytes(s.as_bytes())
    }

    fn write_int(&mut self, n: i64) -> io::Result<()> {
        self.write_bytes(n.to_string().as_bytes())
    }

    fn write_count(&mut self, n: usize) -> io::Result<()> {
        self.write_bytes(n.to_string().as_bytes())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        write!(self.inner, "{}:", bytes.len())?;
        self.inner.write_all(bytes)?;
        self.inner.write_all(b",")
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity_with_delegation() -> (PeerIdentity, DelegationCertV1) {
        identity_with_scopes(vec!["room:post".to_string()])
    }

    fn identity_with_scopes(scopes: Vec<String>) -> (PeerIdentity, DelegationCertV1) {
        let identity = PeerIdentity::generate().expect("identity");
        let delegation = create_delegation(&identity.peer, &identity.device, 900, 2_000, scopes)
            .expect("delegation");
        (identity, delegation)
    }

    fn delegation_for(identity: &PeerIdentity, scopes: Vec<String>) -> DelegationCertV1 {
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

    fn authority_governance_event(
        authority: &PeerIdentity,
        created_ms: i64,
        kind: &str,
        body: serde_json::Value,
    ) -> EventV1 {
        create_event(
            authority,
            delegation_for(authority, vec!["room:governance".to_string()]),
            GOVERNANCE_ROOM_ID,
            created_ms,
            kind,
            vec![],
            body,
        )
        .expect("governance event")
    }

    #[test]
    fn peer_and_device_ids_are_stable_from_spki() {
        let identity = PeerIdentity::generate().expect("identity");

        assert!(identity.peer.id.starts_with("ed25519:"));
        assert!(identity.device.id.starts_with("ed25519:"));
        assert_ne!(identity.peer.id, identity.device.id);
        assert_eq!(
            identity.peer.id,
            id_from_spki_der(&identity.peer.spki_der).expect("peer id")
        );
        assert_eq!(
            identity.device.id,
            id_from_spki_der(&identity.device.spki_der).expect("device id")
        );
    }

    #[test]
    fn device_delegation_verifies_and_binds_ids() {
        let (identity, delegation) = identity_with_delegation();

        validate_delegation_at(
            &delegation,
            &identity.peer.id,
            &identity.device.id,
            "room:post",
            1_000,
        )
        .expect("delegation validates");

        let wrong = validate_delegation_at(
            &delegation,
            &identity.peer.id,
            &identity.device.id,
            "room:admin",
            1_000,
        );
        assert!(wrong.is_err());
    }

    #[test]
    fn event_signing_validation_and_event_id_recompute() {
        let (identity, delegation) = identity_with_delegation();
        let event = create_event(
            &identity,
            delegation,
            "room:general",
            1_100,
            "MSG_POST",
            vec!["z".to_string(), "a".to_string(), "z".to_string()],
            json!({ "text": "hello" }),
        )
        .expect("event");

        assert_eq!(event.parents, vec!["a".to_string(), "z".to_string()]);
        assert!(event.event_id.starts_with("e:"));
        validate_event_at(&event, "room:post", 1_100).expect("event validates");

        let mut tampered = event.clone();
        tampered.body = json!({ "text": "goodbye" });
        assert!(validate_event_at(&tampered, "room:post", 1_100).is_err());
    }

    #[test]
    fn dag_heads_and_deterministic_order_are_stable() {
        let (identity, delegation) = identity_with_delegation();
        let root = create_event(
            &identity,
            delegation.clone(),
            "room:general",
            1_000,
            "MSG_POST",
            vec![],
            json!({ "text": "root" }),
        )
        .expect("root");
        let left = create_event(
            &identity,
            delegation.clone(),
            "room:general",
            1_010,
            "MSG_POST",
            vec![root.event_id.clone()],
            json!({ "text": "left" }),
        )
        .expect("left");
        let right = create_event(
            &identity,
            delegation.clone(),
            "room:general",
            1_010,
            "MSG_POST",
            vec![root.event_id.clone()],
            json!({ "text": "right" }),
        )
        .expect("right");
        let merge = create_event(
            &identity,
            delegation,
            "room:general",
            1_020,
            "MSG_POST",
            vec![right.event_id.clone(), left.event_id.clone()],
            json!({ "text": "merge" }),
        )
        .expect("merge");

        let shuffled = vec![merge.clone(), right.clone(), root.clone(), left.clone()];
        assert_eq!(compute_heads(&shuffled), vec![merge.event_id.clone()]);

        let order = topo_sort_deterministic(&shuffled);
        assert_eq!(order.first(), Some(&root.event_id));
        assert_eq!(order.last(), Some(&merge.event_id));
        let left_pos = order.iter().position(|id| id == &left.event_id).unwrap();
        let right_pos = order.iter().position(|id| id == &right.event_id).unwrap();
        assert_eq!(
            left_pos.cmp(&right_pos),
            compare_events(
                &BTreeMap::from([
                    (left.event_id.clone(), &left),
                    (right.event_id.clone(), &right)
                ]),
                &left.event_id,
                &right.event_id
            )
        );
    }

    #[test]
    fn non_member_message_is_rejected() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id);
        let event = message(&member, 1_100, vec![]);

        let err = accept_event(&event, &[], &context, 1_100).expect_err("not accepted");
        assert_eq!(err, AcceptError::NotMember);
    }

    #[test]
    fn member_join_admits_peer_and_member_message_is_accepted() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id);

        let join = member_join(&member);
        accept_event(&join, &[], &context, 1_000).expect("join accepted");

        let event = message(&member, 1_100, vec![]);
        accept_event(&event, &[join], &context, 1_100).expect("message accepted");
    }

    #[test]
    fn banned_peer_cannot_post() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id.clone());

        let join = member_join(&member);
        let ban = authority_governance_event(
            &authority,
            1_050,
            "MEMBER_BAN",
            json!({ "peer_id": member.peer.id }),
        );
        accept_event(&ban, &[join.clone()], &context, 1_050).expect("ban accepted");

        let event = message(&member, 1_100, vec![]);
        let err = accept_event(&event, &[join, ban], &context, 1_100).expect_err("banned");
        assert_eq!(err, AcceptError::Banned);
    }

    #[test]
    fn revoked_device_cannot_post() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id.clone());

        let join = member_join(&member);
        let revoke = authority_governance_event(
            &authority,
            1_050,
            "DEVICE_REVOKE",
            json!({
                "peer_id": member.peer.id,
                "device_id": member.device.id,
            }),
        );
        accept_event(&revoke, &[join.clone()], &context, 1_050).expect("revoke accepted");

        let event = message(&member, 1_100, vec![]);
        let err = accept_event(&event, &[join, revoke], &context, 1_100).expect_err("revoked");
        assert_eq!(err, AcceptError::DeviceRevoked);
    }

    #[test]
    fn unknown_kind_does_not_bypass_membership() {
        let authority = PeerIdentity::generate().expect("authority");
        let outsider = PeerIdentity::generate().expect("outsider");
        let context = RoomContext::new(authority.peer.id);
        let event = create_event(
            &outsider,
            delegation_for(&outsider, vec!["room:post".to_string()]),
            "room:general",
            1_100,
            "FUTURE_KIND",
            vec![],
            json!({ "opaque": true }),
        )
        .expect("unknown event");

        let err = accept_event(&event, &[], &context, 1_100).expect_err("not accepted");
        assert_eq!(err, AcceptError::NotMember);
    }

    #[test]
    fn missing_ancestors_are_tolerated_for_valid_member_events() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id);
        let join = member_join(&member);
        let event = message(&member, 1_100, vec!["e:missing".to_string()]);

        accept_event(&event, &[join], &context, 1_100).expect("missing ancestor tolerated");
    }

    #[test]
    fn governance_derivation_is_deterministic_from_shuffled_input() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer.id.clone());
        let join = member_join(&member);
        let ban = authority_governance_event(
            &authority,
            1_050,
            "MEMBER_BAN",
            json!({ "peer_id": member.peer.id }),
        );
        let unban = authority_governance_event(
            &authority,
            1_060,
            "MEMBER_UNBAN",
            json!({ "peer_id": member.peer.id }),
        );

        let a =
            derive_governance_state(&[join.clone(), ban.clone(), unban.clone()], &context, 1_100);
        let b = derive_governance_state(&[unban, join, ban], &context, 1_100);
        assert_eq!(a, b);
        assert!(!a.banned.contains(&member.peer.id));
        assert!(!a.members.contains(&member.peer.id));
    }
}
