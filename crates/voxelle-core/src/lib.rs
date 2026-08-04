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
pub const MAX_EVENT_FUTURE_SKEW_MS: i64 = 5 * 60_000;
const CALL_LIVENESS_MS: i64 = 90_000;

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

    pub fn from_secret_key_b64(secret_key_b64: &str) -> Result<Self> {
        let secret = b64_decode(secret_key_b64).context("decode secret key")?;
        let secret: [u8; 32] = secret
            .try_into()
            .map_err(|_| anyhow!("Ed25519 secret key must be 32 bytes"))?;
        Self::from_signing_key(SigningKey::from_bytes(&secret))
    }

    pub fn secret_key_b64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.signing_key.to_bytes())
    }

    pub fn sign(&self, bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.signing_key.sign(bytes).to_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub peer_id: String,
    pub peer: Keypair,
    pub device: Keypair,
    pub recovery: Keypair,
    pub proof: IdentityProofV1,
}

impl PeerIdentity {
    pub fn generate() -> Result<Self> {
        Self::generate_at(0)
    }

    pub fn generate_at(created_ms: i64) -> Result<Self> {
        let peer = Keypair::generate()?;
        let device = Keypair::generate()?;
        let recovery = Keypair::generate()?;
        let genesis = create_identity_genesis(&peer, &recovery)?;
        let peer_id = principal_id(&genesis)?;
        let mut proof = IdentityProofV1 {
            genesis,
            changes: Vec::new(),
        };
        append_identity_change(
            &mut proof,
            &peer,
            IdentityChangeAuthor::Root,
            IdentityChangeKind::DeviceAuthorize {
                device_id: device.id.clone(),
                device_pub: device.spki_b64.clone(),
                scopes: default_device_scopes(),
                expires_ms: i64::MAX,
            },
            created_ms,
        )?;
        Ok(Self {
            peer_id,
            peer,
            device,
            recovery,
            proof,
        })
    }

    pub fn from_secret_keys_b64(
        peer_secret_b64: &str,
        device_secret_b64: &str,
        recovery_secret_b64: &str,
        proof: IdentityProofV1,
    ) -> Result<Self> {
        let peer = Keypair::from_secret_key_b64(peer_secret_b64)?;
        let device = Keypair::from_secret_key_b64(device_secret_b64)?;
        let recovery = Keypair::from_secret_key_b64(recovery_secret_b64)?;
        let state = derive_identity_state(&proof)?;
        if state.root_pub != peer.spki_b64 {
            return Err(anyhow!("identity root secret does not match current proof"));
        }
        if state.recovery_pub != recovery.spki_b64 {
            return Err(anyhow!("identity recovery secret does not match genesis"));
        }
        let authorization = state
            .devices
            .get(&device.id)
            .ok_or_else(|| anyhow!("identity device is not authorized"))?;
        if authorization.device_pub != device.spki_b64 {
            return Err(anyhow!("identity device secret does not match proof"));
        }
        Ok(Self {
            peer_id: state.peer_id,
            peer,
            device,
            recovery,
            proof,
        })
    }

    pub fn recovery_card(&self) -> RecoveryCardV1 {
        RecoveryCardV1 {
            v: 1,
            genesis: self.proof.genesis.clone(),
            recovery_secret_b64: self.recovery.secret_key_b64(),
        }
    }

    pub fn encryption_secret_bytes(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"voxelle/member-encryption-key/v1\0");
        digest.update(self.recovery.signing_key.to_bytes());
        digest.finalize().into()
    }

    pub fn encryption_public_b64(&self) -> String {
        let secret = x25519_dalek::StaticSecret::from(self.encryption_secret_bytes());
        let public = x25519_dalek::PublicKey::from(&secret);
        base64::engine::general_purpose::STANDARD.encode(public.as_bytes())
    }

    pub fn recover(
        card: &RecoveryCardV1,
        latest_proof: &IdentityProofV1,
        created_ms: i64,
    ) -> Result<Self> {
        if card.v != 1 || card.genesis != latest_proof.genesis {
            return Err(anyhow!("recovery card does not match identity proof"));
        }
        let recovery = Keypair::from_secret_key_b64(&card.recovery_secret_b64)?;
        let old_state = derive_identity_state(latest_proof)?;
        if recovery.spki_b64 != old_state.recovery_pub {
            return Err(anyhow!("recovery secret does not match identity genesis"));
        }

        let peer = Keypair::generate()?;
        let device = Keypair::generate()?;
        let mut proof = latest_proof.clone();
        append_identity_change(
            &mut proof,
            &recovery,
            IdentityChangeAuthor::Recovery,
            IdentityChangeKind::RootRotate {
                new_root_pub: peer.spki_b64.clone(),
            },
            created_ms,
        )?;
        for device_id in old_state.devices.keys() {
            append_identity_change(
                &mut proof,
                &peer,
                IdentityChangeAuthor::Root,
                IdentityChangeKind::DeviceRevoke {
                    device_id: device_id.clone(),
                },
                created_ms,
            )?;
        }
        append_identity_change(
            &mut proof,
            &peer,
            IdentityChangeAuthor::Root,
            IdentityChangeKind::DeviceAuthorize {
                device_id: device.id.clone(),
                device_pub: device.spki_b64.clone(),
                scopes: default_device_scopes(),
                expires_ms: i64::MAX,
            },
            created_ms,
        )?;

        let state = derive_identity_state(&proof)?;
        Ok(Self {
            peer_id: state.peer_id,
            peer,
            device,
            recovery,
            proof,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityGenesisV1 {
    pub v: u8,
    pub initial_root_pub: String,
    pub recovery_pub: String,
    pub nonce: String,
    pub sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityProofV1 {
    pub genesis: IdentityGenesisV1,
    pub changes: Vec<IdentityChangeV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityChangeV1 {
    pub v: u8,
    pub peer_id: String,
    pub sequence: i64,
    pub previous: String,
    pub created_ms: i64,
    pub author: IdentityChangeAuthor,
    pub kind: IdentityChangeKind,
    pub change_id: String,
    pub sig: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityChangeAuthor {
    Root,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityChangeKind {
    RootRotate {
        new_root_pub: String,
    },
    DeviceAuthorize {
        device_id: String,
        device_pub: String,
        scopes: Vec<String>,
        expires_ms: i64,
    },
    DeviceRevoke {
        device_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryCardV1 {
    pub v: u8,
    pub genesis: IdentityGenesisV1,
    pub recovery_secret_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceV1 {
    pub v: u8,
    pub space_id: String,
    pub name: String,
    pub authority_peer_id: String,
    pub governance_room_id: String,
    pub default_room_id: String,
    pub nonce: String,
    pub genesis: EventV1,
}

pub fn create_space(
    identity: &PeerIdentity,
    name: impl Into<String>,
    default_channel_name: &str,
    created_ms: i64,
) -> Result<SpaceV1> {
    let name = name.into();
    validate_space_label(&name, "space name")?;
    validate_space_label(default_channel_name, "default channel name")?;
    let nonce = base64::engine::general_purpose::STANDARD.encode(rand::random::<[u8; 16]>());
    let space_id = derive_space_id(&identity.peer_id, &nonce);
    let governance_room_id = format!("{space_id}:governance");
    let default_room_id = format!("{space_id}:channel:{default_channel_name}");
    let genesis = create_event(
        identity,
        create_delegation(
            identity,
            created_ms.saturating_sub(60_000),
            i64::MAX,
            vec!["room:governance".to_string()],
        )?,
        &governance_room_id,
        created_ms,
        "SPACE_CREATE",
        Vec::new(),
        serde_json::json!({
            "space_id": space_id,
            "name": name,
            "authority_peer_id": identity.peer_id,
            "governance_room_id": governance_room_id,
            "default_room_id": default_room_id,
            "nonce": nonce,
        }),
    )?;
    Ok(SpaceV1 {
        v: 1,
        space_id,
        name,
        authority_peer_id: identity.peer_id.clone(),
        governance_room_id,
        default_room_id,
        nonce,
        genesis,
    })
}

pub fn validate_space_at(space: &SpaceV1, now_ms: i64) -> Result<()> {
    if space.v != 1 {
        return Err(anyhow!("space.v must be 1"));
    }
    validate_space_label(&space.name, "space name")?;
    let nonce = b64_decode(&space.nonce).context("decode space nonce")?;
    if nonce.len() != 16 {
        return Err(anyhow!("space nonce must be 16 bytes"));
    }
    if space.space_id != derive_space_id(&space.authority_peer_id, &space.nonce) {
        return Err(anyhow!("space_id mismatch"));
    }
    if space.governance_room_id != format!("{}:governance", space.space_id) {
        return Err(anyhow!("space governance room mismatch"));
    }
    if !space
        .default_room_id
        .starts_with(&format!("{}:channel:", space.space_id))
    {
        return Err(anyhow!("space default room is not namespaced to the space"));
    }
    let event = &space.genesis;
    validate_event_at(event, "room:governance", now_ms).context("space genesis invalid")?;
    if event.kind != "SPACE_CREATE"
        || event.room_id != space.governance_room_id
        || event.author_peer_id != space.authority_peer_id
        || !event.parents.is_empty()
    {
        return Err(anyhow!("space genesis envelope mismatch"));
    }
    let expected = serde_json::json!({
        "space_id": space.space_id,
        "name": space.name,
        "authority_peer_id": space.authority_peer_id,
        "governance_room_id": space.governance_room_id,
        "default_room_id": space.default_room_id,
        "nonce": space.nonce,
    });
    if event.body != expected {
        return Err(anyhow!("space genesis body mismatch"));
    }
    Ok(())
}

pub fn space_from_genesis(genesis: &EventV1, now_ms: i64) -> Result<SpaceV1> {
    let field = |name: &str| {
        genesis
            .body
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .with_context(|| format!("space genesis {name} missing"))
    };
    let space = SpaceV1 {
        v: 1,
        space_id: field("space_id")?,
        name: field("name")?,
        authority_peer_id: field("authority_peer_id")?,
        governance_room_id: field("governance_room_id")?,
        default_room_id: field("default_room_id")?,
        nonce: field("nonce")?,
        genesis: genesis.clone(),
    };
    validate_space_at(&space, now_ms)?;
    Ok(space)
}

pub fn create_space_invite_event(
    identity: &PeerIdentity,
    space: &SpaceV1,
    bootstrap_peers: Vec<serde_json::Value>,
    expires_ms: i64,
    created_ms: i64,
    parents: Vec<String>,
) -> Result<EventV1> {
    validate_space_at(space, created_ms)?;
    if identity.peer_id != space.authority_peer_id {
        return Err(anyhow!(
            "only the current space authority may create an invite"
        ));
    }
    if expires_ms <= created_ms {
        return Err(anyhow!("space invite must expire in the future"));
    }
    if bootstrap_peers.is_empty() || bootstrap_peers.len() > 8 {
        return Err(anyhow!(
            "space invite requires one to eight bootstrap peers"
        ));
    }
    create_event(
        identity,
        create_delegation(
            identity,
            created_ms.saturating_sub(60_000),
            expires_ms,
            vec!["room:governance".to_string()],
        )?,
        &space.governance_room_id,
        created_ms,
        "INVITE_CREATE",
        parents,
        serde_json::json!({
            "space_id": space.space_id,
            "expires_ms": expires_ms,
            "bootstrap_peers": bootstrap_peers,
        }),
    )
}

pub fn validate_space_invite_at(space: &SpaceV1, invite: &EventV1, now_ms: i64) -> Result<()> {
    validate_space_at(space, now_ms)?;
    validate_event_at(invite, "room:governance", now_ms).context("space invite invalid")?;
    if invite.kind != "INVITE_CREATE"
        || invite.room_id != space.governance_room_id
        || invite.author_peer_id != space.authority_peer_id
    {
        return Err(anyhow!("space invite envelope mismatch"));
    }
    if string_body_field(invite, "space_id").as_deref() != Some(space.space_id.as_str()) {
        return Err(anyhow!("space invite body space_id mismatch"));
    }
    let expires_ms = int_body_field(invite, "expires_ms")
        .ok_or_else(|| anyhow!("space invite expires_ms missing"))?;
    if expires_ms < now_ms {
        return Err(anyhow!("space invite expired"));
    }
    let peers = invite
        .body
        .get("bootstrap_peers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("space invite bootstrap_peers missing"))?;
    if peers.is_empty() || peers.len() > 8 {
        return Err(anyhow!(
            "space invite requires one to eight bootstrap peers"
        ));
    }
    Ok(())
}

fn derive_space_id(authority_peer_id: &str, nonce: &str) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"voxelle/space/v1\0");
    bytes.extend_from_slice(authority_peer_id.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(nonce.as_bytes());
    format!("s:{}", base64url_sha256(&bytes))
}

fn validate_space_label(value: &str, name: &str) -> Result<()> {
    let length = value.chars().count();
    if !(1..=80).contains(&length) || value.trim() != value {
        return Err(anyhow!("{name} must be 1 to 80 trimmed characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(anyhow!("{name} contains control characters"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAuthorization {
    pub device_pub: String,
    pub scopes: BTreeSet<String>,
    pub expires_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityState {
    pub peer_id: String,
    pub root_pub: String,
    pub recovery_pub: String,
    pub devices: BTreeMap<String, DeviceAuthorization>,
    pub sequence: i64,
    pub head: String,
}

fn default_device_scopes() -> Vec<String> {
    vec![
        "room:governance".to_string(),
        "room:join".to_string(),
        "room:post".to_string(),
        "room:call".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegationCertV1 {
    pub v: u8,
    pub peer_id: String,
    pub peer_pub: String,
    pub identity_proof: IdentityProofV1,
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
    pub require_invite: bool,
}

impl RoomContext {
    pub fn new(authority_peer_id: impl Into<String>) -> Self {
        Self {
            authority_peer_id: authority_peer_id.into(),
            governance_room_id: GOVERNANCE_ROOM_ID.to_string(),
            require_invite: false,
        }
    }

    pub fn for_space(
        authority_peer_id: impl Into<String>,
        governance_room_id: impl Into<String>,
    ) -> Self {
        Self {
            authority_peer_id: authority_peer_id.into(),
            governance_room_id: governance_room_id.into(),
            require_invite: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GovernanceState {
    pub members: HashSet<String>,
    pub banned: HashSet<String>,
    pub revoked_devices: HashSet<(String, String)>,
    pub active_invites: BTreeMap<String, i64>,
    pub channels: BTreeMap<String, ChannelDefinition>,
    pub roles: BTreeMap<String, RoleDefinition>,
    pub member_roles: BTreeMap<String, BTreeSet<String>>,
    pub member_encryption_keys: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelDefinition {
    pub room_id: String,
    pub name: String,
    pub topic: String,
    pub visibility: ChannelVisibility,
    pub private_members: BTreeSet<String>,
    pub key_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleDefinition {
    pub role_id: String,
    pub name: String,
    pub permissions: BTreeSet<String>,
}

pub const PERMISSION_MESSAGE_POST: &str = "message:post";
pub const PERMISSION_MESSAGE_MODERATE: &str = "message:moderate";
pub const PERMISSION_MESSAGE_PIN: &str = "message:pin";
pub const PERMISSION_CHANNEL_MANAGE: &str = "channel:manage";
pub const PERMISSION_ROLE_MANAGE: &str = "role:manage";
pub const PERMISSION_MEMBER_BAN: &str = "member:ban";
pub const PERMISSION_INVITE_CREATE: &str = "invite:create";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptError {
    Invalid(String),
    NotMember,
    Banned,
    DeviceRevoked,
    NotAuthorized,
    InvalidGovernanceBody(String),
    UnknownRoom,
    PrivateRoom,
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

fn create_identity_genesis(root: &Keypair, recovery: &Keypair) -> Result<IdentityGenesisV1> {
    let nonce = base64::engine::general_purpose::STANDARD.encode(rand::random::<[u8; 16]>());
    let unsigned = IdentityGenesisUnsigned {
        v: 1,
        initial_root_pub: root.spki_b64.clone(),
        recovery_pub: recovery.spki_b64.clone(),
        nonce,
    };
    let sig_input = identity_genesis_signature_input(&unsigned)?;
    Ok(IdentityGenesisV1 {
        v: unsigned.v,
        initial_root_pub: unsigned.initial_root_pub,
        recovery_pub: unsigned.recovery_pub,
        nonce: unsigned.nonce,
        sig: root.sign(&sig_input),
    })
}

pub fn principal_id(genesis: &IdentityGenesisV1) -> Result<String> {
    validate_identity_genesis(genesis)?;
    Ok(format!(
        "p:{}",
        base64url_sha256(&identity_genesis_signature_input(
            &IdentityGenesisUnsigned::from(genesis)
        )?)
    ))
}

pub fn derive_identity_state(proof: &IdentityProofV1) -> Result<IdentityState> {
    validate_identity_genesis(&proof.genesis)?;
    let peer_id = principal_id(&proof.genesis)?;
    let mut state = IdentityState {
        peer_id: peer_id.clone(),
        root_pub: proof.genesis.initial_root_pub.clone(),
        recovery_pub: proof.genesis.recovery_pub.clone(),
        devices: BTreeMap::new(),
        sequence: 0,
        head: peer_id.clone(),
    };

    for change in &proof.changes {
        if change.v != 1 {
            return Err(anyhow!("identity change.v must be 1"));
        }
        if change.peer_id != peer_id {
            return Err(anyhow!("identity change peer_id mismatch"));
        }
        if change.sequence != state.sequence + 1 {
            return Err(anyhow!("identity change sequence is not contiguous"));
        }
        if change.previous != state.head {
            return Err(anyhow!("identity change does not extend current head"));
        }

        let unsigned = IdentityChangeUnsigned::from(change);
        let sig_input = identity_change_signature_input(&unsigned)?;
        if change.change_id != identity_change_id(&sig_input) {
            return Err(anyhow!("identity change_id mismatch"));
        }
        let signer_pub = match (&change.author, &change.kind) {
            (IdentityChangeAuthor::Recovery, IdentityChangeKind::RootRotate { .. }) => {
                &state.recovery_pub
            }
            (IdentityChangeAuthor::Root, _) => &state.root_pub,
            (IdentityChangeAuthor::Recovery, _) => {
                return Err(anyhow!("recovery key may only rotate the root"));
            }
        };
        verify_signature_from_spki_b64(signer_pub, &sig_input, &change.sig)
            .context("identity change signature invalid")?;

        match &change.kind {
            IdentityChangeKind::RootRotate { new_root_pub } => {
                validate_key_id(new_root_pub, None).context("invalid new root key")?;
                if *new_root_pub == state.root_pub {
                    return Err(anyhow!("identity root rotation must change the root"));
                }
                state.root_pub = new_root_pub.clone();
            }
            IdentityChangeKind::DeviceAuthorize {
                device_id,
                device_pub,
                scopes,
                expires_ms,
            } => {
                validate_key_id(device_pub, Some(device_id)).context("invalid device key")?;
                if scopes.is_empty() {
                    return Err(anyhow!("device authorization scopes cannot be empty"));
                }
                let scope_set: BTreeSet<String> = scopes.iter().cloned().collect();
                if scope_set.len() != scopes.len() {
                    return Err(anyhow!("device authorization scopes must be unique"));
                }
                state.devices.insert(
                    device_id.clone(),
                    DeviceAuthorization {
                        device_pub: device_pub.clone(),
                        scopes: scope_set,
                        expires_ms: *expires_ms,
                    },
                );
            }
            IdentityChangeKind::DeviceRevoke { device_id } => {
                if state.devices.remove(device_id).is_none() {
                    return Err(anyhow!("cannot revoke an unauthorized device"));
                }
            }
        }
        state.sequence = change.sequence;
        state.head = change.change_id.clone();
    }
    Ok(state)
}

pub fn identity_proof_extends(
    known: &IdentityProofV1,
    candidate: &IdentityProofV1,
) -> Result<bool> {
    let known_state = derive_identity_state(known)?;
    let candidate_state = derive_identity_state(candidate)?;
    if known_state.peer_id != candidate_state.peer_id
        || known.genesis != candidate.genesis
        || candidate.changes.len() < known.changes.len()
    {
        return Ok(false);
    }
    Ok(candidate.changes[..known.changes.len()] == known.changes)
}

pub fn append_identity_change(
    proof: &mut IdentityProofV1,
    signer: &Keypair,
    author: IdentityChangeAuthor,
    kind: IdentityChangeKind,
    created_ms: i64,
) -> Result<IdentityChangeV1> {
    let state = derive_identity_state(proof)?;
    let expected_signer_pub = match (&author, &kind) {
        (IdentityChangeAuthor::Recovery, IdentityChangeKind::RootRotate { .. }) => {
            &state.recovery_pub
        }
        (IdentityChangeAuthor::Root, _) => &state.root_pub,
        (IdentityChangeAuthor::Recovery, _) => {
            return Err(anyhow!("recovery key may only rotate the root"));
        }
    };
    if signer.spki_b64 != *expected_signer_pub {
        return Err(anyhow!("identity change signer is not authorized"));
    }
    let unsigned = IdentityChangeUnsigned {
        v: 1,
        peer_id: state.peer_id,
        sequence: state.sequence + 1,
        previous: state.head,
        created_ms,
        author,
        kind,
    };
    let sig_input = identity_change_signature_input(&unsigned)?;
    let change = IdentityChangeV1 {
        v: unsigned.v,
        peer_id: unsigned.peer_id,
        sequence: unsigned.sequence,
        previous: unsigned.previous,
        created_ms: unsigned.created_ms,
        author: unsigned.author,
        kind: unsigned.kind,
        change_id: identity_change_id(&sig_input),
        sig: signer.sign(&sig_input),
    };
    let mut candidate = proof.clone();
    candidate.changes.push(change.clone());
    derive_identity_state(&candidate)?;
    proof.changes.push(change.clone());
    Ok(change)
}

fn validate_identity_genesis(genesis: &IdentityGenesisV1) -> Result<()> {
    if genesis.v != 1 {
        return Err(anyhow!("identity genesis.v must be 1"));
    }
    validate_key_id(&genesis.initial_root_pub, None).context("invalid initial root key")?;
    validate_key_id(&genesis.recovery_pub, None).context("invalid recovery key")?;
    let nonce = b64_decode(&genesis.nonce).context("decode identity nonce")?;
    if nonce.len() != 16 {
        return Err(anyhow!("identity nonce must be 16 bytes"));
    }
    let unsigned = IdentityGenesisUnsigned::from(genesis);
    verify_signature_from_spki_b64(
        &genesis.initial_root_pub,
        &identity_genesis_signature_input(&unsigned)?,
        &genesis.sig,
    )
    .context("identity genesis signature invalid")
}

fn validate_key_id(spki_b64: &str, expected_id: Option<&str>) -> Result<String> {
    let spki = b64_decode(spki_b64).context("decode key SPKI")?;
    let id = id_from_spki_der(&spki)?;
    if expected_id.is_some_and(|expected| expected != id) {
        return Err(anyhow!("key id does not match public key"));
    }
    Ok(id)
}

pub fn create_delegation(
    identity: &PeerIdentity,
    not_before_ms: i64,
    expires_ms: i64,
    scopes: Vec<String>,
) -> Result<DelegationCertV1> {
    let state = derive_identity_state(&identity.proof)?;
    if state.peer_id != identity.peer_id || state.root_pub != identity.peer.spki_b64 {
        return Err(anyhow!("identity does not match current proof"));
    }
    let device = state
        .devices
        .get(&identity.device.id)
        .ok_or_else(|| anyhow!("device is not authorized by identity proof"))?;
    if device.device_pub != identity.device.spki_b64 {
        return Err(anyhow!("device key does not match identity proof"));
    }
    if expires_ms > device.expires_ms {
        return Err(anyhow!("delegation outlives device authorization"));
    }
    if scopes.iter().any(|scope| !device.scopes.contains(scope)) {
        return Err(anyhow!("delegation requests an unauthorized device scope"));
    }
    let unsigned = DelegationUnsigned {
        v: 1,
        peer_id: identity.peer_id.clone(),
        peer_pub: identity.peer.spki_b64.clone(),
        identity_proof: identity.proof.clone(),
        device_id: identity.device.id.clone(),
        device_pub: identity.device.spki_b64.clone(),
        not_before_ms,
        expires_ms,
        scopes,
    };
    let sig_input = delegation_signature_input(&unsigned)?;
    Ok(DelegationCertV1 {
        v: unsigned.v,
        peer_id: unsigned.peer_id,
        peer_pub: unsigned.peer_pub,
        identity_proof: unsigned.identity_proof,
        device_id: unsigned.device_id,
        device_pub: unsigned.device_pub,
        not_before_ms: unsigned.not_before_ms,
        expires_ms: unsigned.expires_ms,
        scopes: unsigned.scopes,
        sig: identity.peer.sign(&sig_input),
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

    let identity_state = derive_identity_state(&delegation.identity_proof)
        .context("delegation identity proof invalid")?;
    if identity_state.peer_id != delegation.peer_id {
        return Err(anyhow!("delegation peer_id does not match identity proof"));
    }
    if identity_state.root_pub != delegation.peer_pub {
        return Err(anyhow!(
            "delegation peer_pub is not the current identity root"
        ));
    }
    let authorization = identity_state
        .devices
        .get(&delegation.device_id)
        .ok_or_else(|| anyhow!("delegation device is not authorized"))?;
    if authorization.device_pub != delegation.device_pub {
        return Err(anyhow!(
            "delegation device_pub does not match authorization"
        ));
    }
    if now_ms > authorization.expires_ms {
        return Err(anyhow!("delegation device authorization expired"));
    }
    if delegation
        .scopes
        .iter()
        .any(|scope| !authorization.scopes.contains(scope))
    {
        return Err(anyhow!("delegation contains an unauthorized device scope"));
    }

    let peer_spki = b64_decode(&delegation.peer_pub).context("decode peer_pub")?;
    let device_spki = b64_decode(&delegation.device_pub).context("decode device_pub")?;
    if id_from_spki_der(&device_spki)? != delegation.device_id {
        return Err(anyhow!("delegation device_id does not match device_pub"));
    }

    let unsigned = DelegationUnsigned {
        v: delegation.v,
        peer_id: delegation.peer_id.clone(),
        peer_pub: delegation.peer_pub.clone(),
        identity_proof: delegation.identity_proof.clone(),
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
        author_peer_id: identity.peer_id.clone(),
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
    if event.created_ms > now_ms.saturating_add(MAX_EVENT_FUTURE_SKEW_MS) {
        return Err(AcceptError::Invalid(
            "event timestamp is too far in the future".to_string(),
        ));
    }
    let required_scope = required_scope_for_kind(&event.kind);
    // Durable history is valid at its signed creation time. Evaluating an old
    // event against the receipt time would make delegation expiry erase it and
    // cause peers that were offline to derive a different room.
    validate_event_at(event, required_scope, event.created_ms)
        .map_err(|e| AcceptError::Invalid(e.to_string()))?;

    if let Some(known_proof) = accepted_room_events
        .iter()
        .filter(|accepted| accepted.author_peer_id == event.author_peer_id)
        .filter_map(|accepted| {
            derive_identity_state(&accepted.delegation.identity_proof)
                .ok()
                .map(|state| (state.sequence, &accepted.delegation.identity_proof))
        })
        .max_by_key(|(sequence, _)| *sequence)
        .map(|(_, proof)| proof)
    {
        let extends = identity_proof_extends(known_proof, &event.delegation.identity_proof)
            .map_err(|e| AcceptError::Invalid(e.to_string()))?;
        if !extends {
            return Err(AcceptError::Invalid(
                "event identity proof is stale or forks known identity state".to_string(),
            ));
        }
    }

    let governance_events: Vec<EventV1> = accepted_room_events
        .iter()
        .filter(|e| e.room_id == context.governance_room_id)
        .cloned()
        .collect();
    let state = derive_governance_state(&governance_events, context, event.created_ms);

    if state
        .revoked_devices
        .contains(&(event.author_peer_id.clone(), event.author_device_id.clone()))
    {
        return Err(AcceptError::DeviceRevoked);
    }

    if event.room_id == context.governance_room_id {
        accept_governance_event(event, &state, context, now_ms)
    } else {
        if state.banned.contains(&event.author_peer_id) {
            return Err(AcceptError::Banned);
        }
        if !state.members.contains(&event.author_peer_id) {
            return Err(AcceptError::NotMember);
        }
        if context.require_invite {
            let Some(channel) = state.channels.get(&event.room_id) else {
                return Err(AcceptError::UnknownRoom);
            };
            if !channel_allows_peer(channel, &event.author_peer_id) {
                return Err(AcceptError::PrivateRoom);
            }
        }
        validate_room_event_body(event, accepted_room_events, &state, context)?;
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
        if event.created_ms > now_ms {
            continue;
        }
        let required_scope = required_scope_for_kind(&event.kind);
        if validate_event_at(event, required_scope, event.created_ms).is_err() {
            continue;
        }
        if event.room_id != context.governance_room_id {
            continue;
        }

        match event.kind.as_str() {
            "SPACE_CREATE" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                let Some(room_id) = string_body_field(event, "default_room_id") else {
                    continue;
                };
                let name = room_id.rsplit(':').next().unwrap_or("general").to_string();
                state.channels.insert(
                    room_id.clone(),
                    ChannelDefinition {
                        room_id,
                        name,
                        topic: String::new(),
                        visibility: ChannelVisibility::Public,
                        private_members: BTreeSet::new(),
                        key_epoch: 0,
                    },
                );
                state.roles.insert(
                    "role:everyone".to_string(),
                    RoleDefinition {
                        role_id: "role:everyone".to_string(),
                        name: "Everyone".to_string(),
                        permissions: BTreeSet::from([PERMISSION_MESSAGE_POST.to_string()]),
                    },
                );
            }
            "MEMBER_JOIN" => {
                if member_join_body_matches_author(event)
                    && member_join_has_authority(event, &state, context, event.created_ms)
                    && !state.banned.contains(&event.author_peer_id)
                {
                    state.members.insert(event.author_peer_id.clone());
                    if let Some(encryption_pub) = string_body_field(event, "encryption_pub") {
                        if valid_encryption_public_key(&encryption_pub) {
                            state
                                .member_encryption_keys
                                .insert(event.author_peer_id.clone(), encryption_pub);
                        }
                    }
                }
            }
            "INVITE_CREATE" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                if let Some(expires_ms) = int_body_field(event, "expires_ms") {
                    if expires_ms >= event.created_ms {
                        state
                            .active_invites
                            .insert(event.event_id.clone(), expires_ms);
                    }
                }
            }
            "INVITE_REVOKE" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                if let Some(invite_id) = string_body_field(event, "invite_id") {
                    state.active_invites.remove(&invite_id);
                }
            }
            "MEMBER_BAN" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                if let Some(peer_id) = string_body_field(event, "peer_id") {
                    state.banned.insert(peer_id.clone());
                    state.members.remove(&peer_id);
                    state.member_roles.remove(&peer_id);
                    state.member_encryption_keys.remove(&peer_id);
                    for channel in state.channels.values_mut() {
                        channel.private_members.remove(&peer_id);
                    }
                }
            }
            "MEMBER_UNBAN" => {
                if !governance_event_authorized(event, &state, context) {
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
            "CHANNEL_CREATE" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                if let Some(channel) = channel_from_create_event(event, context) {
                    state
                        .channels
                        .entry(channel.room_id.clone())
                        .or_insert(channel);
                }
            }
            "CHANNEL_UPDATE" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                let Some(room_id) = string_body_field(event, "room_id") else {
                    continue;
                };
                let Some(channel) = state.channels.get_mut(&room_id) else {
                    continue;
                };
                if let Some(name) = string_body_field(event, "name") {
                    if valid_short_text(&name, 80) {
                        channel.name = name;
                    }
                }
                if let Some(topic) = string_body_field(event, "topic") {
                    if topic.chars().count() <= 1024 && !topic.chars().any(char::is_control) {
                        channel.topic = topic;
                    }
                }
            }
            "CHANNEL_KEY_ROTATE" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                if let Some(room_id) = string_body_field(event, "room_id") {
                    if let Some(channel) = state.channels.get_mut(&room_id) {
                        if let Some(epoch) = int_body_field(event, "key_epoch") {
                            if epoch > 0 && epoch as u64 > channel.key_epoch {
                                channel.key_epoch = epoch as u64;
                            }
                        }
                    }
                }
            }
            "CHANNEL_DELETE" => {
                if !governance_event_authorized(event, &state, context) {
                    continue;
                }
                if let Some(room_id) = string_body_field(event, "room_id") {
                    state.channels.remove(&room_id);
                }
            }
            "ROLE_CREATE" | "ROLE_UPDATE" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let Some(role) = role_from_event(event) {
                    state.roles.insert(role.role_id.clone(), role);
                }
            }
            "ROLE_DELETE" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let Some(role_id) = string_body_field(event, "role_id") {
                    if role_id != "role:everyone" {
                        state.roles.remove(&role_id);
                        for roles in state.member_roles.values_mut() {
                            roles.remove(&role_id);
                        }
                    }
                }
            }
            "ROLE_GRANT" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let (Some(peer_id), Some(role_id)) = (
                    string_body_field(event, "peer_id"),
                    string_body_field(event, "role_id"),
                ) {
                    if state.members.contains(&peer_id) && state.roles.contains_key(&role_id) {
                        state
                            .member_roles
                            .entry(peer_id)
                            .or_default()
                            .insert(role_id);
                    }
                }
            }
            "ROLE_REVOKE" => {
                if event.author_peer_id != context.authority_peer_id {
                    continue;
                }
                if let (Some(peer_id), Some(role_id)) = (
                    string_body_field(event, "peer_id"),
                    string_body_field(event, "role_id"),
                ) {
                    if let Some(roles) = state.member_roles.get_mut(&peer_id) {
                        roles.remove(&role_id);
                    }
                }
            }
            _ => {}
        }
    }

    state
}

pub fn peer_has_permission(
    state: &GovernanceState,
    context: &RoomContext,
    peer_id: &str,
    permission: &str,
) -> bool {
    if peer_id == context.authority_peer_id {
        return true;
    }
    let everyone_has = state
        .roles
        .get("role:everyone")
        .is_some_and(|role| role.permissions.contains(permission));
    everyone_has
        || state
            .member_roles
            .get(peer_id)
            .into_iter()
            .flatten()
            .filter_map(|role_id| state.roles.get(role_id))
            .any(|role| role.permissions.contains(permission))
}

pub fn channel_allows_peer(channel: &ChannelDefinition, peer_id: &str) -> bool {
    channel.visibility == ChannelVisibility::Public || channel.private_members.contains(peer_id)
}

fn governance_event_authorized(
    event: &EventV1,
    state: &GovernanceState,
    context: &RoomContext,
) -> bool {
    let permission = match event.kind.as_str() {
        "INVITE_CREATE" | "INVITE_REVOKE" => PERMISSION_INVITE_CREATE,
        "MEMBER_BAN" | "MEMBER_UNBAN" => PERMISSION_MEMBER_BAN,
        "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "CHANNEL_DELETE" | "CHANNEL_KEY_ROTATE" => {
            PERMISSION_CHANNEL_MANAGE
        }
        _ => return event.author_peer_id == context.authority_peer_id,
    };
    peer_has_permission(state, context, &event.author_peer_id, permission)
}

fn channel_from_create_event(event: &EventV1, context: &RoomContext) -> Option<ChannelDefinition> {
    let room_id = string_body_field(event, "room_id")?;
    let prefix = context.governance_room_id.strip_suffix(":governance")?;
    if !room_id.starts_with(&format!("{prefix}:channel:")) {
        return None;
    }
    let name = string_body_field(event, "name")?;
    if !valid_short_text(&name, 80) {
        return None;
    }
    let topic = string_body_field(event, "topic").unwrap_or_default();
    if topic.chars().count() > 1024 || topic.chars().any(char::is_control) {
        return None;
    }
    let visibility = match string_body_field(event, "visibility")?.as_str() {
        "public" => ChannelVisibility::Public,
        "private" => ChannelVisibility::Private,
        _ => return None,
    };
    let private_members: BTreeSet<String> = event
        .body
        .get("private_members")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .collect();
    if visibility == ChannelVisibility::Private
        && (private_members.is_empty() || private_members.len() > 50)
    {
        return None;
    }
    let key_epoch = event
        .body
        .get("key_epoch")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if visibility == ChannelVisibility::Public && key_epoch != 0 {
        return None;
    }
    Some(ChannelDefinition {
        room_id,
        name,
        topic,
        visibility,
        private_members,
        key_epoch,
    })
}

fn valid_encryption_public_key(encoded: &str) -> bool {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .is_ok_and(|bytes| bytes.len() == 32)
}

fn valid_key_packages(event: &EventV1, members: &BTreeSet<String>) -> bool {
    let Some(packages) = event
        .body
        .get("key_packages")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    if packages.len() != members.len() {
        return false;
    }
    let mut recipients = BTreeSet::new();
    for package in packages {
        let Some(peer_id) = package.get("peer_id").and_then(serde_json::Value::as_str) else {
            return false;
        };
        let valid_bytes = |field: &str, expected: usize| {
            package
                .get(field)
                .and_then(serde_json::Value::as_str)
                .and_then(|encoded| {
                    base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .ok()
                })
                .is_some_and(|bytes| bytes.len() == expected)
        };
        if !members.contains(peer_id)
            || !recipients.insert(peer_id.to_string())
            || !valid_bytes("ephemeral_pub_b64", 32)
            || !valid_bytes("nonce_b64", 24)
            || !valid_bytes("ciphertext_b64", 48)
        {
            return false;
        }
    }
    recipients == *members
}

fn role_from_event(event: &EventV1) -> Option<RoleDefinition> {
    let role_id = string_body_field(event, "role_id")?;
    if !role_id.starts_with("role:") || role_id == "role:everyone" {
        return None;
    }
    let name = string_body_field(event, "name")?;
    if !valid_short_text(&name, 80) {
        return None;
    }
    let permissions: BTreeSet<String> = event
        .body
        .get("permissions")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .collect();
    let allowed = [
        PERMISSION_MESSAGE_POST,
        PERMISSION_MESSAGE_MODERATE,
        PERMISSION_MESSAGE_PIN,
        PERMISSION_CHANNEL_MANAGE,
        PERMISSION_ROLE_MANAGE,
        PERMISSION_MEMBER_BAN,
        PERMISSION_INVITE_CREATE,
    ];
    if permissions.is_empty()
        || permissions.len() > allowed.len()
        || permissions
            .iter()
            .any(|permission| !allowed.contains(&permission.as_str()))
    {
        return None;
    }
    Some(RoleDefinition {
        role_id,
        name,
        permissions,
    })
}

fn valid_short_text(value: &str, max: usize) -> bool {
    let length = value.chars().count();
    (1..=max).contains(&length) && value.trim() == value && !value.chars().any(char::is_control)
}

fn accept_governance_event<'a>(
    event: &'a EventV1,
    state: &GovernanceState,
    context: &RoomContext,
    _now_ms: i64,
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
            if !member_join_has_authority(event, state, context, event.created_ms) {
                return Err(AcceptError::NotAuthorized);
            }
            Ok(AcceptedEvent { event })
        }
        "MEMBER_BAN" | "MEMBER_UNBAN" | "INVITE_CREATE" | "INVITE_REVOKE" | "CHANNEL_CREATE"
        | "CHANNEL_UPDATE" | "CHANNEL_DELETE" | "CHANNEL_KEY_ROTATE" => {
            if !governance_event_authorized(event, state, context) {
                return Err(AcceptError::NotAuthorized);
            }
            validate_governance_event_body(event, state, context)?;
            Ok(AcceptedEvent { event })
        }
        "DEVICE_REVOKE" | "ROLE_CREATE" | "ROLE_UPDATE" | "ROLE_DELETE" | "ROLE_GRANT"
        | "ROLE_REVOKE" => {
            if event.author_peer_id != context.authority_peer_id {
                return Err(AcceptError::NotAuthorized);
            }
            validate_governance_event_body(event, state, context)?;
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

fn validate_governance_event_body(
    event: &EventV1,
    state: &GovernanceState,
    context: &RoomContext,
) -> AcceptResult<()> {
    let invalid = |reason: &str| AcceptError::InvalidGovernanceBody(reason.to_string());
    match event.kind.as_str() {
        "MEMBER_BAN" | "MEMBER_UNBAN" => {
            let peer_id =
                string_body_field(event, "peer_id").ok_or_else(|| invalid("peer_id missing"))?;
            if !peer_id.starts_with("p:") || peer_id == context.authority_peer_id {
                return Err(invalid("invalid or protected peer_id"));
            }
        }
        "INVITE_CREATE" => {
            let expires =
                int_body_field(event, "expires_ms").ok_or_else(|| invalid("expires_ms missing"))?;
            if expires <= event.created_ms {
                return Err(invalid("invite expiry must follow creation"));
            }
        }
        "INVITE_REVOKE" => {
            if string_body_field(event, "invite_id").is_none() {
                return Err(invalid("invite_id missing"));
            }
        }
        "CHANNEL_CREATE" => {
            let channel = channel_from_create_event(event, context)
                .ok_or_else(|| invalid("invalid channel definition"))?;
            if state.channels.contains_key(&channel.room_id) {
                return Err(invalid("channel already exists"));
            }
            if channel.visibility == ChannelVisibility::Private
                && (!channel.private_members.contains(&event.author_peer_id)
                    || channel
                        .private_members
                        .iter()
                        .any(|peer_id| !state.members.contains(peer_id)))
            {
                return Err(invalid(
                    "private channel members must be current space members",
                ));
            }
            if channel.visibility == ChannelVisibility::Private
                && (channel.key_epoch == 0
                    || channel
                        .private_members
                        .iter()
                        .any(|peer_id| !state.member_encryption_keys.contains_key(peer_id))
                    || !valid_key_packages(event, &channel.private_members))
            {
                return Err(invalid("private channel key packages are incomplete"));
            }
        }
        "CHANNEL_UPDATE" | "CHANNEL_DELETE" => {
            let room_id =
                string_body_field(event, "room_id").ok_or_else(|| invalid("room_id missing"))?;
            if !state.channels.contains_key(&room_id) {
                return Err(invalid("channel does not exist"));
            }
        }
        "CHANNEL_KEY_ROTATE" => {
            let room_id =
                string_body_field(event, "room_id").ok_or_else(|| invalid("room_id missing"))?;
            let channel = state
                .channels
                .get(&room_id)
                .ok_or_else(|| invalid("channel does not exist"))?;
            let epoch =
                int_body_field(event, "key_epoch").ok_or_else(|| invalid("key_epoch missing"))?;
            if channel.visibility != ChannelVisibility::Private
                || epoch <= 0
                || epoch as u64 <= channel.key_epoch
                || !valid_key_packages(event, &channel.private_members)
            {
                return Err(invalid("invalid private channel key rotation"));
            }
        }
        "ROLE_CREATE" | "ROLE_UPDATE" => {
            let role = role_from_event(event).ok_or_else(|| invalid("invalid role definition"))?;
            if event.kind == "ROLE_CREATE" && state.roles.contains_key(&role.role_id) {
                return Err(invalid("role already exists"));
            }
            if event.kind == "ROLE_UPDATE" && !state.roles.contains_key(&role.role_id) {
                return Err(invalid("role does not exist"));
            }
        }
        "ROLE_DELETE" => {
            let role_id =
                string_body_field(event, "role_id").ok_or_else(|| invalid("role_id missing"))?;
            if role_id == "role:everyone" || !state.roles.contains_key(&role_id) {
                return Err(invalid("role cannot be deleted"));
            }
        }
        "ROLE_GRANT" | "ROLE_REVOKE" => {
            let peer_id =
                string_body_field(event, "peer_id").ok_or_else(|| invalid("peer_id missing"))?;
            let role_id =
                string_body_field(event, "role_id").ok_or_else(|| invalid("role_id missing"))?;
            if !state.members.contains(&peer_id) || !state.roles.contains_key(&role_id) {
                return Err(invalid(
                    "role assignment requires a current member and role",
                ));
            }
        }
        "DEVICE_REVOKE"
            if string_body_field(event, "peer_id").is_none()
                || string_body_field(event, "device_id").is_none() =>
        {
            return Err(invalid("device revocation fields missing"));
        }
        _ => {}
    }
    Ok(())
}

fn validate_room_event_body(
    event: &EventV1,
    accepted_events: &[EventV1],
    state: &GovernanceState,
    context: &RoomContext,
) -> AcceptResult<()> {
    let body_size = serde_json::to_vec(&event.body)
        .map_err(|error| AcceptError::Invalid(error.to_string()))?
        .len();
    if body_size > 384 * 1024 {
        return Err(AcceptError::Invalid(
            "event body exceeds 384 KiB".to_string(),
        ));
    }
    let author = event.author_peer_id.as_str();
    let require_post = || {
        if !context.require_invite
            || peer_has_permission(state, context, author, PERMISSION_MESSAGE_POST)
        {
            Ok(())
        } else {
            Err(AcceptError::NotAuthorized)
        }
    };
    let target = |kind: &str| -> AcceptResult<&EventV1> {
        let target_id = string_body_field(event, "target_event_id")
            .ok_or_else(|| AcceptError::Invalid(format!("{kind} target_event_id missing")))?;
        accepted_events
            .iter()
            .find(|candidate| candidate.room_id == event.room_id && candidate.event_id == target_id)
            .ok_or_else(|| AcceptError::Invalid(format!("{kind} target does not exist")))
    };

    match event.kind.as_str() {
        "MSG_POST" => {
            require_post()?;
            let text = string_body_field(event, "text")
                .ok_or_else(|| AcceptError::Invalid("MSG_POST text missing".to_string()))?;
            if !valid_message_text(&text) {
                return Err(AcceptError::Invalid("MSG_POST text is invalid".to_string()));
            }
            validate_mentions(event)?;
            if let Some(thread_root) = string_body_field(event, "thread_root_event_id") {
                if !accepted_events.iter().any(|candidate| {
                    candidate.room_id == event.room_id
                        && candidate.event_id == thread_root
                        && candidate.kind == "MSG_POST"
                }) {
                    return Err(AcceptError::Invalid(
                        "thread root does not exist".to_string(),
                    ));
                }
            }
        }
        "MSG_EDIT" => {
            require_post()?;
            let original = target("MSG_EDIT")?;
            if original.kind != "MSG_POST" || original.author_peer_id != event.author_peer_id {
                return Err(AcceptError::NotAuthorized);
            }
            if accepted_events.iter().any(|candidate| {
                candidate.room_id == event.room_id
                    && candidate.kind == "MSG_REDACT"
                    && string_body_field(candidate, "target_event_id")
                        == Some(original.event_id.clone())
            }) {
                return Err(AcceptError::Invalid(
                    "a redacted message cannot be edited".to_string(),
                ));
            }
            let text = string_body_field(event, "text")
                .ok_or_else(|| AcceptError::Invalid("MSG_EDIT text missing".to_string()))?;
            if !valid_message_text(&text) {
                return Err(AcceptError::Invalid("MSG_EDIT text is invalid".to_string()));
            }
            validate_mentions(event)?;
        }
        "MSG_REDACT" => {
            let original = target("MSG_REDACT")?;
            if original.kind != "MSG_POST"
                || (original.author_peer_id != event.author_peer_id
                    && !peer_has_permission(state, context, author, PERMISSION_MESSAGE_MODERATE))
            {
                return Err(AcceptError::NotAuthorized);
            }
        }
        "REACTION_ADD" | "REACTION_REMOVE" => {
            require_post()?;
            target("reaction")?;
            let emoji = string_body_field(event, "emoji")
                .ok_or_else(|| AcceptError::Invalid("reaction emoji missing".to_string()))?;
            if !valid_short_text(&emoji, 32) {
                return Err(AcceptError::Invalid(
                    "reaction emoji is invalid".to_string(),
                ));
            }
        }
        "PIN_ADD" | "PIN_REMOVE" => {
            if !peer_has_permission(state, context, author, PERMISSION_MESSAGE_PIN) {
                return Err(AcceptError::NotAuthorized);
            }
            target("pin")?;
        }
        "ATTACHMENT_ADD" => {
            require_post()?;
            let filename = string_body_field(event, "filename")
                .ok_or_else(|| AcceptError::Invalid("attachment filename missing".to_string()))?;
            let mime = string_body_field(event, "mime")
                .ok_or_else(|| AcceptError::Invalid("attachment mime missing".to_string()))?;
            let sha256 = string_body_field(event, "sha256")
                .ok_or_else(|| AcceptError::Invalid("attachment sha256 missing".to_string()))?;
            let data = string_body_field(event, "data_b64")
                .ok_or_else(|| AcceptError::Invalid("attachment data missing".to_string()))?;
            if !valid_short_text(&filename, 255)
                || !valid_short_text(&mime, 127)
                || !sha256.starts_with("sha256:")
            {
                return Err(AcceptError::Invalid(
                    "attachment metadata is invalid".to_string(),
                ));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| AcceptError::Invalid("attachment base64 is invalid".to_string()))?;
            if bytes.is_empty() || bytes.len() > 256 * 1024 {
                return Err(AcceptError::Invalid(
                    "attachment must be 1 to 256 KiB".to_string(),
                ));
            }
            if format!("sha256:{}", base64url_sha256(&bytes)) != sha256 {
                return Err(AcceptError::Invalid("attachment hash mismatch".to_string()));
            }
        }
        "PROFILE_UPDATE" => {
            require_post()?;
            let display_name = string_body_field(event, "display_name")
                .ok_or_else(|| AcceptError::Invalid("profile display_name missing".to_string()))?;
            if !valid_short_text(&display_name, 80) {
                return Err(AcceptError::Invalid(
                    "profile display_name is invalid".to_string(),
                ));
            }
            if string_body_field(event, "about").is_some_and(|about| {
                about.chars().count() > 512 || about.chars().any(char::is_control)
            }) {
                return Err(AcceptError::Invalid("profile about is invalid".to_string()));
            }
        }
        "IDENTITY_UPDATE" => require_post()?,
        "CALL_JOIN" => {
            require_post()?;
            let call_id = string_body_field(event, "call_id")
                .ok_or_else(|| AcceptError::Invalid("call_id missing".to_string()))?;
            if !valid_short_text(&call_id, 128) {
                return Err(AcceptError::Invalid("call_id is invalid".to_string()));
            }
            if event
                .body
                .get("video")
                .and_then(serde_json::Value::as_bool)
                .is_none()
            {
                return Err(AcceptError::Invalid("call video flag missing".to_string()));
            }
        }
        "CALL_LEAVE" => {
            require_post()?;
            let call_id = string_body_field(event, "call_id")
                .ok_or_else(|| AcceptError::Invalid("call_id missing".to_string()))?;
            if !valid_short_text(&call_id, 128) {
                return Err(AcceptError::Invalid("call_id is invalid".to_string()));
            }
            if !active_call_participants(
                accepted_events,
                &event.room_id,
                &call_id,
                event.created_ms,
            )
            .contains(author)
            {
                return Err(AcceptError::NotAuthorized);
            }
        }
        "CALL_HEARTBEAT" => {
            require_post()?;
            let call_id = string_body_field(event, "call_id")
                .ok_or_else(|| AcceptError::Invalid("call_id missing".to_string()))?;
            if !valid_short_text(&call_id, 128)
                || !active_call_participants(
                    accepted_events,
                    &event.room_id,
                    &call_id,
                    event.created_ms,
                )
                .contains(author)
            {
                return Err(AcceptError::NotAuthorized);
            }
        }
        "CALL_OFFER" | "CALL_ANSWER" | "CALL_ICE" => {
            require_post()?;
            let call_id = string_body_field(event, "call_id")
                .ok_or_else(|| AcceptError::Invalid("call_id missing".to_string()))?;
            if !valid_short_text(&call_id, 128) {
                return Err(AcceptError::Invalid("call_id is invalid".to_string()));
            }
            let target = string_body_field(event, "target_peer_id")
                .ok_or_else(|| AcceptError::Invalid("call target missing".to_string()))?;
            if !valid_short_text(&target, 256) {
                return Err(AcceptError::Invalid("call target is invalid".to_string()));
            }
            let participants = active_call_participants(
                accepted_events,
                &event.room_id,
                &call_id,
                event.created_ms,
            );
            if target == author || !participants.contains(author) || !participants.contains(&target)
            {
                return Err(AcceptError::NotAuthorized);
            }
            let field = if event.kind == "CALL_ICE" {
                "candidate"
            } else {
                "sdp"
            };
            let maximum = if field == "candidate" {
                16 * 1024
            } else {
                128 * 1024
            };
            let value = string_body_field(event, field)
                .ok_or_else(|| AcceptError::Invalid(format!("call {field} missing")))?;
            if value.is_empty() || value.len() > maximum {
                return Err(AcceptError::Invalid(format!("call {field} is invalid")));
            }
        }
        "ROOM_ENCRYPTED" => {
            require_post()?;
            let epoch = int_body_field(event, "key_epoch")
                .ok_or_else(|| AcceptError::Invalid("encrypted key_epoch missing".to_string()))?;
            let valid_field = |field: &str, minimum: usize, maximum: usize| {
                event
                    .body
                    .get(field)
                    .and_then(serde_json::Value::as_str)
                    .and_then(|encoded| {
                        base64::engine::general_purpose::STANDARD
                            .decode(encoded)
                            .ok()
                    })
                    .is_some_and(|bytes| (minimum..=maximum).contains(&bytes.len()))
            };
            if epoch <= 0
                || !valid_field("nonce_b64", 24, 24)
                || !valid_field("ciphertext_b64", 17, 384 * 1024)
            {
                return Err(AcceptError::Invalid(
                    "encrypted event envelope is invalid".to_string(),
                ));
            }
        }
        _ => require_post()?,
    }
    Ok(())
}

fn active_call_participants(
    accepted_events: &[EventV1],
    room_id: &str,
    call_id: &str,
    at_ms: i64,
) -> BTreeSet<String> {
    let mut events: Vec<&EventV1> = accepted_events
        .iter()
        .filter(|event| {
            event.room_id == room_id
                && matches!(
                    event.kind.as_str(),
                    "CALL_JOIN" | "CALL_HEARTBEAT" | "CALL_LEAVE"
                )
                && string_body_field(event, "call_id").as_deref() == Some(call_id)
                && event.created_ms <= at_ms
        })
        .collect();
    events.sort_by(|left, right| {
        left.created_ms
            .cmp(&right.created_ms)
            .then(left.event_id.cmp(&right.event_id))
    });
    let mut last_seen = BTreeMap::new();
    for event in events {
        if matches!(event.kind.as_str(), "CALL_JOIN" | "CALL_HEARTBEAT") {
            last_seen.insert(event.author_peer_id.clone(), event.created_ms);
        } else {
            last_seen.remove(&event.author_peer_id);
        }
    }
    last_seen
        .into_iter()
        .filter(|(_, seen_ms)| at_ms.saturating_sub(*seen_ms) <= CALL_LIVENESS_MS)
        .map(|(peer_id, _)| peer_id)
        .take(4)
        .collect()
}

pub fn validate_room_event_semantics(
    event: &EventV1,
    accepted_events: &[EventV1],
    context: &RoomContext,
    now_ms: i64,
) -> AcceptResult<()> {
    let state = derive_governance_state(accepted_events, context, now_ms);
    if context.require_invite {
        if state.banned.contains(&event.author_peer_id) {
            return Err(AcceptError::Banned);
        }
        if !state.members.contains(&event.author_peer_id) {
            return Err(AcceptError::NotMember);
        }
        let channel = state
            .channels
            .get(&event.room_id)
            .ok_or(AcceptError::UnknownRoom)?;
        if !channel_allows_peer(channel, &event.author_peer_id) {
            return Err(AcceptError::PrivateRoom);
        }
    }
    validate_room_event_body(event, accepted_events, &state, context)
}

fn validate_mentions(event: &EventV1) -> AcceptResult<()> {
    let Some(mentions) = event.body.get("mentions") else {
        return Ok(());
    };
    let mentions = mentions
        .as_array()
        .ok_or_else(|| AcceptError::Invalid("mentions must be an array".to_string()))?;
    if mentions.len() > 50
        || mentions.iter().any(|mention| {
            !mention
                .as_str()
                .is_some_and(|peer_id| peer_id.starts_with("p:"))
        })
    {
        return Err(AcceptError::Invalid("mentions are invalid".to_string()));
    }
    Ok(())
}

fn valid_message_text(value: &str) -> bool {
    let length = value.chars().count();
    (1..=4000).contains(&length) && value.trim() == value && !value.contains('\0')
}

fn required_scope_for_kind(kind: &str) -> &'static str {
    match kind {
        "MEMBER_JOIN" => "room:join",
        "MEMBER_BAN" | "MEMBER_UNBAN" | "DEVICE_REVOKE" | "INVITE_CREATE" | "INVITE_REVOKE"
        | "SPACE_CREATE" | "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "CHANNEL_DELETE"
        | "CHANNEL_KEY_ROTATE" | "ROLE_CREATE" | "ROLE_UPDATE" | "ROLE_DELETE" | "ROLE_GRANT"
        | "ROLE_REVOKE" => "room:governance",
        k if k.starts_with("CALL_") => "room:call",
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
        && string_body_field(event, "encryption_pub")
            .is_some_and(|key| valid_encryption_public_key(&key))
}

fn member_join_has_authority(
    event: &EventV1,
    state: &GovernanceState,
    context: &RoomContext,
    now_ms: i64,
) -> bool {
    if !context.require_invite || event.author_peer_id == context.authority_peer_id {
        return true;
    }
    let Some(invite_id) = string_body_field(event, "invite_id") else {
        return false;
    };
    state
        .active_invites
        .get(&invite_id)
        .is_some_and(|expires_ms| *expires_ms >= now_ms)
}

fn string_body_field(event: &EventV1, field: &str) -> Option<String> {
    event.body.get(field)?.as_str().map(ToOwned::to_owned)
}

fn int_body_field(event: &EventV1, field: &str) -> Option<i64> {
    event.body.get(field)?.as_i64()
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
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
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
    identity_proof: IdentityProofV1,
    device_id: String,
    device_pub: String,
    not_before_ms: i64,
    expires_ms: i64,
    scopes: Vec<String>,
}

#[derive(Debug)]
struct IdentityGenesisUnsigned {
    v: u8,
    initial_root_pub: String,
    recovery_pub: String,
    nonce: String,
}

impl From<&IdentityGenesisV1> for IdentityGenesisUnsigned {
    fn from(value: &IdentityGenesisV1) -> Self {
        Self {
            v: value.v,
            initial_root_pub: value.initial_root_pub.clone(),
            recovery_pub: value.recovery_pub.clone(),
            nonce: value.nonce.clone(),
        }
    }
}

#[derive(Debug)]
struct IdentityChangeUnsigned {
    v: u8,
    peer_id: String,
    sequence: i64,
    previous: String,
    created_ms: i64,
    author: IdentityChangeAuthor,
    kind: IdentityChangeKind,
}

impl From<&IdentityChangeV1> for IdentityChangeUnsigned {
    fn from(value: &IdentityChangeV1) -> Self {
        Self {
            v: value.v,
            peer_id: value.peer_id.clone(),
            sequence: value.sequence,
            previous: value.previous.clone(),
            created_ms: value.created_ms,
            author: value.author,
            kind: value.kind.clone(),
        }
    }
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
    w.write_bytes(&jcs_bytes(&unsigned.identity_proof)?)?;
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

fn identity_genesis_signature_input(unsigned: &IdentityGenesisUnsigned) -> Result<Vec<u8>> {
    let mut w = NetstringWriter::new(Vec::new());
    w.write_prefix("voxelle/identity-genesis/v1\n")?;
    w.write_int(unsigned.v.into())?;
    w.write_str(&unsigned.initial_root_pub)?;
    w.write_str(&unsigned.recovery_pub)?;
    w.write_str(&unsigned.nonce)?;
    Ok(w.into_inner())
}

fn identity_change_signature_input(unsigned: &IdentityChangeUnsigned) -> Result<Vec<u8>> {
    let mut w = NetstringWriter::new(Vec::new());
    w.write_prefix("voxelle/identity-change/v1\n")?;
    w.write_int(unsigned.v.into())?;
    w.write_str(&unsigned.peer_id)?;
    w.write_int(unsigned.sequence)?;
    w.write_str(&unsigned.previous)?;
    w.write_int(unsigned.created_ms)?;
    w.write_bytes(&jcs_bytes(&unsigned.author)?)?;
    w.write_bytes(&jcs_bytes(&unsigned.kind)?)?;
    Ok(w.into_inner())
}

fn identity_change_id(signature_input: &[u8]) -> String {
    format!("ic:{}", base64url_sha256(signature_input))
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

pub fn verify_signature_from_spki_b64(spki_b64: &str, message: &[u8], sig_b64: &str) -> Result<()> {
    let spki_der = b64_decode(spki_b64).context("decode SPKI public key")?;
    let verifying_key = ed25519_public_key_from_spki_der(&spki_der)?;
    verify_signature(&verifying_key, message, sig_b64)
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
        let delegation = create_delegation(&identity, 900, 2_000, scopes).expect("delegation");
        (identity, delegation)
    }

    fn delegation_for(identity: &PeerIdentity, scopes: Vec<String>) -> DelegationCertV1 {
        create_delegation(identity, 900, 2_000, scopes).expect("delegation")
    }

    fn test_encryption_pub() -> String {
        base64::engine::general_purpose::STANDARD.encode([7_u8; 32])
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
                "encryption_pub": test_encryption_pub(),
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
    fn call_authority_converges_mesh_to_four_and_requires_participant_targets() {
        let identities: Vec<PeerIdentity> = (0..5)
            .map(|_| PeerIdentity::generate_at(1_000).expect("identity"))
            .collect();
        let context = RoomContext::new(identities[0].peer_id.clone());
        let call_id = "call:test";
        let mut accepted = Vec::new();
        for identity in &identities {
            let join = member_join(identity);
            accept_event(&join, &accepted, &context, 1_000).expect("membership accepted");
            accepted.push(join);
        }
        for (index, identity) in identities.iter().enumerate() {
            let join = create_event(
                identity,
                delegation_for(identity, vec!["room:call".to_string()]),
                "room:general",
                1_100 + index as i64,
                "CALL_JOIN",
                compute_heads(&accepted),
                json!({ "call_id": call_id, "video": index % 2 == 0 }),
            )
            .expect("join event");
            accept_event(&join, &accepted, &context, 1_200).expect("concurrent join accepted");
            accepted.push(join);
        }
        let participants = active_call_participants(&accepted, "room:general", call_id, 1_200);
        assert_eq!(participants.len(), 4);
        let mut selected = participants.iter();
        let author_id = selected.next().expect("selected author");
        let target_id = selected.next().expect("selected target");
        let author = identities
            .iter()
            .find(|identity| identity.peer_id == *author_id)
            .expect("author identity");
        let offer = create_event(
            author,
            delegation_for(author, vec!["room:call".to_string()]),
            "room:general",
            1_200,
            "CALL_OFFER",
            compute_heads(&accepted),
            json!({
                "call_id": call_id,
                "target_peer_id": target_id,
                "sdp": "v=0",
                "candidate": null,
            }),
        )
        .expect("offer");
        accept_event(&offer, &accepted, &context, 1_200).expect("participant offer accepted");

        let excluded = identities
            .iter()
            .find(|identity| !participants.contains(&identity.peer_id))
            .expect("excluded fifth peer");
        let excluded_offer = create_event(
            excluded,
            delegation_for(excluded, vec!["room:call".to_string()]),
            "room:general",
            1_201,
            "CALL_OFFER",
            compute_heads(&accepted),
            json!({
                "call_id": call_id,
                "target_peer_id": target_id,
                "sdp": "v=0",
                "candidate": null,
            }),
        )
        .expect("excluded offer");
        assert_eq!(
            accept_event(&excluded_offer, &accepted, &context, 1_201)
                .expect_err("excluded peer cannot signal"),
            AcceptError::NotAuthorized
        );
        assert!(active_call_participants(
            &accepted,
            "room:general",
            call_id,
            1_200 + CALL_LIVENESS_MS + 1,
        )
        .is_empty());
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
    fn principal_id_is_distinct_from_root_and_device_key_ids() {
        let identity = PeerIdentity::generate().expect("identity");

        assert!(identity.peer_id.starts_with("p:"));
        assert!(identity.peer.id.starts_with("ed25519:"));
        assert!(identity.device.id.starts_with("ed25519:"));
        assert_ne!(identity.peer_id, identity.peer.id);
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
    fn recovery_rotates_root_revokes_old_devices_and_preserves_principal() {
        let original = PeerIdentity::generate_at(1_000).expect("original identity");
        let original_peer_id = original.peer_id.clone();
        let original_root_id = original.peer.id.clone();
        let original_device_id = original.device.id.clone();
        let card = original.recovery_card();

        let recovered =
            PeerIdentity::recover(&card, &original.proof, 2_000).expect("recover identity");
        let recovered_state = derive_identity_state(&recovered.proof).expect("recovered state");

        assert_eq!(recovered.peer_id, original_peer_id);
        assert_ne!(recovered.peer.id, original_root_id);
        assert!(!recovered_state.devices.contains_key(&original_device_id));
        assert!(recovered_state.devices.contains_key(&recovered.device.id));
        assert!(identity_proof_extends(&original.proof, &recovered.proof).expect("extension"));

        let delegation = create_delegation(&recovered, 2_000, 3_000, vec!["room:post".to_string()])
            .expect("recovered delegation");
        validate_delegation_at(
            &delegation,
            &original_peer_id,
            &recovered.device.id,
            "room:post",
            2_100,
        )
        .expect("recovered delegation validates");
    }

    #[test]
    fn recovery_key_cannot_authorize_a_device() {
        let identity = PeerIdentity::generate().expect("identity");
        let unauthorized_device = Keypair::generate().expect("device");
        let mut proof = identity.proof.clone();
        let result = append_identity_change(
            &mut proof,
            &identity.recovery,
            IdentityChangeAuthor::Recovery,
            IdentityChangeKind::DeviceAuthorize {
                device_id: unauthorized_device.id,
                device_pub: unauthorized_device.spki_b64,
                scopes: vec!["room:post".to_string()],
                expires_ms: i64::MAX,
            },
            1_000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn signed_space_is_reconstructed_from_its_genesis_fact() {
        let authority = PeerIdentity::generate_at(900).expect("authority");
        let space = create_space(&authority, "Friends", "general", 1_000).expect("space");
        assert_eq!(
            space_from_genesis(&space.genesis, 1_100).expect("reconstruct space"),
            space
        );

        let mut tampered = space.genesis.clone();
        tampered.body["default_room_id"] = json!("room:attacker");
        assert!(space_from_genesis(&tampered, 1_100).is_err());
    }

    #[test]
    fn signed_space_invite_is_required_for_strict_membership() {
        let authority = PeerIdentity::generate_at(900).expect("authority");
        let member = PeerIdentity::generate_at(900).expect("member");
        let space = create_space(&authority, "Friends", "general", 1_000).expect("space");
        let context =
            RoomContext::for_space(authority.peer_id.clone(), space.governance_room_id.clone());
        let invite = create_space_invite_event(
            &authority,
            &space,
            vec![json!({ "endpoint": "signed bootstrap" })],
            1_500,
            1_100,
            vec![space.genesis.event_id.clone()],
        )
        .expect("invite");
        validate_space_invite_at(&space, &invite, 1_200).expect("valid invite");

        let without_invite = create_event(
            &member,
            delegation_for(&member, vec!["room:join".to_string()]),
            &space.governance_room_id,
            1_200,
            "MEMBER_JOIN",
            vec![invite.event_id.clone()],
            json!({
                "peer_id": member.peer_id,
                "peer_pub": member.peer.spki_b64,
                "encryption_pub": test_encryption_pub(),
            }),
        )
        .expect("join without invite");
        assert_eq!(
            accept_event(
                &without_invite,
                &[space.genesis.clone(), invite.clone()],
                &context,
                1_200,
            )
            .expect_err("invite required"),
            AcceptError::NotAuthorized
        );

        let with_invite = create_event(
            &member,
            delegation_for(&member, vec!["room:join".to_string()]),
            &space.governance_room_id,
            1_200,
            "MEMBER_JOIN",
            vec![invite.event_id.clone()],
            json!({
                "peer_id": member.peer_id,
                "peer_pub": member.peer.spki_b64,
                "encryption_pub": test_encryption_pub(),
                "invite_id": invite.event_id,
            }),
        )
        .expect("join with invite");
        accept_event(
            &with_invite,
            &[space.genesis.clone(), invite.clone()],
            &context,
            1_200,
        )
        .expect("invite admits member");

        assert!(validate_space_invite_at(&space, &invite, 1_501).is_err());
        accept_event(
            &with_invite,
            &[space.genesis.clone(), invite.clone()],
            &context,
            1_501,
        )
        .expect("historical join remains valid after invite expiry");

        let late_join = create_event(
            &member,
            delegation_for(&member, vec!["room:join".to_string()]),
            &space.governance_room_id,
            1_501,
            "MEMBER_JOIN",
            vec![invite.event_id.clone()],
            json!({
                "peer_id": member.peer_id,
                "peer_pub": member.peer.spki_b64,
                "encryption_pub": test_encryption_pub(),
                "invite_id": invite.event_id,
            }),
        )
        .expect("late join");
        assert_eq!(
            accept_event(&late_join, &[space.genesis, invite], &context, 1_501)
                .expect_err("join signed after expiry is rejected"),
            AcceptError::NotAuthorized
        );
    }

    #[test]
    fn retained_event_survives_delegation_expiry_and_future_events_are_bounded() {
        let authority = PeerIdentity::generate_at(900).expect("authority");
        let context = RoomContext::new(authority.peer_id.clone());
        let join = member_join(&authority);
        let retained = message(&authority, 1_100, vec![]);

        accept_event(&retained, std::slice::from_ref(&join), &context, 50_000)
            .expect("historical event remains valid after delegation expiry");

        let future = message(&authority, 500_000, vec![]);
        assert!(matches!(
            accept_event(&future, &[join], &context, 1_100),
            Err(AcceptError::Invalid(reason)) if reason.contains("future")
        ));
    }

    #[test]
    fn tampered_or_regressed_identity_proof_is_rejected() {
        let original = PeerIdentity::generate_at(1_000).expect("original identity");
        let recovered = PeerIdentity::recover(&original.recovery_card(), &original.proof, 2_000)
            .expect("recover identity");

        assert!(!identity_proof_extends(&recovered.proof, &original.proof).expect("regression"));

        let mut tampered = recovered.proof.clone();
        let last = tampered.changes.last_mut().expect("identity change");
        last.created_ms += 1;
        assert!(derive_identity_state(&tampered).is_err());
    }

    #[test]
    fn device_delegation_verifies_and_binds_ids() {
        let (identity, delegation) = identity_with_delegation();

        validate_delegation_at(
            &delegation,
            &identity.peer_id,
            &identity.device.id,
            "room:post",
            1_000,
        )
        .expect("delegation validates");

        let wrong = validate_delegation_at(
            &delegation,
            &identity.peer_id,
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
        let context = RoomContext::new(authority.peer_id);
        let event = message(&member, 1_100, vec![]);

        let err = accept_event(&event, &[], &context, 1_100).expect_err("not accepted");
        assert_eq!(err, AcceptError::NotMember);
    }

    #[test]
    fn member_join_admits_peer_and_member_message_is_accepted() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id);

        let join = member_join(&member);
        accept_event(&join, &[], &context, 1_000).expect("join accepted");

        let event = message(&member, 1_100, vec![]);
        accept_event(&event, &[join], &context, 1_100).expect("message accepted");
    }

    #[test]
    fn banned_peer_cannot_post() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id.clone());

        let join = member_join(&member);
        let ban = authority_governance_event(
            &authority,
            1_050,
            "MEMBER_BAN",
            json!({ "peer_id": member.peer_id }),
        );
        accept_event(&ban, std::slice::from_ref(&join), &context, 1_050).expect("ban accepted");

        let event = message(&member, 1_100, vec![]);
        let err = accept_event(&event, &[join, ban], &context, 1_100).expect_err("banned");
        assert_eq!(err, AcceptError::Banned);
    }

    #[test]
    fn revoked_device_cannot_post() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id.clone());

        let join = member_join(&member);
        let revoke = authority_governance_event(
            &authority,
            1_050,
            "DEVICE_REVOKE",
            json!({
                "peer_id": member.peer_id,
                "device_id": member.device.id,
            }),
        );
        accept_event(&revoke, std::slice::from_ref(&join), &context, 1_050)
            .expect("revoke accepted");

        let event = message(&member, 1_100, vec![]);
        let err = accept_event(&event, &[join, revoke], &context, 1_100).expect_err("revoked");
        assert_eq!(err, AcceptError::DeviceRevoked);
    }

    #[test]
    fn unknown_kind_does_not_bypass_membership() {
        let authority = PeerIdentity::generate().expect("authority");
        let outsider = PeerIdentity::generate().expect("outsider");
        let context = RoomContext::new(authority.peer_id);
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
        let context = RoomContext::new(authority.peer_id);
        let join = member_join(&member);
        let event = message(&member, 1_100, vec!["e:missing".to_string()]);

        accept_event(&event, &[join], &context, 1_100).expect("missing ancestor tolerated");
    }

    #[test]
    fn message_redaction_is_terminal_for_later_edits() {
        let authority = PeerIdentity::generate().expect("authority");
        let context = RoomContext::new(authority.peer_id.clone());
        let join = member_join(&authority);
        let post = create_event(
            &authority,
            delegation_for(&authority, vec!["room:post".to_string()]),
            "room:general",
            1_000,
            "MSG_POST",
            vec![],
            json!({"text":"original","mentions":[]}),
        )
        .expect("post");
        accept_event(&post, std::slice::from_ref(&join), &context, 1_000).expect("post accepted");
        let redact = create_event(
            &authority,
            delegation_for(&authority, vec!["room:post".to_string()]),
            "room:general",
            1_010,
            "MSG_REDACT",
            vec![post.event_id.clone()],
            json!({"target_event_id":post.event_id}),
        )
        .expect("redact");
        accept_event(&redact, &[join.clone(), post.clone()], &context, 1_010)
            .expect("redaction accepted");
        let edit = create_event(
            &authority,
            delegation_for(&authority, vec!["room:post".to_string()]),
            "room:general",
            1_020,
            "MSG_EDIT",
            vec![redact.event_id.clone()],
            json!({"target_event_id":post.event_id,"text":"restored","mentions":[]}),
        )
        .expect("edit");
        let error = accept_event(&edit, &[join, post, redact], &context, 1_020)
            .expect_err("redacted message cannot be restored");
        assert!(matches!(error, AcceptError::Invalid(message) if message.contains("redacted")));
    }

    #[test]
    fn governance_derivation_is_deterministic_from_shuffled_input() {
        let authority = PeerIdentity::generate().expect("authority");
        let member = PeerIdentity::generate().expect("member");
        let context = RoomContext::new(authority.peer_id.clone());
        let join = member_join(&member);
        let ban = authority_governance_event(
            &authority,
            1_050,
            "MEMBER_BAN",
            json!({ "peer_id": member.peer_id }),
        );
        let unban = authority_governance_event(
            &authority,
            1_060,
            "MEMBER_UNBAN",
            json!({ "peer_id": member.peer_id }),
        );

        let a =
            derive_governance_state(&[join.clone(), ban.clone(), unban.clone()], &context, 1_100);
        let b = derive_governance_state(&[unban, join, ban], &context, 1_100);
        assert_eq!(a, b);
        assert!(!a.banned.contains(&member.peer_id));
        assert!(!a.members.contains(&member.peer_id));
    }
}
