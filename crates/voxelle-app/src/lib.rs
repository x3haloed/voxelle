use anyhow::{Context, Result};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use ts_rs::TS;
use voxelle_core::{
    accept_event, create_delegation, create_event, create_space, create_space_invite_event,
    derive_governance_state, space_from_genesis, topo_sort_deterministic,
    validate_room_event_semantics, validate_space_at, validate_space_invite_at, ChannelVisibility,
    EventV1, IdentityProofV1, PeerIdentity, RecoveryCardV1, RoomContext, SpaceV1,
};
use voxelle_net::{
    AddressScope, LocalReachabilityReport, PeerEndpoint, PeerReachabilityReport, QuicCertificate,
    QuicNode, RoomSync, ServedPeerRequest,
};
use voxelle_store::Store;
use voxelle_sync::{merge_stats, SyncLimits, SyncStats};
use voxelle_update::{
    ActiveSource, AvailableProductUpdate, DownloadedProductUpdate, GenerationPointerV1,
    TrustedReleaseKey, TrustedReleaseKeysV1, UpdateManager, VerifiedPackage,
};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};

mod shell;

pub use shell::{ShellError, ShellResult, ShellState};

pub const DEFAULT_ROOM_ID: &str = "room:general";
const CALL_LIVENESS_MS: i64 = 90_000;
const HOME_SELECTION_STATE: &str = "home.selected_space";
const KNOWN_PEERS_STATE: &str = "peers.known";
const READ_STATE: &str = "rooms.read";
const ROOM_KEYS_STATE: &str = "rooms.keys.encrypted";
const UI_PREFERENCES_STATE: &str = "ui.preferences";
const SERVICE_EVENT_QUEUE_CAPACITY: usize = 128;
const MAX_KNOWN_PEERS: usize = 128;
const MAX_PROJECTED_MESSAGES: usize = 500;
const MAX_PROJECTED_CALL_SIGNALS: usize = 256;

pub fn resolve_home_root(explicit: Option<PathBuf>) -> PathBuf {
    resolve_home_root_from(
        explicit,
        std::env::var_os("VOXELLE_HOME_ROOT").map(PathBuf::from),
        dirs::home_dir(),
    )
}

fn resolve_home_root_from(
    explicit: Option<PathBuf>,
    configured: Option<PathBuf>,
    platform_home: Option<PathBuf>,
) -> PathBuf {
    explicit.or(configured).unwrap_or_else(|| {
        platform_home
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".voxelle")
    })
}

#[derive(Debug, Clone)]
pub struct VoxelleHome {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HomeConfig {
    pub space: SpaceV1,
}

impl HomeConfig {
    fn room_context(&self) -> RoomContext {
        RoomContext::for_space(
            self.space.authority_peer_id.clone(),
            self.space.governance_room_id.clone(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct HomeSelectionV1 {
    v: u8,
    space_genesis_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityFile {
    v: u8,
    nonce_b64: String,
    ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct IdentitySecretsV1 {
    v: u8,
    root_secret_b64: String,
    device_secret_b64: String,
    recovery_secret_b64: String,
    proof: IdentityProofV1,
    peer_id: String,
    device_id: String,
}

impl IdentitySecretsV1 {
    fn from_identity(identity: &PeerIdentity) -> Self {
        Self {
            v: 1,
            root_secret_b64: identity.peer.secret_key_b64(),
            device_secret_b64: identity.device.secret_key_b64(),
            recovery_secret_b64: identity.recovery.secret_key_b64(),
            proof: identity.proof.clone(),
            peer_id: identity.peer_id.clone(),
            device_id: identity.device.id.clone(),
        }
    }

    fn to_identity(&self) -> Result<PeerIdentity> {
        if self.v != 1 {
            anyhow::bail!("unsupported identity version {}", self.v);
        }
        let identity = PeerIdentity::from_secret_keys_b64(
            &self.root_secret_b64,
            &self.device_secret_b64,
            &self.recovery_secret_b64,
            self.proof.clone(),
        )?;
        if identity.peer_id != self.peer_id || identity.device.id != self.device_id {
            anyhow::bail!("identity metadata does not match signed identity proof");
        }
        Ok(identity)
    }
}

impl IdentityFile {
    fn encrypt(identity: &PeerIdentity, key: &[u8; 32]) -> Result<Self> {
        let secrets = IdentitySecretsV1::from_identity(identity);
        let plaintext = serde_json::to_vec(&secrets).context("serialize identity secrets")?;
        let cipher = XChaCha20Poly1305::new(key.into());
        let mut nonce = [0_u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &plaintext,
                    aad: b"voxelle/identity-vault/v1",
                },
            )
            .map_err(|_| anyhow::anyhow!("encrypt identity vault"))?;
        Ok(Self {
            v: 1,
            nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce),
            ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        })
    }

    fn decrypt(&self, key: &[u8; 32]) -> Result<PeerIdentity> {
        if self.v != 1 {
            anyhow::bail!("unsupported identity vault version {}", self.v);
        }
        let nonce = base64::engine::general_purpose::STANDARD
            .decode(&self.nonce_b64)
            .context("decode identity vault nonce")?;
        let nonce: [u8; 24] = nonce
            .try_into()
            .map_err(|_| anyhow::anyhow!("identity vault nonce must be 24 bytes"))?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&self.ciphertext_b64)
            .context("decode identity vault ciphertext")?;
        let cipher = XChaCha20Poly1305::new(key.into());
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &ciphertext,
                    aad: b"voxelle/identity-vault/v1",
                },
            )
            .map_err(|_| anyhow::anyhow!("identity vault authentication failed"))?;
        let secrets: IdentitySecretsV1 =
            serde_json::from_slice(&plaintext).context("parse identity secrets")?;
        secrets.to_identity()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ProfileSummary {
    #[ts(type = "string")]
    pub home: PathBuf,
    pub peer_id: String,
    pub device_id: String,
    pub default_room: String,
    pub authority_peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct MessageView {
    pub event_id: String,
    #[ts(type = "number")]
    pub created_ms: i64,
    pub author_peer_id: String,
    pub text: String,
    #[ts(type = "number | null")]
    pub edited_ms: Option<i64>,
    pub redacted: bool,
    pub mentions: Vec<String>,
    pub thread_root_event_id: Option<String>,
    pub reply_count: usize,
    pub pinned: bool,
    pub reactions: Vec<ReactionView>,
    pub attachments: Vec<AttachmentView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ReactionView {
    pub emoji: String,
    pub peer_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct AttachmentView {
    pub event_id: String,
    pub filename: String,
    pub mime: String,
    pub sha256: String,
    pub data_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ChannelView {
    pub room_id: String,
    pub name: String,
    pub topic: String,
    pub visibility: String,
    pub selected: bool,
    pub unread_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct RoleView {
    pub role_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub member_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ProfileView {
    pub peer_id: String,
    pub display_name: String,
    pub about: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SearchResultView {
    pub room_id: String,
    pub message: MessageView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct NotificationView {
    pub event_id: String,
    pub room_id: String,
    pub author_peer_id: String,
    pub summary: String,
    pub kind: String,
    #[ts(type = "number")]
    pub created_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CallSignalView {
    pub event_id: String,
    pub kind: String,
    pub call_id: String,
    pub author_peer_id: String,
    pub target_peer_id: Option<String>,
    pub video: Option<bool>,
    pub sdp: Option<String>,
    pub candidate: Option<String>,
    #[ts(type = "number")]
    pub created_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CallView {
    pub call_id: String,
    pub participants: Vec<String>,
    pub signals: Vec<CallSignalView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct PeerRecord {
    pub v: u8,
    pub label: Option<String>,
    pub space_id: String,
    pub governance_room_id: String,
    pub default_room: String,
    pub authority_peer_id: String,
    pub endpoint: PeerEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct KnownPeersFile {
    v: u8,
    peers: Vec<PeerRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct ReadStateFile {
    v: u8,
    last_read_event_ids: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct EncryptedRoomKeysFile {
    v: u8,
    nonce_b64: String,
    ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RoomKeysV1 {
    v: u8,
    keys: BTreeMap<String, BTreeMap<u64, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PrivateEventPlaintext {
    kind: String,
    body: serde_json::Value,
}

impl Default for RoomKeysV1 {
    fn default() -> Self {
        Self {
            v: 1,
            keys: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryCapsuleV1 {
    pub v: u8,
    pub peer_id: String,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryKitV1 {
    pub v: u8,
    pub card: RecoveryCardV1,
    pub capsule: RecoveryCapsuleV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RecoveryPayloadV1 {
    v: u8,
    identity_proof: IdentityProofV1,
    space: SpaceV1,
    governance_events: Vec<EventV1>,
    known_peers: Vec<PeerRecord>,
    ui_preferences: UiPreferences,
    read_state: ReadStateFile,
    room_keys: RoomKeysV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryReport {
    pub profile: ProfileSummary,
    pub peers_attempted: usize,
    pub peers_reached: usize,
    pub events_recovered: usize,
    pub events_pushed: usize,
    pub peer_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceInviteFileV1 {
    pub v: u8,
    pub space: SpaceV1,
    pub invite_event: EventV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JoinSpaceReport {
    pub profile: ProfileSummary,
    pub invite_id: String,
    pub peers_attempted: usize,
    pub peers_reached: usize,
    pub events_received: usize,
    pub events_pushed: usize,
    pub peer_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct UiOntologyView {
    pub places: Vec<UiPlace>,
    pub views: Vec<UiView>,
    pub commands: Vec<UiCommand>,
    pub semantic_tokens: Vec<SemanticToken>,
    pub metrics: Vec<UiMetric>,
    pub behaviors: Vec<UiBehavior>,
    pub renderers: Vec<UiRenderer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct ProductGenerationV1 {
    pub v: u8,
    pub ontology: UiOntologyView,
    pub component: ProductComponentV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ProductComponentV1 {
    pub api_version: u8,
    pub source: String,
    pub styles: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ProductComponentView {
    pub api_version: u8,
    pub digest: String,
    pub source: String,
    pub styles: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ProductGenerationStatusView {
    pub kernel_version: String,
    pub active_release_id: String,
    pub active_sequence: u64,
    pub source: String,
    pub previous_available: bool,
    pub update_authentication_available: bool,
    pub trusted_update_key_count: usize,
    pub trust_sequence: u64,
    pub available_release_id: Option<String>,
    pub available_sequence: Option<u64>,
    pub staged_release_id: Option<String>,
    pub staged_sequence: Option<u64>,
    pub phase: String,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct UiPlace {
    pub id: String,
    pub label: String,
    pub description: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct UiView {
    pub id: String,
    pub label: String,
    pub default_place_id: String,
    pub place_id: String,
    pub order: usize,
    pub visible: bool,
    pub description: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct UiCommand {
    pub id: String,
    pub label: String,
    pub description: String,
    pub scope: UiCommandScope,
    pub shortcut: Option<String>,
    pub palette: bool,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum UiCommandScope {
    Shell,
    Frontend,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct UiViewPlacement {
    pub view_id: String,
    pub place_id: String,
    pub order: usize,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SemanticToken {
    pub id: String,
    pub label: String,
    pub default_value: String,
    pub current_value: String,
    pub used_by: Vec<String>,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct UiMetric {
    pub id: String,
    pub label: String,
    pub default_value: f64,
    pub current_value: f64,
    pub unit: String,
    pub used_by: Vec<String>,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct UiBehavior {
    pub id: String,
    pub label: String,
    pub default_value: UiBehaviorValue,
    pub current_value: UiBehaviorValue,
    pub used_by: Vec<String>,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct UiRenderer {
    pub id: String,
    pub label: String,
    pub renders: String,
    pub default_renderer: String,
    pub current_renderer: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum UiBehaviorValue {
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiPreferenceKind {
    SemanticToken,
    Metric,
    Behavior,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiPreferences {
    pub v: u8,
    pub semantic_tokens: BTreeMap<String, String>,
    pub metrics: BTreeMap<String, f64>,
    pub behaviors: BTreeMap<String, UiBehaviorValue>,
    pub view_placements: BTreeMap<String, UiViewPlacement>,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            v: 1,
            semantic_tokens: BTreeMap::new(),
            metrics: BTreeMap::new(),
            behaviors: BTreeMap::new(),
            view_placements: BTreeMap::new(),
        }
    }
}

pub fn shell_contract_typescript() -> String {
    let cfg = ts_rs::Config::default();
    let declarations = [
        PeerEndpoint::decl(&cfg),
        ProfileSummary::decl(&cfg),
        MessageView::decl(&cfg),
        ReactionView::decl(&cfg),
        AttachmentView::decl(&cfg),
        ChannelView::decl(&cfg),
        RoleView::decl(&cfg),
        ProfileView::decl(&cfg),
        SearchResultView::decl(&cfg),
        NotificationView::decl(&cfg),
        CallSignalView::decl(&cfg),
        CallView::decl(&cfg),
        PeerRecord::decl(&cfg),
        UiOntologyView::decl(&cfg),
        ProductGenerationV1::decl(&cfg),
        ProductComponentV1::decl(&cfg),
        ProductComponentView::decl(&cfg),
        ProductGenerationStatusView::decl(&cfg),
        UiPlace::decl(&cfg),
        UiView::decl(&cfg),
        UiCommand::decl(&cfg),
        UiCommandScope::decl(&cfg),
        UiViewPlacement::decl(&cfg),
        SemanticToken::decl(&cfg),
        UiMetric::decl(&cfg),
        UiBehavior::decl(&cfg),
        UiRenderer::decl(&cfg),
        UiBehaviorValue::decl(&cfg),
        ShellSnapshotView::decl(&cfg),
        ServiceActivityItem::decl(&cfg),
        ServiceActivityLevel::decl(&cfg),
        InitHomeRequest::decl(&cfg),
        StartServiceRequest::decl(&cfg),
        SendMessageRequest::decl(&cfg),
        SelectChannelRequest::decl(&cfg),
        MarkReadRequest::decl(&cfg),
        CreateChannelRequest::decl(&cfg),
        RotateChannelKeyRequest::decl(&cfg),
        CallJoinRequest::decl(&cfg),
        CallSignalRequest::decl(&cfg),
        CallLeaveRequest::decl(&cfg),
        MessageTargetRequest::decl(&cfg),
        EditMessageRequest::decl(&cfg),
        ReactionRequest::decl(&cfg),
        AttachmentRequest::decl(&cfg),
        ProfileUpdateRequest::decl(&cfg),
        CreateRoleRequest::decl(&cfg),
        AssignRoleRequest::decl(&cfg),
        BanMemberRequest::decl(&cfg),
        SearchMessagesRequest::decl(&cfg),
        ImportPeerRecordRequest::decl(&cfg),
        CreateSpaceInviteRequest::decl(&cfg),
        JoinSpaceRequest::decl(&cfg),
        PeerCommandRequest::decl(&cfg),
        SetUiPreferenceRequest::decl(&cfg),
        SetWorkbenchLayoutRequest::decl(&cfg),
        InstallProductUpdateRequest::decl(&cfg),
        HomeScreenView::decl(&cfg),
        NetworkHealthView::decl(&cfg),
        NetworkHealthRow::decl(&cfg),
        NetworkHealthStatus::decl(&cfg),
        RuntimeStatusView::decl(&cfg),
        RuntimeState::decl(&cfg),
        InviteExchangeView::decl(&cfg),
        PeerListItemView::decl(&cfg),
        RoomTimelineView::decl(&cfg),
    ];
    let mut output = typescript_module(declarations);
    output.push_str("export ");
    output.push_str(&ShellError::decl(&cfg));
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

pub fn write_shell_contract(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, shell_contract_typescript())?;
    Ok(())
}

pub fn ui_ontology_fixture_javascript() -> Result<String> {
    let ontology = builtin_product_generation().ontology;
    Ok(format!(
        "// This file is generated from the Rust UI ontology. Do not edit by hand.\n\nexport const defaultUiOntology = {};\n",
        serde_json::to_string(&ontology)?
    ))
}

pub fn builtin_product_generation() -> ProductGenerationV1 {
    ProductGenerationV1 {
        v: 1,
        ontology: default_ui_ontology(UiPreferences::default()),
        component: ProductComponentV1 {
            api_version: 1,
            source: builtin_product_component_source(),
            styles: include_str!("../../../web/src/styles.css").to_string(),
        },
    }
}

fn builtin_product_component_source() -> String {
    const MODULES: [&str; 4] = [
        include_str!("../../../web/src/call-media.mjs"),
        include_str!("../../../web/src/dom-reconcile.mjs"),
        include_str!("../../../web/src/ui-ontology.mjs"),
        include_str!("../../../web/src/workbench.mjs"),
    ];
    let mut source = String::from("// Signed Voxelle product component modules.\n");
    for module in MODULES {
        source.push_str(&module.replace("export ", ""));
        source.push('\n');
    }
    source.push_str(include_str!("../../../web/src/product-component.js"));
    source
}

pub fn write_ui_ontology_fixture(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, ui_ontology_fixture_javascript()?)?;
    Ok(())
}

#[derive(Debug)]
struct PeerServer {
    home: VoxelleHome,
    node: QuicNode,
    online: OnlineHome,
}

#[derive(Debug)]
pub struct VoxelleService {
    online: OnlineHome,
    events: mpsc::Receiver<VoxelleServiceEvent>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

pub struct VoxelleCommandHost {
    home: VoxelleHome,
    service: Option<VoxelleService>,
    activity: Vec<ServiceActivityItem>,
    next_activity_id: u64,
    last_space_invite_json: Option<String>,
    selected_room_id: Option<String>,
    search_results: Vec<SearchResultView>,
    snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
    update_manager: UpdateManager,
    product_generation: Option<ActiveProductGeneration>,
    product_generation_notice: Option<String>,
    available_product_update: Option<AvailableProductUpdate>,
    update_phase: String,
}

#[derive(Debug, Clone)]
struct ActiveProductGeneration {
    pointer: GenerationPointerV1,
    generation: ProductGenerationV1,
    source: ActiveSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelleServiceEvent {
    Served(Box<ServedPeerRequest>),
    Failed(String),
    Stopped,
}

impl VoxelleServiceEvent {
    pub fn summary(&self) -> String {
        match self {
            VoxelleServiceEvent::Served(served) => match served.as_ref() {
                ServedPeerRequest::Diagnostic(report) if report.reachable => {
                    let remote = report
                        .remote
                        .as_ref()
                        .map(|remote| short_peer_label(&remote.peer_id))
                        .unwrap_or_else(|| "peer".to_string());
                    format!("served diagnostic: {remote} reached this home")
                }
                ServedPeerRequest::Diagnostic(report) => {
                    format!(
                        "served diagnostic: unreachable ({})",
                        report.error.as_deref().unwrap_or("no error detail")
                    )
                }
                ServedPeerRequest::RoomSync(sync) => {
                    let truncated = if sync.truncated { ", truncated" } else { "" };
                    format!(
                        "served sync: room {}, offered {}, accepted {}, rejected {} event(s){}",
                        sync.room_id,
                        sync.offered,
                        sync.accepted_from_remote,
                        sync.rejected_from_remote,
                        truncated
                    )
                }
            },
            VoxelleServiceEvent::Failed(error) => format!("service error: {error}"),
            VoxelleServiceEvent::Stopped => "service stopped".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OnlineHome {
    pub endpoint: PeerEndpoint,
    pub local_report: LocalReachabilityReport,
    pub default_room: String,
    pub authority_peer_id: String,
    pub space_id: String,
    pub governance_room_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSyncReport {
    pub governance: SyncStats,
    pub room: SyncStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct ShellSnapshotView {
    #[ts(type = "string")]
    pub home_root: PathBuf,
    pub home: Option<HomeScreenView>,
    pub home_error: Option<String>,
    pub network_health: NetworkHealthView,
    pub ui_ontology: UiOntologyView,
    pub product_generation: ProductGenerationStatusView,
    pub product_component: ProductComponentView,
    pub service_activity: Vec<ServiceActivityItem>,
    pub search_results: Vec<SearchResultView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ServiceActivityItem {
    #[ts(type = "number")]
    pub id: u64,
    pub level: ServiceActivityLevel,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum ServiceActivityLevel {
    Info,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct InitHomeRequest {
    pub default_room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct StartServiceRequest {
    #[ts(type = "string | null")]
    pub bind: Option<SocketAddr>,
    #[ts(type = "string | null")]
    pub advertise: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SendMessageRequest {
    pub text: String,
    pub room: Option<String>,
    #[serde(default)]
    pub mentions: Vec<String>,
    #[serde(default)]
    pub thread_root_event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SelectChannelRequest {
    pub room_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct MarkReadRequest {
    pub room_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CreateChannelRequest {
    pub name: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub private_members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct RotateChannelKeyRequest {
    pub room_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CallJoinRequest {
    pub room: Option<String>,
    #[serde(default)]
    pub video: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CallSignalRequest {
    pub room: Option<String>,
    pub call_id: String,
    pub target_peer_id: String,
    pub signal_type: String,
    #[serde(default)]
    pub sdp: Option<String>,
    #[serde(default)]
    pub candidate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CallLeaveRequest {
    pub room: Option<String>,
    pub call_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct MessageTargetRequest {
    pub target_event_id: String,
    pub room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct EditMessageRequest {
    pub target_event_id: String,
    pub text: String,
    pub room: Option<String>,
    #[serde(default)]
    pub mentions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ReactionRequest {
    pub target_event_id: String,
    pub emoji: String,
    pub room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct AttachmentRequest {
    pub filename: String,
    pub mime: String,
    pub data_b64: String,
    pub room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ProfileUpdateRequest {
    pub display_name: String,
    #[serde(default)]
    pub about: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CreateRoleRequest {
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct AssignRoleRequest {
    pub peer_id: String,
    pub role_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct BanMemberRequest {
    pub peer_id: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SearchMessagesRequest {
    pub query: String,
    pub room: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ImportPeerRecordRequest {
    pub peer_record_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CreateSpaceInviteRequest {
    #[ts(type = "number | null")]
    pub expires_minutes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct JoinSpaceRequest {
    pub space_invite_json: String,
    pub max_events: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct PeerCommandRequest {
    pub peer_id: String,
    pub device_id: String,
    pub max_events: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetUiPreferenceRequest {
    SemanticToken { id: String, value: String },
    Metric { id: String, value: f64 },
    Behavior { id: String, value: UiBehaviorValue },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SetWorkbenchLayoutRequest {
    pub placements: Vec<UiViewPlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct InstallProductUpdateRequest {
    pub package_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct InstallTrustTransitionRequest {
    pub transition_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct HomeScreenView {
    pub profile: ProfileSummary,
    pub runtime: RuntimeStatusView,
    pub invite: Option<InviteExchangeView>,
    pub peers: Vec<PeerListItemView>,
    pub channels: Vec<ChannelView>,
    pub roles: Vec<RoleView>,
    pub profiles: Vec<ProfileView>,
    pub notifications: Vec<NotificationView>,
    pub call: CallView,
    pub room: RoomTimelineView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct NetworkHealthView {
    pub rows: Vec<NetworkHealthRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct NetworkHealthRow {
    pub id: String,
    pub label: String,
    pub status: NetworkHealthStatus,
    pub summary: String,
    pub primary_action: Option<String>,
    pub details: Vec<String>,
    pub related_views: Vec<String>,
    pub related_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkHealthStatus {
    Unknown,
    Working,
    NeedsAttention,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct RuntimeStatusView {
    pub state: RuntimeState,
    #[ts(type = "string | null")]
    pub listen_addr: Option<SocketAddr>,
    #[ts(type = "string | null")]
    pub advertised_addr: Option<SocketAddr>,
    pub reachability_notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Offline,
    Online,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct InviteExchangeView {
    pub peer_record: PeerRecord,
    pub peer_record_json: String,
    pub space_invite_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct PeerListItemView {
    pub label: String,
    pub peer_id: String,
    pub device_id: String,
    #[ts(type = "string")]
    pub addr: SocketAddr,
    pub default_room: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct RoomTimelineView {
    pub room_id: String,
    pub messages: Vec<MessageView>,
}

impl VoxelleHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn init(&self, default_room: impl Into<String>) -> Result<ProfileSummary> {
        ensure_private_dir(&self.root)?;
        let identity = self.load_or_create_identity()?;
        self.load_or_create_certificate()?;

        let default_room = default_room.into();
        let config = if self.local_state_exists(HOME_SELECTION_STATE)? {
            self.load_config()?
        } else {
            let channel_name = default_room
                .rsplit(':')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or("general");
            let space = create_space(&identity, "My Space", channel_name, now_ms())?;
            HomeConfig { space }
        };

        let store = self.open_store()?;
        self.ensure_space_genesis(&store, &config)?;
        self.put_local_state(
            HOME_SELECTION_STATE,
            &HomeSelectionV1 {
                v: 1,
                space_genesis_event_id: config.space.genesis.event_id.clone(),
            },
        )?;
        self.ensure_member_join(&store, &identity, &config, None)?;

        Ok(ProfileSummary {
            home: self.root.clone(),
            peer_id: identity.peer_id,
            device_id: identity.device.id,
            default_room: config.space.default_room_id,
            authority_peer_id: config.space.authority_peer_id,
        })
    }

    pub fn profile_summary(&self) -> Result<ProfileSummary> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        Ok(ProfileSummary {
            home: self.root.clone(),
            peer_id: identity.peer_id,
            device_id: identity.device.id,
            default_room: config.space.default_room_id,
            authority_peer_id: config.space.authority_peer_id,
        })
    }

    pub fn home_screen_view(&self, online: Option<&OnlineHome>) -> Result<HomeScreenView> {
        self.home_screen_view_for_room(online, None)
    }

    pub fn home_screen_view_for_room(
        &self,
        online: Option<&OnlineHome>,
        selected_room: Option<&str>,
    ) -> Result<HomeScreenView> {
        let config = self.load_config()?;
        let invite = online
            .map(|online| online.invite_view(None, None))
            .transpose()?;
        let runtime = online
            .map(RuntimeStatusView::online)
            .unwrap_or_else(RuntimeStatusView::offline);
        let channels = self.channels(selected_room)?;
        let selected_room = channels
            .iter()
            .find(|channel| channel.selected)
            .map(|channel| channel.room_id.clone())
            .unwrap_or_else(|| config.space.default_room_id.clone());
        let mut projected_messages = self.read_messages(Some(&selected_room))?;
        retain_latest(&mut projected_messages, MAX_PROJECTED_MESSAGES);
        Ok(HomeScreenView {
            profile: self.profile_summary()?,
            runtime,
            invite,
            peers: self
                .known_peers()?
                .into_iter()
                .map(PeerListItemView::from_peer_record)
                .collect(),
            channels,
            roles: self.roles()?,
            profiles: self.profiles()?,
            notifications: self.notifications()?,
            call: self.call_view(&selected_room)?,
            room: RoomTimelineView {
                room_id: selected_room.clone(),
                messages: projected_messages,
            },
        })
    }

    pub fn network_health_view(&self, online: Option<&OnlineHome>) -> Result<NetworkHealthView> {
        let home_status = match self.load_config() {
            Ok(config) => NetworkHealthRow::working(
                "home",
                "Home",
                format!("Home is initialized for {}.", config.space.default_room_id),
            )
            .detail(format!("root: {}", self.root.display())),
            Err(error)
                if self
                    .local_state_exists(HOME_SELECTION_STATE)
                    .unwrap_or_else(|_| self.path("store.sqlite3").exists()) =>
            {
                NetworkHealthRow::broken(
                    "home",
                    "Home",
                    "Home exists but cannot be read.",
                    Some("home.init"),
                )
                .detail(format!("{error:#}"))
                .related_command("home.init")
            }
            Err(_) => NetworkHealthRow::needs_attention(
                "home",
                "Home",
                "Create the local home before networking can start.",
                Some("home.init"),
            )
            .detail(format!("root: {}", self.root.display()))
            .related_command("home.init"),
        }
        .related_view("profile.summary");

        let identity_status = match self.load_identity() {
            Ok(identity) => NetworkHealthRow::working(
                "identity",
                "Identity",
                format!(
                    "Local peer {} is available.",
                    short_peer_label(&identity.peer_id)
                ),
            )
            .detail(format!("device: {}", short_peer_label(&identity.device.id))),
            Err(error) if self.path("identity.json").exists() => NetworkHealthRow::broken(
                "identity",
                "Identity",
                "Identity file exists but cannot be loaded.",
                Some("home.init"),
            )
            .detail(format!("{error:#}"))
            .related_command("home.init"),
            Err(_) => NetworkHealthRow::needs_attention(
                "identity",
                "Identity",
                "Create a local peer identity.",
                Some("home.init"),
            )
            .related_command("home.init"),
        }
        .related_view("profile.summary");

        let certificate_status = match self.load_certificate() {
            Ok(certificate) => NetworkHealthRow::working(
                "certificate",
                "Certificate",
                "Persistent QUIC certificate is available.",
            )
            .detail(format!("fingerprint: {}", certificate.fingerprint)),
            Err(error) if self.path("quic-cert.json").exists() => NetworkHealthRow::broken(
                "certificate",
                "Certificate",
                "Certificate file exists but cannot be loaded.",
                Some("home.init"),
            )
            .detail(format!("{error:#}"))
            .related_command("home.init"),
            Err(_) => NetworkHealthRow::needs_attention(
                "certificate",
                "Certificate",
                "Create persistent QUIC certificate material.",
                Some("home.init"),
            )
            .related_command("home.init"),
        }
        .related_view("runtime.status");

        let ipv6_status = match local_ipv6_socket_available() {
            Ok(()) => {
                NetworkHealthRow::working("ipv6", "IPv6", "This machine can open an IPv6 socket.")
            }
            Err(error) => NetworkHealthRow::broken(
                "ipv6",
                "IPv6",
                "This machine could not open an IPv6 socket.",
                None,
            )
            .detail(format!("{error:#}")),
        }
        .related_view("network.health");

        let service_status = match online {
            Some(online) => NetworkHealthRow::working(
                "service",
                "Service",
                format!("Resident service is online at {}.", online.endpoint.addr),
            )
            .related_command("runtime.goOffline"),
            None => NetworkHealthRow::needs_attention(
                "service",
                "Service",
                "Go online to accept peer diagnostics and sync requests.",
                Some("runtime.goOnline"),
            )
            .related_command("runtime.goOnline"),
        }
        .related_view("runtime.status");

        let bind_status = match online {
            Some(online) if online.local_report.listen_addr.is_ipv6() => NetworkHealthRow::working(
                "bind",
                "Bind",
                format!("Listening on {}.", online.local_report.listen_addr),
            )
            .related_command("runtime.goOffline"),
            Some(online) => NetworkHealthRow::broken(
                "bind",
                "Bind",
                format!("Listener is not IPv6: {}.", online.local_report.listen_addr),
                Some("runtime.goOffline"),
            )
            .related_command("runtime.goOffline"),
            None => NetworkHealthRow::unknown(
                "bind",
                "Bind",
                "Binding has not been tested in this session.",
                Some("runtime.goOnline"),
            )
            .related_command("runtime.goOnline"),
        }
        .related_view("runtime.status");

        let advertise_status = match online {
            Some(online) => advertised_address_row(&online.local_report),
            None => NetworkHealthRow::unknown(
                "advertise",
                "Advertise",
                "No advertised address until the service is online.",
                Some("runtime.goOnline"),
            )
            .related_command("runtime.goOnline"),
        }
        .related_view("runtime.status");

        let invite_status = match online {
            Some(online) => match online.invite_view(None, None) {
                Ok(invite) => NetworkHealthRow::new(
                    "invite",
                    "Invite",
                    NetworkHealthStatus::Working,
                    "A peer record can be generated from the current service.",
                    Some("invite.copy"),
                )
                .detail(format!(
                    "advertised address: {}",
                    invite.peer_record.endpoint.addr
                ))
                .related_command("invite.copy"),
                Err(error) => NetworkHealthRow::broken(
                    "invite",
                    "Invite",
                    "Current service could not produce an invite.",
                    Some("runtime.goOnline"),
                )
                .detail(format!("{error:#}"))
                .related_command("runtime.goOnline"),
            },
            None => NetworkHealthRow::unknown(
                "invite",
                "Invite",
                "Go online before copying an invite.",
                Some("runtime.goOnline"),
            )
            .related_command("runtime.goOnline"),
        }
        .related_view("invite.exchange");

        let peers = self.known_peers()?;
        let peer_status = if peers.is_empty() {
            NetworkHealthRow::needs_attention(
                "peers",
                "Peers",
                "Import a peer record before peer diagnostics or sync can run.",
                Some("peer.import"),
            )
            .related_command("peer.import")
        } else {
            NetworkHealthRow::working(
                "peers",
                "Peers",
                format!("{} known peer record(s).", peers.len()),
            )
            .related_command("peer.import")
        }
        .related_view("peer.list");

        let reachability_status = if peers.is_empty() {
            NetworkHealthRow::unknown(
                "reachability",
                "Reachability",
                "No peer is available to verify incoming reachability.",
                Some("peer.import"),
            )
            .related_command("peer.import")
        } else {
            NetworkHealthRow::needs_attention(
                "reachability",
                "Reachability",
                "Run a peer-assisted diagnostic against a known peer.",
                Some("peer.diagnose"),
            )
            .detail("A real incoming check requires another peer to connect back.")
            .related_command("peer.diagnose")
        }
        .related_view("network.health")
        .related_view("service.activity");

        let sync_status = if peers.is_empty() {
            NetworkHealthRow::unknown(
                "sync",
                "Sync",
                "No peer is available to test room sync.",
                Some("peer.import"),
            )
            .related_command("peer.import")
        } else {
            NetworkHealthRow::needs_attention(
                "sync",
                "Sync",
                "Run sync with a known peer to verify durable room exchange.",
                Some("peer.sync"),
            )
            .related_command("peer.sync")
        }
        .related_view("network.health")
        .related_view("service.activity");

        Ok(NetworkHealthView {
            rows: vec![
                home_status,
                identity_status,
                certificate_status,
                ipv6_status,
                service_status,
                bind_status,
                advertise_status,
                invite_status,
                peer_status,
                reachability_status,
                sync_status,
            ],
        })
    }

    pub fn send_message(&self, text: &str, room: Option<&str>) -> Result<EventV1> {
        self.send_message_with_metadata(text, room, Vec::new(), None)
    }

    pub fn send_message_with_metadata(
        &self,
        text: &str,
        room: Option<&str>,
        mentions: Vec<String>,
        thread_root_event_id: Option<String>,
    ) -> Result<EventV1> {
        self.create_room_event(
            room,
            "MSG_POST",
            serde_json::json!({
                "text": text,
                "mentions": mentions,
                "thread_root_event_id": thread_root_event_id,
            }),
        )
    }

    pub fn edit_message(&self, request: &EditMessageRequest) -> Result<EventV1> {
        self.create_room_event(
            request.room.as_deref(),
            "MSG_EDIT",
            serde_json::json!({
                "target_event_id": request.target_event_id,
                "text": request.text,
                "mentions": request.mentions,
            }),
        )
    }

    pub fn redact_message(&self, request: &MessageTargetRequest) -> Result<EventV1> {
        self.create_room_event(
            request.room.as_deref(),
            "MSG_REDACT",
            serde_json::json!({ "target_event_id": request.target_event_id }),
        )
    }

    pub fn set_reaction(&self, request: &ReactionRequest, add: bool) -> Result<EventV1> {
        self.create_room_event(
            request.room.as_deref(),
            if add {
                "REACTION_ADD"
            } else {
                "REACTION_REMOVE"
            },
            serde_json::json!({
                "target_event_id": request.target_event_id,
                "emoji": request.emoji,
            }),
        )
    }

    pub fn set_pin(&self, request: &MessageTargetRequest, add: bool) -> Result<EventV1> {
        self.create_room_event(
            request.room.as_deref(),
            if add { "PIN_ADD" } else { "PIN_REMOVE" },
            serde_json::json!({ "target_event_id": request.target_event_id }),
        )
    }

    pub fn add_attachment(&self, request: &AttachmentRequest) -> Result<EventV1> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&request.data_b64)
            .context("decode attachment")?;
        let sha256 = format!(
            "sha256:{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(&bytes))
        );
        self.create_room_event(
            request.room.as_deref(),
            "ATTACHMENT_ADD",
            serde_json::json!({
                "filename": request.filename,
                "mime": request.mime,
                "sha256": sha256,
                "data_b64": request.data_b64,
            }),
        )
    }

    pub fn update_profile(&self, request: &ProfileUpdateRequest) -> Result<EventV1> {
        self.create_room_event(
            None,
            "PROFILE_UPDATE",
            serde_json::json!({
                "display_name": request.display_name,
                "about": request.about,
            }),
        )
    }

    pub fn create_channel(&self, request: &CreateChannelRequest) -> Result<EventV1> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let slug: String = request
            .name
            .chars()
            .flat_map(char::to_lowercase)
            .map(|character| {
                if character.is_alphanumeric() {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .chars()
            .take(48)
            .collect();
        if slug.is_empty() {
            anyhow::bail!("channel name must contain a letter or number");
        }
        let suffix =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8; 6]>());
        let room_id = format!("{}:channel:{slug}-{suffix}", config.space.space_id);
        let mut private_members: std::collections::BTreeSet<String> =
            request.private_members.iter().cloned().collect();
        let private = !private_members.is_empty();
        let (key_epoch, key_packages, room_key) = if private {
            private_members.insert(identity.peer_id.clone());
            let governance = store.room_events(&config.space.governance_room_id)?;
            let state = derive_governance_state(&governance, &config.room_context(), now_ms());
            if private_members
                .iter()
                .any(|peer_id| !state.members.contains(peer_id))
            {
                anyhow::bail!("private channel members must already belong to the space");
            }
            let mut room_key = [0_u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut room_key);
            let packages = create_room_key_packages(
                &room_id,
                1,
                &room_key,
                &private_members,
                &state.member_encryption_keys,
            )?;
            (1_u64, packages, Some(room_key))
        } else {
            (0_u64, Vec::new(), None)
        };
        let event = create_event(
            &identity,
            create_delegation(
                &identity,
                now_ms() - 60_000,
                now_ms() + 30 * 24 * 60 * 60_000,
                vec!["room:governance".to_string()],
            )?,
            &config.space.governance_room_id,
            now_ms(),
            "CHANNEL_CREATE",
            store.room_heads(&config.space.governance_room_id)?,
            serde_json::json!({
                "room_id": room_id.clone(),
                "name": request.name,
                "topic": request.topic,
                "visibility": if private { "private" } else { "public" },
                "private_members": private_members,
                "key_epoch": key_epoch,
                "key_packages": key_packages,
            }),
        )?;
        self.accept_local_event(&store, &config, &event)
            .context("create channel")?;
        if let Some(room_key) = room_key {
            self.store_room_key(&room_id, key_epoch, &room_key)?;
        }
        Ok(event)
    }

    pub fn rotate_channel_key(&self, request: &RotateChannelKeyRequest) -> Result<EventV1> {
        let config = self.load_config()?;
        let store = self.open_store()?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let state = derive_governance_state(&governance, &config.room_context(), now_ms());
        let channel = state
            .channels
            .get(&request.room_id)
            .ok_or_else(|| anyhow::anyhow!("channel does not exist"))?;
        if channel.visibility != ChannelVisibility::Private {
            anyhow::bail!("only private channels have rotatable keys");
        }
        let epoch = channel.key_epoch.saturating_add(1);
        let mut room_key = [0_u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut room_key);
        let packages = create_room_key_packages(
            &request.room_id,
            epoch,
            &room_key,
            &channel.private_members,
            &state.member_encryption_keys,
        )?;
        let event = self.create_governance_event(
            "CHANNEL_KEY_ROTATE",
            serde_json::json!({
                "room_id": request.room_id,
                "key_epoch": epoch,
                "key_packages": packages,
            }),
        )?;
        self.store_room_key(&request.room_id, epoch, &room_key)?;
        Ok(event)
    }

    pub fn create_role(&self, request: &CreateRoleRequest) -> Result<EventV1> {
        let slug: String = request
            .name
            .chars()
            .flat_map(char::to_lowercase)
            .map(|character| {
                if character.is_alphanumeric() {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .chars()
            .take(48)
            .collect();
        if slug.is_empty() {
            anyhow::bail!("role name must contain a letter or number");
        }
        let suffix =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8; 6]>());
        self.create_governance_event(
            "ROLE_CREATE",
            serde_json::json!({
                "role_id": format!("role:{slug}-{suffix}"),
                "name": request.name,
                "permissions": request.permissions,
            }),
        )
    }

    pub fn assign_role(&self, request: &AssignRoleRequest, grant: bool) -> Result<EventV1> {
        self.create_governance_event(
            if grant { "ROLE_GRANT" } else { "ROLE_REVOKE" },
            serde_json::json!({
                "peer_id": request.peer_id,
                "role_id": request.role_id,
            }),
        )
    }

    pub fn ban_member(&self, request: &BanMemberRequest, ban: bool) -> Result<EventV1> {
        self.create_governance_event(
            if ban { "MEMBER_BAN" } else { "MEMBER_UNBAN" },
            serde_json::json!({
                "peer_id": request.peer_id,
                "reason": request.reason,
            }),
        )
    }

    pub fn read_messages(&self, room: Option<&str>) -> Result<Vec<MessageView>> {
        let config = self.load_config()?;
        let room = room.unwrap_or(&config.space.default_room_id);
        Ok(project_messages(self.decrypted_room_events(room)?))
    }

    pub fn channels(&self, selected_room: Option<&str>) -> Result<Vec<ChannelView>> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let state = derive_governance_state(&governance, &config.room_context(), now_ms());
        let read_state = self.read_state()?;
        let mut unread_counts = BTreeMap::new();
        for channel in state.channels.values() {
            if voxelle_core::channel_allows_peer(channel, &identity.peer_id) {
                unread_counts.insert(
                    channel.room_id.clone(),
                    unread_count(
                        self.decrypted_room_events(&channel.room_id)?,
                        read_state.last_read_event_ids.get(&channel.room_id),
                        &identity.peer_id,
                    ),
                );
            }
        }
        let mut channels: Vec<ChannelView> = state
            .channels
            .values()
            .filter(|channel| voxelle_core::channel_allows_peer(channel, &identity.peer_id))
            .map(|channel| ChannelView {
                room_id: channel.room_id.clone(),
                name: channel.name.clone(),
                topic: channel.topic.clone(),
                visibility: match channel.visibility {
                    ChannelVisibility::Public => "public",
                    ChannelVisibility::Private => "private",
                }
                .to_string(),
                selected: selected_room.unwrap_or(&config.space.default_room_id) == channel.room_id,
                unread_count: unread_counts.get(&channel.room_id).copied().unwrap_or(0),
            })
            .collect();
        channels.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.room_id.cmp(&right.room_id))
        });
        Ok(channels)
    }

    pub fn mark_read(&self, room: Option<&str>) -> Result<()> {
        let config = self.load_config()?;
        let room_id = room.unwrap_or(&config.space.default_room_id);
        if !self
            .channels(Some(room_id))?
            .iter()
            .any(|channel| channel.room_id == room_id)
        {
            anyhow::bail!("channel is unknown or inaccessible");
        }
        let mut read_state = self.read_state()?;
        let mut events = self.open_store()?.room_events(room_id)?;
        events.sort_by(|left, right| {
            left.created_ms
                .cmp(&right.created_ms)
                .then(left.event_id.cmp(&right.event_id))
        });
        if let Some(event) = events.last() {
            read_state
                .last_read_event_ids
                .insert(room_id.to_string(), event.event_id.clone());
        } else {
            read_state.last_read_event_ids.remove(room_id);
        }
        self.put_local_state(READ_STATE, &read_state)
    }

    pub fn notifications(&self) -> Result<Vec<NotificationView>> {
        let identity = self.load_identity()?;
        let read_state = self.read_state()?;
        let mut notifications = Vec::new();
        for channel in self.channels(None)? {
            let mut events = self.decrypted_room_events(&channel.room_id)?;
            events.sort_by(|left, right| {
                left.created_ms
                    .cmp(&right.created_ms)
                    .then(left.event_id.cmp(&right.event_id))
            });
            let start = unread_start(
                &events,
                read_state.last_read_event_ids.get(&channel.room_id),
            );
            for event in events.into_iter().skip(start) {
                if event.author_peer_id == identity.peer_id || event.kind != "MSG_POST" {
                    continue;
                }
                let mentioned = event
                    .body
                    .get("mentions")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|mentions| {
                        mentions
                            .iter()
                            .any(|mention| mention.as_str() == Some(&identity.peer_id))
                    });
                if mentioned {
                    notifications.push(NotificationView {
                        event_id: event.event_id,
                        room_id: event.room_id,
                        author_peer_id: event.author_peer_id,
                        summary: event
                            .body
                            .get("text")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Mentioned you")
                            .to_string(),
                        kind: "mention".to_string(),
                        created_ms: event.created_ms,
                    });
                }
            }
        }
        notifications.sort_by_key(|item| std::cmp::Reverse(item.created_ms));
        Ok(notifications)
    }

    pub fn roles(&self) -> Result<Vec<RoleView>> {
        let config = self.load_config()?;
        let store = self.open_store()?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let state = derive_governance_state(&governance, &config.room_context(), now_ms());
        let mut roles: Vec<RoleView> = state
            .roles
            .values()
            .map(|role| RoleView {
                role_id: role.role_id.clone(),
                name: role.name.clone(),
                permissions: role.permissions.iter().cloned().collect(),
                member_count: if role.role_id == "role:everyone" {
                    state.members.len()
                } else {
                    state
                        .member_roles
                        .values()
                        .filter(|roles| roles.contains(&role.role_id))
                        .count()
                },
            })
            .collect();
        roles.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(roles)
    }

    pub fn profiles(&self) -> Result<Vec<ProfileView>> {
        let config = self.load_config()?;
        let store = self.open_store()?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let state = derive_governance_state(&governance, &config.room_context(), now_ms());
        let mut profiles: BTreeMap<String, ProfileView> = state
            .members
            .iter()
            .map(|peer_id| {
                (
                    peer_id.clone(),
                    ProfileView {
                        peer_id: peer_id.clone(),
                        display_name: short_peer_label(peer_id),
                        about: String::new(),
                    },
                )
            })
            .collect();
        let mut updates = Vec::new();
        for channel in state.channels.values() {
            updates.extend(
                store
                    .room_events(&channel.room_id)?
                    .into_iter()
                    .filter(|event| {
                        event.kind == "PROFILE_UPDATE"
                            && state.members.contains(&event.author_peer_id)
                    }),
            );
        }
        updates.sort_by(|left, right| {
            left.created_ms
                .cmp(&right.created_ms)
                .then(left.event_id.cmp(&right.event_id))
        });
        for event in updates {
            profiles.insert(
                event.author_peer_id.clone(),
                ProfileView {
                    peer_id: event.author_peer_id,
                    display_name: event
                        .body
                        .get("display_name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Member")
                        .to_string(),
                    about: event
                        .body
                        .get("about")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                },
            );
        }
        Ok(profiles.into_values().collect())
    }

    pub fn search_messages(
        &self,
        request: &SearchMessagesRequest,
    ) -> Result<Vec<SearchResultView>> {
        let query = request.query.trim().to_lowercase();
        if query.is_empty() {
            anyhow::bail!("search query is empty");
        }
        let terms: Vec<&str> = query.split_whitespace().collect();
        let rooms: Vec<String> = if let Some(room) = &request.room {
            vec![room.clone()]
        } else {
            self.channels(None)?
                .into_iter()
                .map(|channel| channel.room_id)
                .collect()
        };
        let mut results = Vec::new();
        for room_id in rooms {
            for message in self.read_messages(Some(&room_id))? {
                let haystack = format!(
                    "{} {} {}",
                    message.text,
                    message.author_peer_id,
                    message
                        .attachments
                        .iter()
                        .map(|attachment| attachment.filename.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
                .to_lowercase();
                if terms.iter().all(|term| haystack.contains(term)) {
                    results.push(SearchResultView {
                        room_id: room_id.clone(),
                        message,
                    });
                }
            }
        }
        results.sort_by_key(|item| std::cmp::Reverse(item.message.created_ms));
        results.truncate(request.limit.unwrap_or(50).clamp(1, 100));
        Ok(results)
    }

    pub fn call_view(&self, room_id: &str) -> Result<CallView> {
        let call_id = room_call_id(room_id);
        let mut events = self.decrypted_room_events(room_id)?;
        events.sort_by(|left, right| {
            left.created_ms
                .cmp(&right.created_ms)
                .then(left.event_id.cmp(&right.event_id))
        });
        let now = now_ms();
        let mut last_seen = BTreeMap::new();
        for event in &events {
            if event
                .body
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                != Some(call_id.as_str())
            {
                continue;
            }
            if matches!(event.kind.as_str(), "CALL_JOIN" | "CALL_HEARTBEAT") {
                last_seen.insert(event.author_peer_id.clone(), event.created_ms);
            } else if event.kind == "CALL_LEAVE" {
                last_seen.remove(&event.author_peer_id);
            }
        }
        let participants: Vec<String> = last_seen
            .into_iter()
            .filter(|(_, seen_ms)| now.saturating_sub(*seen_ms) <= CALL_LIVENESS_MS)
            .map(|(peer_id, _)| peer_id)
            .take(4)
            .collect();
        let signals = if participants.is_empty() {
            Vec::new()
        } else {
            events
                .into_iter()
                .filter(|event| {
                    event.kind.starts_with("CALL_")
                        && now.saturating_sub(event.created_ms) <= CALL_LIVENESS_MS
                        && event
                            .body
                            .get("call_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(call_id.as_str())
                })
                .map(|event| CallSignalView {
                    event_id: event.event_id,
                    kind: event.kind,
                    call_id: call_id.clone(),
                    author_peer_id: event.author_peer_id,
                    target_peer_id: event
                        .body
                        .get("target_peer_id")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned),
                    video: event.body.get("video").and_then(serde_json::Value::as_bool),
                    sdp: event
                        .body
                        .get("sdp")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned),
                    candidate: event
                        .body
                        .get("candidate")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned),
                    created_ms: event.created_ms,
                })
                .rev()
                .take(MAX_PROJECTED_CALL_SIGNALS)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        };
        Ok(CallView {
            call_id,
            participants,
            signals,
        })
    }

    pub fn join_call(&self, request: &CallJoinRequest) -> Result<EventV1> {
        let config = self.load_config()?;
        let room_id = request
            .room
            .as_deref()
            .unwrap_or(&config.space.default_room_id);
        let identity = self.load_identity()?;
        let call = self.call_view(room_id)?;
        if call.participants.contains(&identity.peer_id) {
            anyhow::bail!("already joined to the room call");
        }
        if call.participants.len() >= 4 {
            anyhow::bail!("room calls are limited to four peers");
        }
        self.create_room_event(
            Some(room_id),
            "CALL_JOIN",
            serde_json::json!({ "call_id": call.call_id, "video": request.video }),
        )
    }

    pub fn signal_call(&self, request: &CallSignalRequest) -> Result<EventV1> {
        let kind = match request.signal_type.as_str() {
            "offer" => "CALL_OFFER",
            "answer" => "CALL_ANSWER",
            "ice" => "CALL_ICE",
            _ => anyhow::bail!("unknown call signal type"),
        };
        self.create_room_event(
            request.room.as_deref(),
            kind,
            serde_json::json!({
                "call_id": request.call_id,
                "target_peer_id": request.target_peer_id,
                "sdp": request.sdp,
                "candidate": request.candidate,
            }),
        )
    }

    pub fn heartbeat_call(&self, request: &CallLeaveRequest) -> Result<EventV1> {
        self.create_room_event(
            request.room.as_deref(),
            "CALL_HEARTBEAT",
            serde_json::json!({ "call_id": request.call_id }),
        )
    }

    pub fn leave_call(&self, request: &CallLeaveRequest) -> Result<EventV1> {
        self.create_room_event(
            request.room.as_deref(),
            "CALL_LEAVE",
            serde_json::json!({ "call_id": request.call_id }),
        )
    }

    fn create_room_event(
        &self,
        room: Option<&str>,
        kind: &str,
        body: serde_json::Value,
    ) -> Result<EventV1> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let room = room.unwrap_or(&config.space.default_room_id);
        let created_ms = now_ms();
        let parents = store.room_heads(room)?;
        let delegation_scope = if kind.starts_with("CALL_") {
            "room:call"
        } else {
            "room:post"
        };
        let semantic_event = create_event(
            &identity,
            create_delegation(
                &identity,
                created_ms - 60_000,
                created_ms + 30 * 24 * 60 * 60_000,
                vec![delegation_scope.to_string()],
            )?,
            room,
            created_ms,
            kind,
            parents.clone(),
            body.clone(),
        )?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let state = derive_governance_state(&governance, &config.room_context(), created_ms);
        let channel = state
            .channels
            .get(room)
            .ok_or_else(|| anyhow::anyhow!("room is not a current channel"))?;
        let event = if channel.visibility == ChannelVisibility::Private {
            self.import_private_room_keys()?;
            let mut accepted = governance;
            accepted.extend(self.decrypted_room_events(room)?);
            validate_room_event_semantics(
                &semantic_event,
                &accepted,
                &config.room_context(),
                created_ms,
            )
            .map_err(|error| anyhow::anyhow!("private event semantics rejected: {error:?}"))?;
            let key = self.room_key(room, channel.key_epoch)?;
            let cipher = XChaCha20Poly1305::new((&key).into());
            let mut nonce = [0_u8; 24];
            rand::rngs::OsRng.fill_bytes(&mut nonce);
            let plaintext = serde_json::to_vec(&PrivateEventPlaintext {
                kind: kind.to_string(),
                body,
            })?;
            let aad = private_event_aad(room, channel.key_epoch, &identity.peer_id);
            let ciphertext = cipher
                .encrypt(
                    XNonce::from_slice(&nonce),
                    chacha20poly1305::aead::Payload {
                        msg: &plaintext,
                        aad: aad.as_bytes(),
                    },
                )
                .map_err(|_| anyhow::anyhow!("encrypt private room event"))?;
            create_event(
                &identity,
                create_delegation(
                    &identity,
                    created_ms - 60_000,
                    created_ms + 30 * 24 * 60 * 60_000,
                    vec![delegation_scope.to_string()],
                )?,
                room,
                created_ms,
                "ROOM_ENCRYPTED",
                parents,
                serde_json::json!({
                    "key_epoch": channel.key_epoch,
                    "nonce_b64": base64::engine::general_purpose::STANDARD.encode(nonce),
                    "ciphertext_b64": base64::engine::general_purpose::STANDARD.encode(ciphertext),
                }),
            )?
        } else {
            semantic_event
        };
        self.accept_local_event(&store, &config, &event)
            .with_context(|| format!("accept local {kind}"))?;
        Ok(event)
    }

    fn decrypted_room_events(&self, room_id: &str) -> Result<Vec<EventV1>> {
        self.import_private_room_keys()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let state = derive_governance_state(&governance, &config.room_context(), now_ms());
        let private = state
            .channels
            .get(room_id)
            .is_some_and(|channel| channel.visibility == ChannelVisibility::Private);
        let mut raw_events = store.room_events(room_id)?;
        raw_events.sort_by(|left, right| {
            left.created_ms
                .cmp(&right.created_ms)
                .then(left.event_id.cmp(&right.event_id))
        });
        if !private {
            return Ok(raw_events);
        }
        let mut accepted = governance;
        let mut decrypted = Vec::new();
        for raw in raw_events {
            if raw.kind != "ROOM_ENCRYPTED" {
                continue;
            }
            let Some(epoch) = raw
                .body
                .get("key_epoch")
                .and_then(serde_json::Value::as_u64)
            else {
                continue;
            };
            let Ok(key) = self.room_key(room_id, epoch) else {
                continue;
            };
            let Some(nonce_b64) = raw
                .body
                .get("nonce_b64")
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let Some(ciphertext_b64) = raw
                .body
                .get("ciphertext_b64")
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let Ok(nonce) = base64::engine::general_purpose::STANDARD.decode(nonce_b64) else {
                continue;
            };
            let Ok(nonce) = <[u8; 24]>::try_from(nonce) else {
                continue;
            };
            let Ok(ciphertext) = base64::engine::general_purpose::STANDARD.decode(ciphertext_b64)
            else {
                continue;
            };
            let cipher = XChaCha20Poly1305::new((&key).into());
            let aad = private_event_aad(room_id, epoch, &raw.author_peer_id);
            let Ok(plaintext) = cipher.decrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &ciphertext,
                    aad: aad.as_bytes(),
                },
            ) else {
                continue;
            };
            let Ok(inner) = serde_json::from_slice::<PrivateEventPlaintext>(&plaintext) else {
                continue;
            };
            let mut event = raw;
            event.kind = inner.kind;
            event.body = inner.body;
            if validate_room_event_semantics(
                &event,
                &accepted,
                &config.room_context(),
                event.created_ms,
            )
            .is_ok()
            {
                accepted.push(event.clone());
                decrypted.push(event);
            }
        }
        Ok(decrypted)
    }

    fn create_governance_event(&self, kind: &str, body: serde_json::Value) -> Result<EventV1> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let event = create_event(
            &identity,
            create_delegation(
                &identity,
                now_ms() - 60_000,
                now_ms() + 30 * 24 * 60 * 60_000,
                vec!["room:governance".to_string()],
            )?,
            &config.space.governance_room_id,
            now_ms(),
            kind,
            store.room_heads(&config.space.governance_room_id)?,
            body,
        )?;
        self.accept_local_event(&store, &config, &event)
            .with_context(|| format!("accept local {kind}"))?;
        Ok(event)
    }

    fn accept_local_event(
        &self,
        store: &Store,
        config: &HomeConfig,
        event: &EventV1,
    ) -> Result<()> {
        let mut accepted_events = store.room_events(&config.space.governance_room_id)?;
        if event.room_id != config.space.governance_room_id {
            accepted_events.extend(store.room_events(&event.room_id)?);
        }
        let accepted = accept_event(event, &accepted_events, &config.room_context(), now_ms())
            .map_err(|error| anyhow::anyhow!("event rejected: {error:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(())
    }

    pub fn import_peer_record(&self, record: PeerRecord) -> Result<()> {
        record.validate()?;
        let mut peers = self.known_peers()?;
        if let Some(existing) = peers.iter_mut().find(|peer| peer.same_peer(&record)) {
            *existing = record;
        } else {
            if peers.len() >= MAX_KNOWN_PEERS {
                anyhow::bail!("known peer records are limited to {MAX_KNOWN_PEERS}");
            }
            peers.push(record);
        }
        peers.sort_by(|a, b| {
            a.label
                .cmp(&b.label)
                .then_with(|| a.endpoint.peer_id.cmp(&b.endpoint.peer_id))
                .then_with(|| a.endpoint.device_id.cmp(&b.endpoint.device_id))
        });
        self.put_local_state(KNOWN_PEERS_STATE, &KnownPeersFile { v: 1, peers })
    }

    pub fn known_peers(&self) -> Result<Vec<PeerRecord>> {
        let Some(file): Option<KnownPeersFile> = self.local_state(KNOWN_PEERS_STATE)? else {
            return Ok(Vec::new());
        };
        if file.v != 1 {
            anyhow::bail!("unsupported known peers version {}", file.v);
        }
        for record in &file.peers {
            record.validate()?;
        }
        Ok(file.peers)
    }

    fn read_state(&self) -> Result<ReadStateFile> {
        let Some(state): Option<ReadStateFile> = self.local_state(READ_STATE)? else {
            return Ok(ReadStateFile {
                v: 1,
                last_read_event_ids: BTreeMap::new(),
            });
        };
        if state.v != 1 {
            anyhow::bail!("unsupported read state version {}", state.v);
        }
        Ok(state)
    }

    fn room_keys(&self) -> Result<RoomKeysV1> {
        let Some(file): Option<EncryptedRoomKeysFile> = self.local_state(ROOM_KEYS_STATE)? else {
            return Ok(RoomKeysV1::default());
        };
        let identity = self.load_identity()?;
        if file.v != 1 {
            anyhow::bail!("unsupported encrypted room keys version {}", file.v);
        }
        let nonce: [u8; 24] = base64::engine::general_purpose::STANDARD
            .decode(&file.nonce_b64)
            .context("decode room keys nonce")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("room keys nonce must be 24 bytes"))?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&file.ciphertext_b64)
            .context("decode room keys ciphertext")?;
        let key = identity_vault_key(&self.path("identity.json"), false)?;
        let cipher = XChaCha20Poly1305::new((&key).into());
        let aad = format!("voxelle/room-keys/v1\n{}", identity.peer_id);
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("room keys authentication failed"))?;
        let keys: RoomKeysV1 =
            serde_json::from_slice(&plaintext).context("parse decrypted room keys")?;
        if keys.v != 1 {
            anyhow::bail!("unsupported room keys version {}", keys.v);
        }
        Ok(keys)
    }

    fn write_room_keys(&self, keys: &RoomKeysV1) -> Result<()> {
        if keys.v != 1 {
            anyhow::bail!("unsupported room keys version {}", keys.v);
        }
        let identity = self.load_identity()?;
        let key = identity_vault_key(&self.path("identity.json"), false)?;
        let cipher = XChaCha20Poly1305::new((&key).into());
        let mut nonce = [0_u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let plaintext = serde_json::to_vec(keys).context("serialize room keys")?;
        let aad = format!("voxelle/room-keys/v1\n{}", identity.peer_id);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("encrypt room keys"))?;
        self.put_local_state(
            ROOM_KEYS_STATE,
            &EncryptedRoomKeysFile {
                v: 1,
                nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce),
                ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
            },
        )
    }

    fn store_room_key(&self, room_id: &str, epoch: u64, key: &[u8; 32]) -> Result<()> {
        let mut keys = self.room_keys()?;
        keys.keys
            .entry(room_id.to_string())
            .or_default()
            .insert(epoch, base64::engine::general_purpose::STANDARD.encode(key));
        self.write_room_keys(&keys)
    }

    fn room_key(&self, room_id: &str, epoch: u64) -> Result<[u8; 32]> {
        let keys = self.room_keys()?;
        let encoded = keys
            .keys
            .get(room_id)
            .and_then(|epochs| epochs.get(&epoch))
            .ok_or_else(|| anyhow::anyhow!("private room key epoch {epoch} is unavailable"))?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("decode private room key")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("private room key must be 32 bytes"))
    }

    fn import_private_room_keys(&self) -> Result<()> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let governance = self
            .open_store()?
            .room_events(&config.space.governance_room_id)?;
        let mut keys = self.room_keys()?;
        let mut changed = false;
        for event in governance {
            if !matches!(event.kind.as_str(), "CHANNEL_CREATE" | "CHANNEL_KEY_ROTATE") {
                continue;
            }
            let Some(room_id) = event
                .body
                .get("room_id")
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let Some(epoch) = event
                .body
                .get("key_epoch")
                .and_then(serde_json::Value::as_u64)
            else {
                continue;
            };
            if keys
                .keys
                .get(room_id)
                .is_some_and(|epochs| epochs.contains_key(&epoch))
            {
                continue;
            }
            let package = event
                .body
                .get("key_packages")
                .and_then(serde_json::Value::as_array)
                .and_then(|packages| {
                    packages.iter().find(|package| {
                        package.get("peer_id").and_then(serde_json::Value::as_str)
                            == Some(identity.peer_id.as_str())
                    })
                });
            let Some(package) = package else { continue };
            let key = unwrap_room_key_package(&identity, room_id, epoch, package)?;
            keys.keys
                .entry(room_id.to_string())
                .or_default()
                .insert(epoch, base64::engine::general_purpose::STANDARD.encode(key));
            changed = true;
        }
        if changed {
            self.write_room_keys(&keys)?;
        }
        Ok(())
    }

    pub fn recovery_kit(&self) -> Result<RecoveryKitV1> {
        let identity = self.load_identity()?;
        let card = identity.recovery_card();
        let config = self.load_config()?;
        let governance_events = self
            .open_store()?
            .room_events(&config.space.governance_room_id)?;
        let payload = RecoveryPayloadV1 {
            v: 1,
            identity_proof: identity.proof.clone(),
            space: config.space,
            governance_events,
            known_peers: self.known_peers()?,
            ui_preferences: self.ui_preferences()?,
            read_state: self.read_state()?,
            room_keys: self.room_keys()?,
        };
        let capsule = encrypt_recovery_capsule(&card, &identity.peer_id, &payload)?;
        Ok(RecoveryKitV1 {
            v: 1,
            card,
            capsule,
        })
    }

    pub fn write_recovery_kit(&self, path: impl AsRef<Path>) -> Result<()> {
        let kit = self.recovery_kit()?;
        write_secret_json(path.as_ref(), &kit)
    }

    pub async fn recover_from_kit(
        &self,
        kit: &RecoveryKitV1,
        max_events_per_peer: usize,
    ) -> Result<RecoveryReport> {
        if kit.v != 1 {
            anyhow::bail!("unsupported recovery kit version {}", kit.v);
        }
        if self.path("identity.json").exists() || self.local_state_exists(HOME_SELECTION_STATE)? {
            anyhow::bail!("recovery requires a fresh Voxelle home");
        }
        if max_events_per_peer == 0 {
            anyhow::bail!("max_events_per_peer must be positive");
        }

        let payload = decrypt_recovery_capsule(&kit.card, &kit.capsule)?;
        if payload.v != 1 {
            anyhow::bail!("unsupported recovery payload version {}", payload.v);
        }
        if payload.identity_proof.genesis != kit.card.genesis {
            anyhow::bail!("recovery capsule identity does not match recovery card");
        }
        let identity = PeerIdentity::recover(&kit.card, &payload.identity_proof, now_ms())?;
        validate_space_at(&payload.space, now_ms())?;
        let config = HomeConfig {
            space: payload.space.clone(),
        };
        for peer in &payload.known_peers {
            peer.validate()?;
        }
        validate_ui_preferences(&payload.ui_preferences)?;
        if payload.read_state.v != 1 {
            anyhow::bail!(
                "unsupported recovered read state version {}",
                payload.read_state.v
            );
        }
        if payload.room_keys.v != 1 {
            anyhow::bail!(
                "unsupported recovered room keys version {}",
                payload.room_keys.v
            );
        }

        ensure_private_dir(&self.root)?;
        write_identity_vault(&self.path("identity.json"), &identity)?;
        self.put_local_state(
            KNOWN_PEERS_STATE,
            &KnownPeersFile {
                v: 1,
                peers: payload.known_peers.clone(),
            },
        )?;
        self.write_ui_preferences(&payload.ui_preferences)?;
        self.put_local_state(READ_STATE, &payload.read_state)?;
        self.write_room_keys(&payload.room_keys)?;
        self.load_or_create_certificate()?;
        let store = self.open_store()?;
        self.ensure_space_genesis(&store, &config)?;
        self.put_local_state(
            HOME_SELECTION_STATE,
            &HomeSelectionV1 {
                v: 1,
                space_genesis_event_id: config.space.genesis.event_id.clone(),
            },
        )?;
        let event_by_id: BTreeMap<String, EventV1> = payload
            .governance_events
            .iter()
            .cloned()
            .map(|event| (event.event_id.clone(), event))
            .collect();
        let mut events_recovered = 0;
        for event_id in topo_sort_deterministic(&payload.governance_events) {
            if store.has_event(&event_id)? {
                continue;
            }
            let event = event_by_id
                .get(&event_id)
                .ok_or_else(|| anyhow::anyhow!("recovery governance event disappeared"))?;
            let governance = store.room_events(&config.space.governance_room_id)?;
            let accepted = accept_event(event, &governance, &config.room_context(), now_ms())
                .map_err(|error| {
                    anyhow::anyhow!("recovery governance event rejected: {error:?}")
                })?;
            store.insert_accepted_event(accepted, now_ms())?;
            events_recovered += 1;
        }

        let mut peers_reached = 0;
        let mut events_pushed = 0;
        let mut peer_errors = Vec::new();
        for peer in &payload.known_peers {
            match self.sync_peer(peer, max_events_per_peer).await {
                Ok(report) => {
                    peers_reached += 1;
                    events_recovered += report.governance.accepted + report.room.accepted;
                }
                Err(error) => peer_errors.push(format!(
                    "{}: {error:#}",
                    peer.label.as_deref().unwrap_or("unlabelled peer")
                )),
            }
        }

        self.ensure_member_join(&store, &identity, &config, None)?;
        self.ensure_identity_announcement(&store, &identity, &config)?;
        for peer in &payload.known_peers {
            match self.sync_peer(peer, max_events_per_peer).await {
                Ok(report) => {
                    events_pushed +=
                        report.governance.remote_accepted + report.room.remote_accepted;
                }
                Err(error) => peer_errors.push(format!(
                    "{} during recovery propagation: {error:#}",
                    peer.label.as_deref().unwrap_or("unlabelled peer")
                )),
            }
        }
        let profile = self.profile_summary()?;
        Ok(RecoveryReport {
            profile,
            peers_attempted: payload.known_peers.len(),
            peers_reached,
            events_recovered,
            events_pushed,
            peer_errors,
        })
    }

    pub fn create_space_invite(
        &self,
        online: &OnlineHome,
        expires_ms: i64,
    ) -> Result<SpaceInviteFileV1> {
        self.create_space_invite_with_bootstraps(online, &[], expires_ms)
    }

    pub fn create_space_invite_with_bootstraps(
        &self,
        online: &OnlineHome,
        additional_bootstraps: &[PeerRecord],
        expires_ms: i64,
    ) -> Result<SpaceInviteFileV1> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        if online.space_id != config.space.space_id
            || online.authority_peer_id != config.space.authority_peer_id
        {
            anyhow::bail!("online endpoint does not describe the configured space");
        }
        let store = self.open_store()?;
        let mut bootstraps = vec![online.peer_record(Some("Inviter".to_string()), None)?];
        for peer in additional_bootstraps {
            peer.validate()?;
            if peer.space_id != config.space.space_id
                || peer.governance_room_id != config.space.governance_room_id
                || peer.default_room != config.space.default_room_id
                || peer.authority_peer_id != config.space.authority_peer_id
            {
                anyhow::bail!("additional bootstrap peer does not match the configured space");
            }
            if !bootstraps.iter().any(|existing| existing.same_peer(peer)) {
                bootstraps.push(peer.clone());
            }
        }
        if bootstraps.len() > 8 {
            anyhow::bail!("a space invite supports at most 8 bootstrap peers");
        }
        let event = create_space_invite_event(
            &identity,
            &config.space,
            bootstraps
                .into_iter()
                .map(serde_json::to_value)
                .collect::<serde_json::Result<Vec<_>>>()?,
            expires_ms,
            now_ms(),
            store.room_heads(&config.space.governance_room_id)?,
        )?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let accepted = accept_event(&event, &governance, &config.room_context(), now_ms())
            .map_err(|error| anyhow::anyhow!("space invite rejected: {error:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(SpaceInviteFileV1 {
            v: 1,
            space: config.space,
            invite_event: event,
        })
    }

    pub fn write_space_invite(
        &self,
        online: &OnlineHome,
        expires_ms: i64,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        write_json(
            path.as_ref(),
            &self.create_space_invite(online, expires_ms)?,
        )
    }

    pub async fn join_space_from_invite(
        &self,
        invite: &SpaceInviteFileV1,
        max_events_per_peer: usize,
    ) -> Result<JoinSpaceReport> {
        if invite.v != 1 {
            anyhow::bail!("unsupported space invite file version {}", invite.v);
        }
        if self.path("identity.json").exists() || self.local_state_exists(HOME_SELECTION_STATE)? {
            anyhow::bail!("joining a space requires a fresh Voxelle home");
        }
        if max_events_per_peer == 0 {
            anyhow::bail!("max_events_per_peer must be positive");
        }
        validate_space_invite_at(&invite.space, &invite.invite_event, now_ms())?;
        let peers = invite.bootstrap_peers()?;

        ensure_private_dir(&self.root)?;
        let identity = PeerIdentity::generate_at(now_ms())?;
        write_identity_vault(&self.path("identity.json"), &identity)?;
        self.load_or_create_certificate()?;
        let config = HomeConfig {
            space: invite.space.clone(),
        };
        self.put_local_state(
            KNOWN_PEERS_STATE,
            &KnownPeersFile {
                v: 1,
                peers: peers.clone(),
            },
        )?;
        let store = self.open_store()?;
        self.ensure_space_genesis(&store, &config)?;
        self.put_local_state(
            HOME_SELECTION_STATE,
            &HomeSelectionV1 {
                v: 1,
                space_genesis_event_id: config.space.genesis.event_id.clone(),
            },
        )?;
        let governance = store.room_events(&config.space.governance_room_id)?;
        let accepted_invite = accept_event(
            &invite.invite_event,
            &governance,
            &config.room_context(),
            now_ms(),
        )
        .map_err(|error| anyhow::anyhow!("space invite rejected locally: {error:?}"))?;
        store.insert_accepted_event(accepted_invite, now_ms())?;

        let mut peers_reached = 0;
        let mut events_received = 0;
        let mut events_pushed = 0;
        let mut peer_errors = Vec::new();
        self.ensure_member_join(
            &store,
            &identity,
            &config,
            Some(&invite.invite_event.event_id),
        )?;
        for peer in &peers {
            match self.sync_peer(peer, max_events_per_peer).await {
                Ok(report) => {
                    peers_reached += 1;
                    events_received += report.governance.accepted + report.room.accepted;
                    events_pushed +=
                        report.governance.remote_accepted + report.room.remote_accepted;
                }
                Err(error) => peer_errors.push(format!(
                    "{}: {error:#}",
                    peer.label.as_deref().unwrap_or("unlabelled peer")
                )),
            }
        }
        Ok(JoinSpaceReport {
            profile: self.profile_summary()?,
            invite_id: invite.invite_event.event_id.clone(),
            peers_attempted: peers.len(),
            peers_reached,
            events_received,
            events_pushed,
            peer_errors,
        })
    }

    pub fn ui_ontology(&self) -> Result<UiOntologyView> {
        Ok(default_ui_ontology(self.ui_preferences()?))
    }

    pub fn ui_preferences(&self) -> Result<UiPreferences> {
        let Some(preferences): Option<UiPreferences> = self.local_state(UI_PREFERENCES_STATE)?
        else {
            return Ok(UiPreferences::default());
        };
        if preferences.v != 1 {
            anyhow::bail!("unsupported UI preferences version {}", preferences.v);
        }
        validate_ui_preferences(&preferences)?;
        Ok(preferences)
    }

    pub fn set_ui_preference(&self, request: SetUiPreferenceRequest) -> Result<String> {
        let mut preferences = self.ui_preferences()?;
        let id = match request {
            SetUiPreferenceRequest::SemanticToken { id, value } => {
                preferences.semantic_tokens.insert(id.clone(), value);
                id
            }
            SetUiPreferenceRequest::Metric { id, value } => {
                preferences.metrics.insert(id.clone(), value);
                id
            }
            SetUiPreferenceRequest::Behavior { id, value } => {
                preferences.behaviors.insert(id.clone(), value);
                id
            }
        };
        self.write_ui_preferences(&preferences)?;
        Ok(id)
    }

    pub fn set_workbench_layout(&self, request: SetWorkbenchLayoutRequest) -> Result<()> {
        validate_workbench_layout(&request.placements)?;
        let mut preferences = self.ui_preferences()?;
        preferences.view_placements = request
            .placements
            .into_iter()
            .map(|placement| (placement.view_id.clone(), placement))
            .collect();
        self.write_ui_preferences(&preferences)
    }

    pub fn reset_workbench_layout(&self) -> Result<()> {
        let mut preferences = self.ui_preferences()?;
        preferences.view_placements.clear();
        self.write_ui_preferences(&preferences)
    }

    pub fn reset_ui_preference(&self, kind: UiPreferenceKind, id: &str) -> Result<()> {
        let mut preferences = self.ui_preferences()?;
        let removed = match kind {
            UiPreferenceKind::SemanticToken => preferences.semantic_tokens.remove(id).is_some(),
            UiPreferenceKind::Metric => preferences.metrics.remove(id).is_some(),
            UiPreferenceKind::Behavior => preferences.behaviors.remove(id).is_some(),
        };
        if !removed {
            validate_ui_preference_id(kind, id)?;
        }
        self.write_ui_preferences(&preferences)
    }

    pub fn reset_all_ui_preferences(&self) -> Result<()> {
        self.write_ui_preferences(&UiPreferences::default())
    }

    pub fn start_service(
        &self,
        bind: SocketAddr,
        advertise: Option<SocketAddr>,
    ) -> Result<VoxelleService> {
        VoxelleService::start(self.clone(), bind, advertise, Arc::new(|| {}))
    }

    fn start_service_with_notifier(
        &self,
        bind: SocketAddr,
        advertise: Option<SocketAddr>,
        snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<VoxelleService> {
        VoxelleService::start(self.clone(), bind, advertise, snapshot_invalidated)
    }

    pub async fn diagnose_peer(&self, peer: &PeerRecord) -> Result<PeerReachabilityReport> {
        peer.validate()?;
        let identity = self.load_identity()?;
        let certificate = self.load_certificate()?;
        let node = QuicNode::bind_ipv6_loopback_with_certificate(identity, certificate)?;
        Ok(node.diagnose_peer(&peer.endpoint).await)
    }

    pub async fn sync_peer(&self, peer: &PeerRecord, max_events: usize) -> Result<PeerSyncReport> {
        peer.validate()?;
        let config = self.load_config()?;
        if peer.space_id != config.space.space_id
            || peer.governance_room_id != config.space.governance_room_id
            || peer.default_room != config.space.default_room_id
            || peer.authority_peer_id != config.space.authority_peer_id
        {
            anyhow::bail!("peer record does not match the active home authority");
        }
        if max_events == 0 {
            anyhow::bail!("max_events must be positive");
        }

        let identity = self.load_identity()?;
        let local_peer_id = identity.peer_id.clone();
        let certificate = self.load_certificate()?;
        let mut store = self.open_store()?;
        let node = QuicNode::bind_ipv6_loopback_with_certificate(identity, certificate)?;
        let endpoint = &peer.endpoint;
        let context = RoomContext::for_space(
            peer.authority_peer_id.clone(),
            peer.governance_room_id.clone(),
        );
        let limits = SyncLimits {
            max_events_per_batch: max_events,
        };
        let governance = node
            .sync_room_once(
                &mut store,
                RoomSync {
                    remote: endpoint,
                    room_id: &peer.governance_room_id,
                    context: &context,
                    now_ms: now_ms(),
                    limits,
                },
            )
            .await?;
        self.import_private_room_keys()?;
        let governance_events = store.room_events(&peer.governance_room_id)?;
        let state = derive_governance_state(&governance_events, &context, now_ms());
        let mut room = SyncStats::default();
        let mut room_ids: Vec<String> = state
            .channels
            .values()
            .filter(|channel| {
                voxelle_core::channel_allows_peer(channel, &local_peer_id)
                    && voxelle_core::channel_allows_peer(channel, &peer.endpoint.peer_id)
            })
            .map(|channel| channel.room_id.clone())
            .collect();
        room_ids.sort();
        room_ids.dedup();
        for room_id in room_ids {
            let next = node
                .sync_room_once(
                    &mut store,
                    RoomSync {
                        remote: endpoint,
                        room_id: &room_id,
                        context: &context,
                        now_ms: now_ms(),
                        limits,
                    },
                )
                .await?;
            merge_stats(&mut room, next);
        }

        Ok(PeerSyncReport { governance, room })
    }

    fn load_identity(&self) -> Result<PeerIdentity> {
        read_identity_vault(&self.path("identity.json"))
    }

    fn load_certificate(&self) -> Result<QuicCertificate> {
        read_json(&self.path("quic-cert.json"))
    }

    fn load_config(&self) -> Result<HomeConfig> {
        let selection: HomeSelectionV1 = self
            .local_state(HOME_SELECTION_STATE)?
            .ok_or_else(|| anyhow::anyhow!("home space selection is unavailable"))?;
        if selection.v != 1 {
            anyhow::bail!("unsupported home selection version {}", selection.v);
        }
        let genesis = self
            .open_store()?
            .get_event(&selection.space_genesis_event_id)?
            .ok_or_else(|| anyhow::anyhow!("selected space genesis is unavailable"))?;
        Ok(HomeConfig {
            space: space_from_genesis(&genesis, now_ms()).context("reconstruct selected space")?,
        })
    }

    fn open_store(&self) -> Result<Store> {
        Store::open(self.path("store.sqlite3"))
    }

    fn local_state<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        if !self.path("store.sqlite3").exists() {
            return Ok(None);
        }
        self.open_store()?.local_state(key)
    }

    fn local_state_exists(&self, key: &str) -> Result<bool> {
        Ok(self.local_state::<serde_json::Value>(key)?.is_some())
    }

    fn put_local_state<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        ensure_private_dir(&self.root)?;
        self.open_store()?.put_local_state(key, value)
    }

    fn write_ui_preferences(&self, preferences: &UiPreferences) -> Result<()> {
        validate_ui_preferences(preferences)?;
        self.put_local_state(UI_PREFERENCES_STATE, preferences)
    }

    fn load_or_create_identity(&self) -> Result<PeerIdentity> {
        if self.path("identity.json").exists() {
            return self.load_identity();
        }
        let identity = PeerIdentity::generate_at(now_ms())?;
        write_identity_vault(&self.path("identity.json"), &identity)?;
        Ok(identity)
    }

    fn load_or_create_certificate(&self) -> Result<QuicCertificate> {
        if self.path("quic-cert.json").exists() {
            return self.load_certificate();
        }
        let certificate = QuicCertificate::generate()?;
        write_secret_json(&self.path("quic-cert.json"), &certificate)?;
        Ok(certificate)
    }

    fn ensure_space_genesis(&self, store: &Store, config: &HomeConfig) -> Result<()> {
        if store.has_event(&config.space.genesis.event_id)? {
            return Ok(());
        }
        let context = config.room_context();
        let accepted = accept_event(&config.space.genesis, &[], &context, now_ms())
            .map_err(|error| anyhow::anyhow!("space genesis rejected: {error:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(())
    }

    fn ensure_member_join(
        &self,
        store: &Store,
        identity: &PeerIdentity,
        config: &HomeConfig,
        invite_id: Option<&str>,
    ) -> Result<()> {
        let governance = store.room_events(&config.space.governance_room_id)?;
        let existing_join = governance.iter().any(|event| {
            event.kind == "MEMBER_JOIN"
                && event.author_peer_id == identity.peer_id
                && event.body.get("peer_id").and_then(|value| value.as_str())
                    == Some(identity.peer_id.as_str())
        });
        if existing_join {
            return Ok(());
        }

        let context = config.room_context();
        let join = create_event(
            identity,
            create_delegation(
                identity,
                now_ms() - 60_000,
                now_ms() + 30 * 24 * 60 * 60_000,
                vec!["room:join".to_string()],
            )?,
            &config.space.governance_room_id,
            now_ms(),
            "MEMBER_JOIN",
            store.room_heads(&config.space.governance_room_id)?,
            serde_json::json!({
                "peer_id": identity.peer_id,
                "peer_pub": identity.peer.spki_b64,
                "encryption_pub": identity_encryption_public_b64(identity)?,
                "invite_id": invite_id,
            }),
        )?;
        let accepted = accept_event(&join, &governance, &context, now_ms())
            .map_err(|e| anyhow::anyhow!("member join rejected: {e:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(())
    }

    fn ensure_identity_announcement(
        &self,
        store: &Store,
        identity: &PeerIdentity,
        config: &HomeConfig,
    ) -> Result<()> {
        let existing = store
            .room_events(&config.space.default_room_id)?
            .into_iter()
            .any(|event| {
                event.kind == "IDENTITY_UPDATE"
                    && event.author_peer_id == identity.peer_id
                    && event.author_device_id == identity.device.id
            });
        if existing {
            return Ok(());
        }
        let governance = store.room_events(&config.space.governance_room_id)?;
        let event = create_event(
            identity,
            create_delegation(
                identity,
                now_ms() - 60_000,
                now_ms() + 30 * 24 * 60 * 60_000,
                vec!["room:post".to_string()],
            )?,
            &config.space.default_room_id,
            now_ms(),
            "IDENTITY_UPDATE",
            store.room_heads(&config.space.default_room_id)?,
            serde_json::json!({
                "peer_id": identity.peer_id,
                "device_id": identity.device.id,
            }),
        )?;
        let accepted = accept_event(&event, &governance, &config.room_context(), now_ms())
            .map_err(|error| anyhow::anyhow!("identity announcement rejected: {error:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(())
    }
}

fn unread_start(events: &[EventV1], last_read_event_id: Option<&String>) -> usize {
    last_read_event_id
        .and_then(|event_id| events.iter().position(|event| &event.event_id == event_id))
        .map_or(0, |index| index + 1)
}

fn unread_count(
    mut events: Vec<EventV1>,
    last_read_event_id: Option<&String>,
    local_peer_id: &str,
) -> usize {
    events.sort_by(|left, right| {
        left.created_ms
            .cmp(&right.created_ms)
            .then(left.event_id.cmp(&right.event_id))
    });
    let start = unread_start(&events, last_read_event_id);
    events
        .into_iter()
        .skip(start)
        .filter(|event| {
            event.author_peer_id != local_peer_id
                && matches!(event.kind.as_str(), "MSG_POST" | "ATTACHMENT_ADD")
        })
        .count()
}

fn project_messages(mut events: Vec<EventV1>) -> Vec<MessageView> {
    events.sort_by(|left, right| {
        left.created_ms
            .cmp(&right.created_ms)
            .then(left.event_id.cmp(&right.event_id))
    });
    let mut messages: BTreeMap<String, MessageView> = BTreeMap::new();
    let redacted_targets = events
        .iter()
        .filter(|event| event.kind == "MSG_REDACT")
        .map(|event| string_event_body(event, "target_event_id"))
        .collect::<std::collections::BTreeSet<_>>();
    let mut order = Vec::new();
    let mut reactions: BTreeMap<String, BTreeMap<String, std::collections::BTreeSet<String>>> =
        BTreeMap::new();
    for event in &events {
        match event.kind.as_str() {
            "MSG_POST" => {
                let event_id = event.event_id.clone();
                let redacted = redacted_targets.contains(&event_id);
                order.push(event_id.clone());
                messages.insert(
                    event_id,
                    MessageView {
                        event_id: event.event_id.clone(),
                        created_ms: event.created_ms,
                        author_peer_id: event.author_peer_id.clone(),
                        text: if redacted {
                            "Message removed".to_string()
                        } else {
                            event
                                .body
                                .get("text")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_string()
                        },
                        edited_ms: None,
                        redacted,
                        mentions: event
                            .body
                            .get("mentions")
                            .and_then(serde_json::Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect(),
                        thread_root_event_id: event
                            .body
                            .get("thread_root_event_id")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned),
                        reply_count: 0,
                        pinned: false,
                        reactions: Vec::new(),
                        attachments: Vec::new(),
                    },
                );
            }
            "ATTACHMENT_ADD" => {
                let event_id = event.event_id.clone();
                order.push(event_id.clone());
                messages.insert(
                    event_id,
                    MessageView {
                        event_id: event.event_id.clone(),
                        created_ms: event.created_ms,
                        author_peer_id: event.author_peer_id.clone(),
                        text: String::new(),
                        edited_ms: None,
                        redacted: false,
                        mentions: Vec::new(),
                        thread_root_event_id: None,
                        reply_count: 0,
                        pinned: false,
                        reactions: Vec::new(),
                        attachments: vec![AttachmentView {
                            event_id: event.event_id.clone(),
                            filename: string_event_body(event, "filename"),
                            mime: string_event_body(event, "mime"),
                            sha256: string_event_body(event, "sha256"),
                            data_b64: string_event_body(event, "data_b64"),
                        }],
                    },
                );
            }
            "MSG_EDIT" => {
                let target = string_event_body(event, "target_event_id");
                if !redacted_targets.contains(&target) {
                    if let Some(message) = event_target_message(&mut messages, event) {
                        message.text = string_event_body(event, "text");
                        message.edited_ms = Some(event.created_ms);
                        message.mentions = event
                            .body
                            .get("mentions")
                            .and_then(serde_json::Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect();
                    }
                }
            }
            "MSG_REDACT" => {
                if let Some(message) = event_target_message(&mut messages, event) {
                    message.text = "Message removed".to_string();
                    message.redacted = true;
                    message.attachments.clear();
                    message.mentions.clear();
                }
            }
            "PIN_ADD" | "PIN_REMOVE" => {
                if let Some(message) = event_target_message(&mut messages, event) {
                    message.pinned = event.kind == "PIN_ADD";
                }
            }
            "REACTION_ADD" | "REACTION_REMOVE" => {
                let target = string_event_body(event, "target_event_id");
                let emoji = string_event_body(event, "emoji");
                let peers = reactions
                    .entry(target)
                    .or_default()
                    .entry(emoji)
                    .or_default();
                if event.kind == "REACTION_ADD" {
                    peers.insert(event.author_peer_id.clone());
                } else {
                    peers.remove(&event.author_peer_id);
                }
            }
            _ => {}
        }
    }
    let roots: Vec<String> = messages
        .values()
        .filter_map(|message| message.thread_root_event_id.clone())
        .collect();
    for root in roots {
        if let Some(message) = messages.get_mut(&root) {
            message.reply_count += 1;
        }
    }
    for (target, by_emoji) in reactions {
        if let Some(message) = messages.get_mut(&target) {
            message.reactions = by_emoji
                .into_iter()
                .filter(|(_, peers)| !peers.is_empty())
                .map(|(emoji, peers)| ReactionView {
                    emoji,
                    peer_ids: peers.into_iter().collect(),
                })
                .collect();
        }
    }
    order
        .into_iter()
        .filter_map(|event_id| messages.remove(&event_id))
        .collect()
}

fn event_target_message<'a>(
    messages: &'a mut BTreeMap<String, MessageView>,
    event: &EventV1,
) -> Option<&'a mut MessageView> {
    let target = event.body.get("target_event_id")?.as_str()?;
    messages.get_mut(target)
}

fn string_event_body(event: &EventV1, field: &str) -> String {
    event
        .body
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

impl VoxelleCommandHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::new_with_notifier(root, Arc::new(|| {}))
    }

    pub fn new_with_notifier(
        root: impl Into<PathBuf>,
        snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self::new_with_notifier_and_update_keys(
            root,
            snapshot_invalidated,
            embedded_trusted_update_keys().expect("valid embedded update trust roots"),
        )
    }

    pub fn new_with_notifier_and_update_keys(
        root: impl Into<PathBuf>,
        snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
        trusted_update_keys: Vec<TrustedReleaseKey>,
    ) -> Self {
        let root = root.into();
        let update_manager = UpdateManager::new(
            root.join("product-updates"),
            env!("CARGO_PKG_VERSION"),
            trusted_update_keys,
        )
        .expect("valid product update manager configuration");
        let (product_generation, mut product_generation_notice) =
            load_product_generation(&update_manager);
        let update_phase = match update_manager.load_staged() {
            Ok(Some(staged)) => match parse_product_generation(&staged)
                .and_then(|generation| validate_product_generation(&generation))
            {
                Ok(()) => "staged".to_string(),
                Err(error) => {
                    product_generation_notice = Some(format!(
                        "Staged product generation failed validation: {error:#}. Discard it before continuing."
                    ));
                    "failed".to_string()
                }
            },
            Ok(None) => "idle".to_string(),
            Err(error) => {
                product_generation_notice = Some(format!(
                    "Staged product generation could not be verified: {error:#}. Discard it before continuing."
                ));
                "failed".to_string()
            }
        };
        Self {
            home: VoxelleHome::new(root),
            service: None,
            activity: Vec::new(),
            next_activity_id: 1,
            last_space_invite_json: None,
            selected_room_id: None,
            search_results: Vec::new(),
            snapshot_invalidated,
            update_manager,
            product_generation,
            product_generation_notice,
            available_product_update: None,
            update_phase,
        }
    }

    pub fn update_transport_context(&self) -> (UpdateManager, u64) {
        (
            self.update_manager.clone(),
            self.product_generation
                .as_ref()
                .map_or(0, |generation| generation.pointer.sequence),
        )
    }

    pub fn available_product_update(&self) -> Result<AvailableProductUpdate> {
        self.available_product_update
            .clone()
            .ok_or_else(|| anyhow::anyhow!("check GitHub Releases before staging an update"))
    }

    pub fn record_available_product_update(
        &mut self,
        available: AvailableProductUpdate,
    ) -> Result<ShellSnapshotView> {
        let active_sequence = self
            .product_generation
            .as_ref()
            .map_or(0, |generation| generation.pointer.sequence);
        if available.manifest.channel != "beta" {
            return Err(anyhow::anyhow!(
                "latest signed release is on unsupported channel {}",
                available.manifest.channel
            ));
        }
        if available.manifest.sequence <= active_sequence {
            self.available_product_update = None;
            self.update_phase = "current".to_string();
            self.product_generation_notice = Some(format!(
                "This installation is current at sequence {active_sequence}."
            ));
        } else {
            self.update_phase = "available".to_string();
            self.product_generation_notice = Some(format!(
                "Signed generation {} (sequence {}) is available from GitHub Releases.",
                available.manifest.release_id, available.manifest.sequence
            ));
            self.available_product_update = Some(available);
        }
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn stage_downloaded_product_update(
        &mut self,
        downloaded: DownloadedProductUpdate,
    ) -> Result<ShellSnapshotView> {
        let verified = self
            .update_manager
            .verify_bytes(&downloaded.package_bytes)?;
        let generation = parse_product_generation(&verified)?;
        validate_product_generation(&generation)?;
        let pointer = self.update_manager.stage_candidate(&verified)?;
        self.available_product_update = Some(downloaded.available);
        self.update_phase = "staged".to_string();
        self.product_generation_notice = Some(format!(
            "Verified and staged generation {} (sequence {}). Activation is still explicit.",
            pointer.release_id, pointer.sequence
        ));
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn activate_staged_product_update(&mut self) -> Result<ShellSnapshotView> {
        let verified = self
            .update_manager
            .load_staged()?
            .ok_or_else(|| anyhow::anyhow!("no verified product generation is staged"))?;
        let generation = parse_product_generation(&verified)?;
        validate_product_generation(&generation)?;
        let pointer = self.update_manager.activate_staged()?;
        self.product_generation = Some(ActiveProductGeneration {
            pointer: pointer.clone(),
            generation,
            source: ActiveSource::Current,
        });
        self.available_product_update = None;
        self.update_phase = "active".to_string();
        self.product_generation_notice = Some(format!(
            "Activated staged product generation {}.",
            pointer.release_id
        ));
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("activated staged product generation {}", pointer.release_id),
        );
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn discard_staged_product_update(&mut self) -> Result<ShellSnapshotView> {
        let discarded = self.update_manager.discard_staged()?;
        self.update_phase = "idle".to_string();
        self.product_generation_notice = Some(match discarded {
            Some(pointer) => format!("Discarded staged generation {}.", pointer.release_id),
            None => "No staged product generation was present.".to_string(),
        });
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn record_product_update_failure(&mut self, message: &str) {
        self.update_phase = "failed".to_string();
        self.product_generation_notice = Some(format!("Product update failed: {message}"));
        self.push_activity(
            ServiceActivityLevel::Error,
            format!("product update failed: {message}"),
        );
        (self.snapshot_invalidated)();
    }

    pub fn install_release_trust_transition(
        &mut self,
        request: InstallTrustTransitionRequest,
    ) -> Result<ShellSnapshotView> {
        let transition = self
            .update_manager
            .apply_trust_transition_bytes(request.transition_json.as_bytes())?;
        self.product_generation_notice = Some(format!(
            "Applied signed release trust transition {}. {} release key(s) are now trusted.",
            transition.sequence,
            self.update_manager.trusted_key_count()
        ));
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("applied release trust transition {}", transition.sequence),
        );
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn install_product_update(
        &mut self,
        request: InstallProductUpdateRequest,
    ) -> Result<ShellSnapshotView> {
        let verified = self
            .update_manager
            .verify_bytes(request.package_json.as_bytes())?;
        let generation = parse_product_generation(&verified)?;
        validate_product_generation(&generation)?;
        let pointer = self.update_manager.activate(&verified)?;
        self.product_generation = Some(ActiveProductGeneration {
            pointer: pointer.clone(),
            generation,
            source: ActiveSource::Current,
        });
        self.product_generation_notice = None;
        self.update_phase = "active".to_string();
        self.push_activity(
            ServiceActivityLevel::Info,
            format!(
                "activated signed product generation {} (sequence {})",
                pointer.release_id, pointer.sequence
            ),
        );
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn rollback_product_update(&mut self) -> Result<ShellSnapshotView> {
        if self.update_manager.previous_pointer()?.is_none() {
            let previous = self
                .update_manager
                .deactivate_to_builtin()?
                .ok_or_else(|| anyhow::anyhow!("no signed product generation is active"))?;
            self.product_generation = None;
            self.product_generation_notice = Some(format!(
                "Rolled back signed generation {} to the built-in recovery generation.",
                previous.release_id
            ));
            self.push_activity(
                ServiceActivityLevel::Info,
                format!(
                    "rolled back product generation {} to builtin recovery",
                    previous.release_id
                ),
            );
            (self.snapshot_invalidated)();
            return self.snapshot();
        }
        let pointer = self.update_manager.rollback()?;
        let loaded = self
            .update_manager
            .load_active()?
            .ok_or_else(|| anyhow::anyhow!("rolled-back product generation is unavailable"))?;
        let generation = parse_product_generation(&loaded.package)?;
        validate_product_generation(&generation)?;
        self.product_generation = Some(ActiveProductGeneration {
            pointer: pointer.clone(),
            generation,
            source: ActiveSource::Current,
        });
        self.product_generation_notice = Some(format!(
            "Rolled back to signed generation {}.",
            pointer.release_id
        ));
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("rolled back product generation to {}", pointer.release_id),
        );
        (self.snapshot_invalidated)();
        self.snapshot()
    }

    pub fn snapshot(&mut self) -> Result<ShellSnapshotView> {
        self.drain_service_events();
        self.snapshot_without_drain()
    }

    pub fn init_home(&mut self, request: InitHomeRequest) -> Result<ShellSnapshotView> {
        let default_room = request.default_room.as_deref().unwrap_or(DEFAULT_ROOM_ID);
        self.home.init(default_room)?;
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("initialized home for {default_room}"),
        );
        self.snapshot()
    }

    pub fn start_service(&mut self, request: StartServiceRequest) -> Result<ShellSnapshotView> {
        if self.service.is_some() {
            return self.snapshot();
        }

        let bind = request
            .bind
            .unwrap_or_else(|| SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0));
        let service = self.home.start_service_with_notifier(
            bind,
            request.advertise,
            self.snapshot_invalidated.clone(),
        )?;
        let addr = service.online().endpoint.addr;
        self.service = Some(service);
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("service started at {addr}"),
        );
        self.snapshot()
    }

    pub fn stop_service(&mut self) -> Result<ShellSnapshotView> {
        if let Some(service) = self.service.take() {
            service.stop()?;
            self.push_activity(ServiceActivityLevel::Info, "service stopped");
        }
        self.snapshot()
    }

    pub fn create_space_invite(
        &mut self,
        request: CreateSpaceInviteRequest,
    ) -> Result<ShellSnapshotView> {
        let online = self
            .service
            .as_ref()
            .map(VoxelleService::online)
            .ok_or_else(|| anyhow::anyhow!("go online before creating a space invite"))?;
        let minutes = request
            .expires_minutes
            .unwrap_or(24 * 60)
            .clamp(1, 30 * 24 * 60);
        let expires_ms = now_ms().saturating_add((minutes as i64).saturating_mul(60_000));
        let additional_bootstraps = self
            .home
            .known_peers()?
            .into_iter()
            .filter(|peer| {
                peer.space_id == online.space_id
                    && peer.governance_room_id == online.governance_room_id
                    && peer.default_room == online.default_room
                    && peer.authority_peer_id == online.authority_peer_id
            })
            .take(7)
            .collect::<Vec<_>>();
        let invite = self.home.create_space_invite_with_bootstraps(
            online,
            &additional_bootstraps,
            expires_ms,
        )?;
        let bootstrap_count = invite.bootstrap_peers()?.len();
        self.last_space_invite_json = Some(serde_json::to_string_pretty(&invite)? + "\n");
        self.push_activity(
            ServiceActivityLevel::Info,
            format!(
                "created signed space invite {} with {} bootstrap peer(s)",
                invite.invite_event.event_id, bootstrap_count
            ),
        );
        self.snapshot()
    }

    pub async fn join_space(&mut self, request: JoinSpaceRequest) -> Result<ShellSnapshotView> {
        let invite: SpaceInviteFileV1 = serde_json::from_str(&request.space_invite_json)
            .context("parse signed space invite JSON")?;
        let report = self
            .home
            .join_space_from_invite(&invite, request.max_events.unwrap_or(4096))
            .await?;
        self.push_activity(
            ServiceActivityLevel::Info,
            format!(
                "joined space {} via {}, received {}, pushed {}",
                invite.space.name, report.invite_id, report.events_received, report.events_pushed
            ),
        );
        if self.service.is_none() {
            self.service = Some(self.home.start_service_with_notifier(
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
                None,
                self.snapshot_invalidated.clone(),
            )?);
            self.push_activity(ServiceActivityLevel::Info, "service started after join");
        }
        self.snapshot()
    }

    pub async fn send_message(&mut self, request: SendMessageRequest) -> Result<ShellSnapshotView> {
        let room = request.room.as_deref().or(self.selected_room_id.as_deref());
        let event = self.home.send_message_with_metadata(
            &request.text,
            room,
            request.mentions,
            request.thread_root_event_id,
        )?;
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("sent message {}", event.event_id),
        );
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub fn select_channel(&mut self, request: SelectChannelRequest) -> Result<ShellSnapshotView> {
        if !self
            .home
            .channels(Some(&request.room_id))?
            .iter()
            .any(|channel| channel.room_id == request.room_id)
        {
            anyhow::bail!("channel is unknown or inaccessible");
        }
        self.home.mark_read(Some(&request.room_id))?;
        self.selected_room_id = Some(request.room_id);
        self.snapshot()
    }

    pub fn mark_read(&mut self, request: MarkReadRequest) -> Result<ShellSnapshotView> {
        let room = request
            .room_id
            .as_deref()
            .or(self.selected_room_id.as_deref());
        self.home.mark_read(room)?;
        self.snapshot()
    }

    pub async fn create_channel(
        &mut self,
        request: CreateChannelRequest,
    ) -> Result<ShellSnapshotView> {
        let event = self.home.create_channel(&request)?;
        self.selected_room_id = event
            .body
            .get("room_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn rotate_channel_key(
        &mut self,
        request: RotateChannelKeyRequest,
    ) -> Result<ShellSnapshotView> {
        self.home.rotate_channel_key(&request)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn join_call(&mut self, mut request: CallJoinRequest) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        if self.service.is_none() {
            self.service = Some(self.home.start_service_with_notifier(
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
                None,
                self.snapshot_invalidated.clone(),
            )?);
            self.push_activity(ServiceActivityLevel::Info, "service started for room call");
        }
        self.home.join_call(&request)?;
        self.sync_known_peers(512).await?;
        self.snapshot()
    }

    pub async fn signal_call(
        &mut self,
        mut request: CallSignalRequest,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.signal_call(&request)?;
        self.sync_known_peers(512).await?;
        self.snapshot()
    }

    pub async fn heartbeat_call(
        &mut self,
        mut request: CallLeaveRequest,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.heartbeat_call(&request)?;
        self.sync_known_peers(512).await?;
        self.snapshot()
    }

    pub async fn leave_call(&mut self, mut request: CallLeaveRequest) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.leave_call(&request)?;
        self.sync_known_peers(512).await?;
        self.snapshot()
    }

    pub async fn edit_message(
        &mut self,
        mut request: EditMessageRequest,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.edit_message(&request)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn redact_message(
        &mut self,
        mut request: MessageTargetRequest,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.redact_message(&request)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn set_reaction(
        &mut self,
        mut request: ReactionRequest,
        add: bool,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.set_reaction(&request, add)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn set_pin(
        &mut self,
        mut request: MessageTargetRequest,
        add: bool,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.set_pin(&request, add)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn add_attachment(
        &mut self,
        mut request: AttachmentRequest,
    ) -> Result<ShellSnapshotView> {
        request.room = request.room.or_else(|| self.selected_room_id.clone());
        self.home.add_attachment(&request)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn update_profile(
        &mut self,
        request: ProfileUpdateRequest,
    ) -> Result<ShellSnapshotView> {
        self.home.update_profile(&request)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn create_role(&mut self, request: CreateRoleRequest) -> Result<ShellSnapshotView> {
        self.home.create_role(&request)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn assign_role(
        &mut self,
        request: AssignRoleRequest,
        grant: bool,
    ) -> Result<ShellSnapshotView> {
        self.home.assign_role(&request, grant)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub async fn ban_member(
        &mut self,
        request: BanMemberRequest,
        ban: bool,
    ) -> Result<ShellSnapshotView> {
        self.home.ban_member(&request, ban)?;
        self.sync_known_peers(256).await?;
        self.snapshot()
    }

    pub fn search_messages(&mut self, request: SearchMessagesRequest) -> Result<ShellSnapshotView> {
        self.search_results = self.home.search_messages(&request)?;
        self.snapshot()
    }

    pub async fn refresh_and_sync(&mut self) -> Result<ShellSnapshotView> {
        if self.service.is_some() && self.home.local_state_exists(HOME_SELECTION_STATE)? {
            self.sync_known_peers(256).await?;
        }
        self.snapshot()
    }

    pub fn import_peer_record(
        &mut self,
        request: ImportPeerRecordRequest,
    ) -> Result<ShellSnapshotView> {
        let peer_record: PeerRecord =
            serde_json::from_str(&request.peer_record_json).context("parse peer record JSON")?;
        let label = peer_record
            .label
            .clone()
            .unwrap_or_else(|| short_peer_label(&peer_record.endpoint.peer_id));
        self.home.import_peer_record(peer_record)?;
        self.push_activity(ServiceActivityLevel::Info, format!("imported peer {label}"));
        self.snapshot()
    }

    pub fn set_ui_preference(
        &mut self,
        request: SetUiPreferenceRequest,
    ) -> Result<ShellSnapshotView> {
        let id = self.home.set_ui_preference(request)?;
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("updated UI preference {id}"),
        );
        self.snapshot()
    }

    pub fn set_workbench_layout(
        &mut self,
        request: SetWorkbenchLayoutRequest,
    ) -> Result<ShellSnapshotView> {
        self.home.set_workbench_layout(request)?;
        self.snapshot()
    }

    pub fn reset_workbench_layout(&mut self) -> Result<ShellSnapshotView> {
        self.home.reset_workbench_layout()?;
        self.push_activity(ServiceActivityLevel::Info, "reset workbench layout");
        self.snapshot()
    }

    pub async fn diagnose_peer(
        &mut self,
        request: PeerCommandRequest,
    ) -> Result<ShellSnapshotView> {
        let peer = self.find_known_peer(&request.peer_id, &request.device_id)?;
        let label = peer
            .label
            .clone()
            .unwrap_or_else(|| short_peer_label(&peer.endpoint.peer_id));
        let report = self.home.diagnose_peer(&peer).await?;
        if report.reachable {
            self.push_activity(
                ServiceActivityLevel::Info,
                format!("diagnostic reached {label}"),
            );
        } else {
            self.push_activity(
                ServiceActivityLevel::Error,
                format!(
                    "diagnostic failed for {label}: {}",
                    report.error.as_deref().unwrap_or("no error detail")
                ),
            );
        }
        self.snapshot()
    }

    pub async fn sync_peer(&mut self, request: PeerCommandRequest) -> Result<ShellSnapshotView> {
        let peer = self.find_known_peer(&request.peer_id, &request.device_id)?;
        let label = peer
            .label
            .clone()
            .unwrap_or_else(|| short_peer_label(&peer.endpoint.peer_id));
        let max_events = request.max_events.unwrap_or(64);
        let report = self.home.sync_peer(&peer, max_events).await?;
        self.push_activity(
            ServiceActivityLevel::Info,
            format!(
                "synced {label}: governance accepted {}, room accepted {}",
                report.governance.accepted, report.room.accepted
            ),
        );
        self.snapshot()
    }

    async fn sync_known_peers(&mut self, max_events: usize) -> Result<()> {
        let peers = self.home.known_peers()?;
        let mut tasks = tokio::task::JoinSet::new();
        for peer in peers {
            let home = self.home.clone();
            tasks.spawn(async move {
                let result = home
                    .sync_peer(&peer, max_events)
                    .await
                    .map_err(|error| format!("{error:#}"));
                (peer, result)
            });
        }
        while let Some(result) = tasks.join_next().await {
            let (peer, sync) = result.context("automatic peer sync task failed")?;
            let label = peer
                .label
                .as_deref()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| short_peer_label(&peer.endpoint.peer_id));
            match sync {
                Ok(report) => {
                    let received = report.governance.accepted + report.room.accepted;
                    let pushed = report.governance.remote_accepted + report.room.remote_accepted;
                    if received > 0 || pushed > 0 {
                        self.push_activity(
                            ServiceActivityLevel::Info,
                            format!(
                                "automatic sync with {label}: received {received}, pushed {pushed}"
                            ),
                        );
                    }
                }
                Err(error) => self.push_activity(
                    ServiceActivityLevel::Error,
                    format!("automatic sync could not reach {label}: {error}"),
                ),
            }
        }
        Ok(())
    }

    fn snapshot_without_drain(&self) -> Result<ShellSnapshotView> {
        let online = self.service.as_ref().map(VoxelleService::online);
        let (home, home_error) = match self
            .home
            .home_screen_view_for_room(online, self.selected_room_id.as_deref())
        {
            Ok(mut home) => {
                if let Some(invite) = home.invite.as_mut() {
                    invite.space_invite_json = self.last_space_invite_json.clone();
                }
                (Some(home), None)
            }
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        let preferences = self.home.ui_preferences()?;
        let ui_ontology = match &self.product_generation {
            Some(active) => apply_ui_preferences(active.generation.ontology.clone(), preferences),
            None => default_ui_ontology(preferences),
        };
        let component = self
            .product_generation
            .as_ref()
            .map(|active| active.generation.component.clone())
            .unwrap_or_else(|| builtin_product_generation().component);
        let mut component_digest = Sha256::new();
        component_digest.update(b"voxelle-product-component/v1\0");
        component_digest.update(component.source.as_bytes());
        component_digest.update(b"\0styles\0");
        component_digest.update(component.styles.as_bytes());
        let product_component = ProductComponentView {
            api_version: component.api_version,
            digest: format!("{:x}", component_digest.finalize()),
            source: component.source,
            styles: component.styles,
        };
        let product_generation = self.product_generation_status()?;
        Ok(ShellSnapshotView {
            home_root: self.home.root.clone(),
            home,
            home_error,
            network_health: self.home.network_health_view(online)?,
            ui_ontology,
            product_generation,
            product_component,
            service_activity: self.activity.clone(),
            search_results: self.search_results.clone(),
        })
    }

    fn product_generation_status(&self) -> Result<ProductGenerationStatusView> {
        let previous_available =
            self.product_generation.is_some() || self.update_manager.previous_pointer()?.is_some();
        let (active_release_id, active_sequence, source) = match &self.product_generation {
            Some(active) => (
                active.pointer.release_id.clone(),
                active.pointer.sequence,
                match active.source {
                    ActiveSource::Current => "signed".to_string(),
                    ActiveSource::PreviousRecovery => "recovered_previous".to_string(),
                },
            ),
            None => ("builtin-recovery".to_string(), 0, "builtin".to_string()),
        };
        let staged = self.update_manager.staged_pointer().ok().flatten();
        Ok(ProductGenerationStatusView {
            kernel_version: env!("CARGO_PKG_VERSION").to_string(),
            active_release_id,
            active_sequence,
            source,
            previous_available,
            update_authentication_available: self.update_manager.trusted_key_count() > 0,
            trusted_update_key_count: self.update_manager.trusted_key_count(),
            trust_sequence: self.update_manager.trust_sequence(),
            available_release_id: self
                .available_product_update
                .as_ref()
                .map(|available| available.manifest.release_id.clone()),
            available_sequence: self
                .available_product_update
                .as_ref()
                .map(|available| available.manifest.sequence),
            staged_release_id: staged.as_ref().map(|pointer| pointer.release_id.clone()),
            staged_sequence: staged.map(|pointer| pointer.sequence),
            phase: self.update_phase.clone(),
            notice: self.product_generation_notice.clone(),
        })
    }

    fn drain_service_events(&mut self) {
        let Some(service) = self.service.as_ref() else {
            return;
        };

        let mut drained = Vec::new();
        while let Some(event) = service.try_recv_event() {
            let level = match event {
                VoxelleServiceEvent::Failed(_) => ServiceActivityLevel::Error,
                VoxelleServiceEvent::Served(_) | VoxelleServiceEvent::Stopped => {
                    ServiceActivityLevel::Info
                }
            };
            drained.push((level, event.summary()));
        }

        for (level, summary) in drained {
            self.push_activity(level, summary);
        }
    }

    fn push_activity(&mut self, level: ServiceActivityLevel, summary: impl Into<String>) {
        let item = ServiceActivityItem {
            id: self.next_activity_id,
            level,
            summary: summary.into(),
        };
        self.next_activity_id += 1;
        self.activity.push(item);
        if self.activity.len() > 200 {
            let overflow = self.activity.len() - 200;
            self.activity.drain(0..overflow);
        }
    }

    fn find_known_peer(&self, peer_id: &str, device_id: &str) -> Result<PeerRecord> {
        self.home
            .known_peers()?
            .into_iter()
            .find(|peer| peer.endpoint.peer_id == peer_id && peer.endpoint.device_id == device_id)
            .with_context(|| {
                format!(
                    "unknown peer record for {} / {}",
                    short_peer_label(peer_id),
                    short_peer_label(device_id)
                )
            })
    }
}

impl PeerRecord {
    pub fn validate(&self) -> Result<()> {
        if self.v != 1 {
            anyhow::bail!("unsupported peer record version {}", self.v);
        }
        if self.default_room.trim().is_empty() {
            anyhow::bail!("peer record default room is empty");
        }
        if !self.space_id.starts_with("s:")
            || self.governance_room_id != format!("{}:governance", self.space_id)
            || !self
                .default_room
                .starts_with(&format!("{}:channel:", self.space_id))
        {
            anyhow::bail!("peer record room identifiers do not match its space");
        }
        if !self.authority_peer_id.starts_with("p:") {
            anyhow::bail!("peer record authority is not a principal ID");
        }
        self.endpoint.validate()
    }

    pub fn same_peer(&self, other: &Self) -> bool {
        self.endpoint.peer_id == other.endpoint.peer_id
            && self.endpoint.device_id == other.endpoint.device_id
    }
}

impl SpaceInviteFileV1 {
    pub fn validate_at(&self, now_ms: i64) -> Result<()> {
        if self.v != 1 {
            anyhow::bail!("unsupported space invite file version {}", self.v);
        }
        validate_space_invite_at(&self.space, &self.invite_event, now_ms)?;
        self.bootstrap_peers().map(|_| ())
    }

    pub fn bootstrap_peers(&self) -> Result<Vec<PeerRecord>> {
        let values = self
            .invite_event
            .body
            .get("bootstrap_peers")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("space invite bootstrap peers missing"))?;
        let mut peers = Vec::with_capacity(values.len());
        for value in values {
            let peer: PeerRecord =
                serde_json::from_value(value.clone()).context("parse signed bootstrap peer")?;
            peer.validate()?;
            if peer.space_id != self.space.space_id
                || peer.governance_room_id != self.space.governance_room_id
                || peer.default_room != self.space.default_room_id
                || peer.authority_peer_id != self.space.authority_peer_id
            {
                anyhow::bail!("signed bootstrap peer does not match invited space");
            }
            peers.push(peer);
        }
        Ok(peers)
    }
}

impl OnlineHome {
    pub fn peer_record(&self, label: Option<String>, room: Option<&str>) -> Result<PeerRecord> {
        let record = PeerRecord {
            v: 1,
            label,
            space_id: self.space_id.clone(),
            governance_room_id: self.governance_room_id.clone(),
            default_room: room.unwrap_or(&self.default_room).to_string(),
            authority_peer_id: self.authority_peer_id.clone(),
            endpoint: self.endpoint.clone(),
        };
        record.validate()?;
        Ok(record)
    }

    pub fn invite_view(
        &self,
        label: Option<String>,
        room: Option<&str>,
    ) -> Result<InviteExchangeView> {
        let peer_record = self.peer_record(label, room)?;
        let peer_record_json = serde_json::to_string_pretty(&peer_record)? + "\n";
        Ok(InviteExchangeView {
            peer_record,
            peer_record_json,
            space_invite_json: None,
        })
    }
}

impl RuntimeStatusView {
    fn offline() -> Self {
        Self {
            state: RuntimeState::Offline,
            listen_addr: None,
            advertised_addr: None,
            reachability_notes: vec!["offline".to_string()],
        }
    }

    fn online(online: &OnlineHome) -> Self {
        Self {
            state: RuntimeState::Online,
            listen_addr: Some(online.local_report.listen_addr),
            advertised_addr: Some(online.local_report.advertised_addr),
            reachability_notes: online.local_report.notes.clone(),
        }
    }
}

impl PeerListItemView {
    fn from_peer_record(record: PeerRecord) -> Self {
        Self {
            label: record
                .label
                .clone()
                .unwrap_or_else(|| short_peer_label(&record.endpoint.peer_id)),
            peer_id: record.endpoint.peer_id,
            device_id: record.endpoint.device_id,
            addr: record.endpoint.addr,
            default_room: record.default_room,
        }
    }
}

impl NetworkHealthRow {
    fn working(id: &str, label: &str, summary: impl Into<String>) -> Self {
        Self::new(id, label, NetworkHealthStatus::Working, summary, None)
    }

    fn needs_attention(
        id: &str,
        label: &str,
        summary: impl Into<String>,
        primary_action: Option<&str>,
    ) -> Self {
        Self::new(
            id,
            label,
            NetworkHealthStatus::NeedsAttention,
            summary,
            primary_action,
        )
    }

    fn unknown(
        id: &str,
        label: &str,
        summary: impl Into<String>,
        primary_action: Option<&str>,
    ) -> Self {
        Self::new(
            id,
            label,
            NetworkHealthStatus::Unknown,
            summary,
            primary_action,
        )
    }

    fn broken(
        id: &str,
        label: &str,
        summary: impl Into<String>,
        primary_action: Option<&str>,
    ) -> Self {
        Self::new(
            id,
            label,
            NetworkHealthStatus::Broken,
            summary,
            primary_action,
        )
    }

    fn new(
        id: &str,
        label: &str,
        status: NetworkHealthStatus,
        summary: impl Into<String>,
        primary_action: Option<&str>,
    ) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            status,
            summary: summary.into(),
            primary_action: primary_action.map(ToOwned::to_owned),
            details: Vec::new(),
            related_views: Vec::new(),
            related_commands: primary_action
                .map(|action| vec![action.to_string()])
                .unwrap_or_default(),
        }
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }

    fn related_view(mut self, view_id: &str) -> Self {
        push_unique(&mut self.related_views, view_id);
        self
    }

    fn related_command(mut self, command_id: &str) -> Self {
        push_unique(&mut self.related_commands, command_id);
        self
    }
}

impl PeerServer {
    fn start(home: VoxelleHome, bind: SocketAddr, advertise: Option<SocketAddr>) -> Result<Self> {
        let identity = home.load_identity()?;
        let certificate = home.load_certificate()?;
        let node = QuicNode::bind_with_certificate(identity, certificate, bind)?;
        let advertised_addr = advertise.unwrap_or(node.local_addr()?);
        let endpoint = node.peer_endpoint(advertised_addr)?;
        let local_report = node.local_reachability_report(advertised_addr)?;
        let config = home.load_config()?;
        Ok(Self {
            home,
            node,
            online: OnlineHome {
                endpoint,
                local_report,
                default_room: config.space.default_room_id,
                authority_peer_id: config.space.authority_peer_id,
                space_id: config.space.space_id,
                governance_room_id: config.space.governance_room_id,
            },
        })
    }

    async fn serve_next_request(&self) -> Result<ServedPeerRequest> {
        let store = self.home.open_store()?;
        let context = self.home.load_config()?.room_context();
        self.node
            .serve_peer_request_once(&store, &context, now_ms())
            .await
    }

    async fn stop(self) {
        self.node.close(b"runtime stopped");
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), self.node.wait_idle())
            .await;
    }
}

impl VoxelleService {
    pub fn start(
        home: VoxelleHome,
        bind: SocketAddr,
        advertise: Option<SocketAddr>,
        snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        let server = PeerServer::start(home, bind, advertise)?;
        let online = server.online.clone();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let (event_tx, events) = mpsc::sync_channel(SERVICE_EVENT_QUEUE_CAPACITY);
        let thread = thread::Builder::new()
            .name("voxelle-service".to_string())
            .spawn(move || run_service_thread(server, stop_rx, event_tx, snapshot_invalidated))
            .context("spawn voxelle service thread")?;

        Ok(Self {
            online,
            events,
            stop: Some(stop_tx),
            thread: Some(thread),
        })
    }

    pub fn online(&self) -> &OnlineHome {
        &self.online
    }

    pub fn try_recv_event(&self) -> Option<VoxelleServiceEvent> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => None,
        }
    }

    pub fn stop(mut self) -> Result<()> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("voxelle service thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for VoxelleService {
    fn drop(&mut self) {
        let _ = self.stop_inner();
    }
}

fn run_service_thread(
    server: PeerServer,
    stop_rx: tokio::sync::oneshot::Receiver<()>,
    event_tx: mpsc::SyncSender<VoxelleServiceEvent>,
    snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
) {
    let Ok(task_runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let _ = event_tx.try_send(VoxelleServiceEvent::Failed(
            "failed to create service runtime".to_string(),
        ));
        return;
    };
    task_runtime.block_on(run_service_loop(
        server,
        stop_rx,
        event_tx,
        snapshot_invalidated,
    ));
}

async fn run_service_loop(
    server: PeerServer,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    event_tx: mpsc::SyncSender<VoxelleServiceEvent>,
    snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            result = server.serve_next_request() => {
                match result {
                    Ok(served) => {
                        if event_tx.try_send(VoxelleServiceEvent::Served(Box::new(served))).is_ok() {
                            snapshot_invalidated();
                        }
                    }
                    Err(error) => {
                        if event_tx.try_send(VoxelleServiceEvent::Failed(format!("{error:#}"))).is_ok() {
                            snapshot_invalidated();
                        }
                    }
                }
            }
        }
    }
    server.stop().await;
    let _ = event_tx.try_send(VoxelleServiceEvent::Stopped);
    snapshot_invalidated();
}

fn default_ui_ontology(preferences: UiPreferences) -> UiOntologyView {
    apply_ui_preferences(
        UiOntologyView {
            places: default_places(),
            views: default_views(),
            commands: default_commands(),
            semantic_tokens: default_semantic_tokens(),
            metrics: default_metrics(),
            behaviors: default_behaviors(),
            renderers: default_renderers(),
        },
        preferences,
    )
}

fn apply_ui_preferences(
    mut ontology: UiOntologyView,
    preferences: UiPreferences,
) -> UiOntologyView {
    let semantic_tokens = &mut ontology.semantic_tokens;
    for token in semantic_tokens {
        if let Some(value) = preferences.semantic_tokens.get(&token.id) {
            token.current_value = value.clone();
        }
    }

    let metrics = &mut ontology.metrics;
    for metric in metrics {
        if let Some(value) = preferences.metrics.get(&metric.id) {
            metric.current_value = *value;
        }
    }

    let behaviors = &mut ontology.behaviors;
    for behavior in behaviors {
        if let Some(value) = preferences.behaviors.get(&behavior.id) {
            behavior.current_value = value.clone();
        }
    }

    let views = &mut ontology.views;
    for view in views {
        if let Some(placement) = preferences.view_placements.get(&view.id) {
            view.place_id = placement.place_id.clone();
            view.order = placement.order;
            view.visible = placement.visible;
        }
    }

    ontology
}

fn embedded_trusted_update_keys() -> Result<Vec<TrustedReleaseKey>> {
    let roots: TrustedReleaseKeysV1 =
        serde_json::from_str(include_str!("../../../release/trusted-update-keys.json"))
            .context("parse embedded update trust roots")?;
    if roots.v != 1 {
        anyhow::bail!(
            "unsupported embedded update trust roots version {}",
            roots.v
        );
    }
    Ok(roots.keys)
}

fn load_product_generation(
    manager: &UpdateManager,
) -> (Option<ActiveProductGeneration>, Option<String>) {
    let loaded = match manager.load_active() {
        Ok(Some(loaded)) => loaded,
        Ok(None) => return (None, None),
        Err(error) => {
            return (
                None,
                Some(format!(
                    "Signed product generation could not be recovered; using the built-in recovery generation: {error:#}"
                )),
            )
        }
    };
    let generation = match parse_product_generation(&loaded.package).and_then(|generation| {
        validate_product_generation(&generation)?;
        Ok(generation)
    }) {
        Ok(generation) => generation,
        Err(error) => {
            return (
                None,
                Some(format!(
                    "Active product generation is invalid; using the built-in recovery generation: {error:#}"
                )),
            )
        }
    };
    let notice = (loaded.source == ActiveSource::PreviousRecovery).then(|| {
        format!(
            "Recovered signed product generation {} after the active package failed verification.",
            loaded.package.package().release_id
        )
    });
    (
        Some(ActiveProductGeneration {
            pointer: loaded.package.pointer(),
            generation,
            source: loaded.source,
        }),
        notice,
    )
}

fn parse_product_generation(package: &VerifiedPackage) -> Result<ProductGenerationV1> {
    serde_json::from_value(package.package().payload.clone())
        .context("parse product generation payload")
}

fn validate_product_generation(generation: &ProductGenerationV1) -> Result<()> {
    if generation.v != 1 {
        anyhow::bail!("unsupported product generation version {}", generation.v);
    }
    if generation.component.api_version != 1 {
        anyhow::bail!(
            "unsupported product component API {}",
            generation.component.api_version
        );
    }
    if generation.component.source.is_empty()
        || generation.component.source.len() > 256 * 1024
        || generation.component.styles.is_empty()
        || generation.component.styles.len() > 256 * 1024
        || generation.component.source.chars().any(|character| character == '\0')
        || generation.component.styles.chars().any(|character| character == '\0')
    {
        anyhow::bail!("product component source is empty, oversized, or contains NUL");
    }
    let expected = default_ui_ontology(UiPreferences::default());
    validate_exact_ids(
        "place",
        &generation.ontology.places,
        &expected.places,
        |item| &item.id,
    )?;
    validate_exact_ids(
        "view",
        &generation.ontology.views,
        &expected.views,
        |item| &item.id,
    )?;
    validate_exact_ids(
        "command",
        &generation.ontology.commands,
        &expected.commands,
        |item| &item.id,
    )?;
    validate_exact_ids(
        "semantic token",
        &generation.ontology.semantic_tokens,
        &expected.semantic_tokens,
        |item| &item.id,
    )?;
    validate_exact_ids(
        "metric",
        &generation.ontology.metrics,
        &expected.metrics,
        |item| &item.id,
    )?;
    validate_exact_ids(
        "behavior",
        &generation.ontology.behaviors,
        &expected.behaviors,
        |item| &item.id,
    )?;
    validate_exact_ids(
        "renderer",
        &generation.ontology.renderers,
        &expected.renderers,
        |item| &item.id,
    )?;

    let known_places: BTreeSet<&str> = generation
        .ontology
        .places
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    for view in &generation.ontology.views {
        if !known_places.contains(view.default_place_id.as_str())
            || !known_places.contains(view.place_id.as_str())
        {
            anyhow::bail!("view {} references an unknown place", view.id);
        }
    }
    let expected_scopes: BTreeMap<_, _> = expected
        .commands
        .iter()
        .map(|command| (command.id.as_str(), command.scope))
        .collect();
    for command in &generation.ontology.commands {
        if expected_scopes.get(command.id.as_str()) != Some(&command.scope) {
            anyhow::bail!(
                "command {} changes its kernel-owned authority class",
                command.id
            );
        }
    }
    for metric in &generation.ontology.metrics {
        if !metric.default_value.is_finite()
            || !metric.current_value.is_finite()
            || metric.default_value.abs() > 1_000_000.0
            || metric.current_value.abs() > 1_000_000.0
        {
            anyhow::bail!(
                "metric {} is non-finite or outside product bounds",
                metric.id
            );
        }
    }
    if serde_json::to_vec(&generation.ontology)?.len() > 512 * 1024 {
        anyhow::bail!("product generation ontology is too large");
    }
    for value in ontology_strings(&generation.ontology) {
        if value.len() > 4096 || value.chars().any(|character| character == '\0') {
            anyhow::bail!("product generation contains an invalid or oversized string");
        }
    }
    Ok(())
}

fn validate_exact_ids<T, F>(label: &str, actual: &[T], expected: &[T], id: F) -> Result<()>
where
    F: Fn(&T) -> &String,
{
    let actual_ids: BTreeSet<&str> = actual.iter().map(|item| id(item).as_str()).collect();
    if actual_ids.len() != actual.len() {
        anyhow::bail!("product generation contains duplicate {label} ids");
    }
    let expected_ids: BTreeSet<&str> = expected.iter().map(|item| id(item).as_str()).collect();
    if actual_ids != expected_ids {
        anyhow::bail!("product generation {label} ids do not match the stable kernel inventory");
    }
    Ok(())
}

fn ontology_strings(ontology: &UiOntologyView) -> Vec<&str> {
    let mut values = Vec::new();
    for item in &ontology.places {
        values.extend([
            item.id.as_str(),
            item.label.as_str(),
            item.description.as_str(),
        ]);
    }
    for item in &ontology.views {
        values.extend([
            item.id.as_str(),
            item.label.as_str(),
            item.default_place_id.as_str(),
            item.place_id.as_str(),
            item.description.as_str(),
        ]);
    }
    for item in &ontology.commands {
        values.extend([
            item.id.as_str(),
            item.label.as_str(),
            item.description.as_str(),
        ]);
        if let Some(shortcut) = &item.shortcut {
            values.push(shortcut);
        }
    }
    values
}

fn default_places() -> Vec<UiPlace> {
    vec![
        ui_place(
            "sidebar",
            "Sidebar",
            "Navigation and secondary app surfaces",
        ),
        ui_place("main", "Main", "Primary room and message surfaces"),
        ui_place(
            "inspector",
            "Inspector",
            "Future selected peer or message details",
        ),
        ui_place(
            "activity",
            "Activity",
            "Service, diagnostic, and sync activity",
        ),
        ui_place("status", "Status", "Runtime and reachability state"),
    ]
}

fn default_views() -> Vec<UiView> {
    vec![
        ui_view(
            "profile.summary",
            "Profile Summary",
            "sidebar",
            0,
            "Local peer and device identity",
        ),
        ui_view(
            "runtime.status",
            "Runtime Status",
            "status",
            0,
            "Online/offline and reachability state",
        ),
        ui_view(
            "network.health",
            "Network Health",
            "status",
            1,
            "Re-entrant checklist for setup, reachability, and repair",
        ),
        ui_view(
            "field.test",
            "Field Test",
            "status",
            2,
            "Re-entrant end-to-end workflow checks",
        ),
        ui_view(
            "product.update",
            "Product Update",
            "status",
            3,
            "Signed live product generation status, activation, and rollback",
        ),
        ui_view(
            "invite.exchange",
            "Invite Exchange",
            "sidebar",
            1,
            "Copyable peer record and peer import",
        ),
        ui_view(
            "peer.list",
            "Peer List",
            "sidebar",
            2,
            "Known peers and peer actions",
        ),
        ui_view(
            "channel.list",
            "Channels",
            "sidebar",
            3,
            "Public and private conversation channels",
        ),
        ui_view(
            "member.profiles",
            "Members",
            "inspector",
            0,
            "Member profiles and presence identity",
        ),
        ui_view(
            "role.list",
            "Roles",
            "inspector",
            1,
            "Roles, permissions, and assignments",
        ),
        ui_view(
            "message.search",
            "Message Search",
            "inspector",
            2,
            "Local full-text message and attachment search",
        ),
        ui_view(
            "notification.center",
            "Notifications",
            "activity",
            1,
            "Unread mentions from replicated channels",
        ),
        ui_view(
            "room.timeline",
            "Room Timeline",
            "main",
            0,
            "Messages in the selected room",
        ),
        ui_view(
            "message.composer",
            "Message Composer",
            "main",
            1,
            "Message entry and send command",
        ),
        ui_view(
            "call.mesh",
            "Voice & Video",
            "main",
            2,
            "Direct WebRTC mesh for two to four room members",
        ),
        ui_view(
            "service.activity",
            "Service Activity",
            "activity",
            0,
            "Served requests, diagnostics, and sync events",
        ),
    ]
}

fn default_commands() -> Vec<UiCommand> {
    vec![
        shell_command(
            "shell.refresh",
            "Refresh",
            "Refresh the current shell snapshot",
            Some("Mod+R"),
            true,
        ),
        shell_command(
            "home.init",
            "Create My Space",
            "Create local identity and a private space",
            None,
            true,
        ),
        shell_command(
            "runtime.goOnline",
            "Go Online",
            "Start resident peer serving",
            None,
            true,
        ),
        shell_command(
            "runtime.goOffline",
            "Go Offline",
            "Stop resident peer serving",
            None,
            true,
        ),
        shell_command(
            "space.invite.create",
            "Create Space Invite",
            "Create a signed expiring invite for the current space",
            None,
            true,
        ),
        shell_command(
            "space.join",
            "Join Space",
            "Join from a signed invite and synchronize automatically",
            None,
            true,
        ),
        shell_command(
            "message.send",
            "Send Message",
            "Send a message to the current room",
            None,
            false,
        ),
        shell_command(
            "channel.select",
            "Select Channel",
            "Open an accessible channel",
            None,
            false,
        ),
        shell_command(
            "channel.markRead",
            "Mark Channel Read",
            "Advance the local read cursor for the selected channel",
            Some("Shift+Escape"),
            true,
        ),
        shell_command(
            "channel.rotateKey",
            "Rotate Private Channel Key",
            "Advance a private channel to a freshly wrapped key epoch",
            None,
            false,
        ),
        shell_command(
            "channel.create",
            "Create Channel",
            "Create a space channel",
            None,
            true,
        ),
        shell_command(
            "message.edit",
            "Edit Message",
            "Edit one of your messages",
            None,
            false,
        ),
        shell_command(
            "message.redact",
            "Delete Message",
            "Replace a message with a signed tombstone",
            None,
            false,
        ),
        shell_command(
            "reaction.add",
            "Add Reaction",
            "React to a message",
            None,
            false,
        ),
        shell_command(
            "reaction.remove",
            "Remove Reaction",
            "Remove your reaction",
            None,
            false,
        ),
        shell_command(
            "pin.add",
            "Pin Message",
            "Pin a message when authorized",
            None,
            false,
        ),
        shell_command(
            "pin.remove",
            "Unpin Message",
            "Remove a message pin when authorized",
            None,
            false,
        ),
        shell_command(
            "attachment.add",
            "Attach File",
            "Send a content-addressed attachment",
            None,
            false,
        ),
        shell_command(
            "profile.update",
            "Update Profile",
            "Set your shared display name and profile",
            None,
            true,
        ),
        shell_command(
            "role.create",
            "Create Role",
            "Create a role and its permissions",
            None,
            true,
        ),
        shell_command(
            "role.grant",
            "Grant Role",
            "Grant a role to a member",
            None,
            false,
        ),
        shell_command(
            "role.revoke",
            "Revoke Role",
            "Revoke a member role",
            None,
            false,
        ),
        shell_command(
            "member.ban",
            "Ban Member",
            "Ban a member from the space",
            None,
            false,
        ),
        shell_command(
            "member.unban",
            "Unban Member",
            "Remove a member ban",
            None,
            false,
        ),
        shell_command(
            "message.search",
            "Search Messages",
            "Search the local replicated message index",
            Some("Mod+F"),
            true,
        ),
        shell_command(
            "call.join",
            "Join Voice & Video",
            "Capture local media and join the selected room mesh",
            Some("Mod+Shift+V"),
            true,
        ),
        shell_command(
            "call.signal",
            "Send Call Signal",
            "Replicate an authenticated WebRTC negotiation signal",
            None,
            false,
        ),
        shell_command(
            "call.heartbeat",
            "Keep Call Alive",
            "Replicate short-lived room call presence",
            None,
            false,
        ),
        shell_command(
            "call.leave",
            "Leave Voice & Video",
            "Leave the selected room call and stop local capture",
            None,
            true,
        ),
        frontend_command(
            "message.composer.focus",
            "Focus Message Composer",
            "Move keyboard focus to the message composer",
            Some("Mod+K"),
            true,
        ),
        frontend_command(
            "invite.copy",
            "Copy Signed Invite",
            "Copy the current signed membership invite",
            None,
            true,
        ),
        shell_command(
            "peer.import",
            "Import Peer",
            "Import a peer availability record",
            None,
            true,
        ),
        shell_command(
            "peer.diagnose",
            "Diagnose Peer",
            "Check peer reachability",
            None,
            true,
        ),
        shell_command(
            "peer.sync",
            "Sync Peer",
            "Sync governance and room events with a peer",
            None,
            true,
        ),
        shell_command(
            "ui.preference.set",
            "Save Preference",
            "Persist a UI customization",
            None,
            false,
        ),
        shell_command(
            "workbench.layout.save",
            "Save Workbench Layout",
            "Persist view docking, order, and visibility",
            None,
            false,
        ),
        shell_command(
            "workbench.layout.reset",
            "Reset Workbench Layout",
            "Restore every view to its default dock",
            None,
            true,
        ),
        shell_command(
            "product.update.check",
            "Check GitHub Releases",
            "Discover the latest signed beta manifest without trusting GitHub as authority",
            None,
            true,
        ),
        shell_command(
            "product.update.stageAvailable",
            "Download and Stage Update",
            "Download, authenticate, validate, and stage the available generation",
            None,
            true,
        ),
        shell_command(
            "product.update.activateStaged",
            "Activate Staged Update",
            "Atomically activate the verified staged product generation",
            None,
            true,
        ),
        shell_command(
            "product.update.discardStaged",
            "Discard Staged Update",
            "Remove the staged generation without changing the active product",
            None,
            true,
        ),
        shell_command(
            "product.update.install",
            "Install Signed Product Update",
            "Verify, stage, and activate a signed product generation",
            None,
            true,
        ),
        shell_command(
            "product.update.rotateTrust",
            "Apply Signed Trust Transition",
            "Rotate release-signing authority through a transition signed by a currently trusted key",
            None,
            true,
        ),
        shell_command(
            "product.update.rollback",
            "Roll Back Product Update",
            "Reactivate the previous verified product generation",
            None,
            true,
        ),
        frontend_command(
            "workbench.commandPalette.open",
            "Show Command Palette",
            "Search and run commands",
            Some("Mod+Shift+P"),
            false,
        ),
    ]
}

pub fn shell_command_ids() -> Vec<String> {
    default_commands()
        .into_iter()
        .filter(|command| command.scope == UiCommandScope::Shell)
        .map(|command| command.id)
        .collect()
}

fn default_semantic_tokens() -> Vec<SemanticToken> {
    vec![
        semantic_token(
            "app.background",
            "App Background",
            "Canvas",
            "Canvas",
            &["profile.summary", "room.timeline"],
        ),
        semantic_token(
            "panel.background",
            "Panel Background",
            "Panel surface",
            "Canvas",
            &["peer.list", "invite.exchange", "service.activity"],
        ),
        semantic_token(
            "panel.border",
            "Panel Border",
            "Panel boundary",
            "ButtonBorder",
            &["sidebar", "inspector"],
        ),
        semantic_token(
            "text.primary",
            "Primary Text",
            "Primary readable text",
            "CanvasText",
            &["profile.summary", "room.timeline", "message.composer"],
        ),
        semantic_token(
            "text.secondary",
            "Secondary Text",
            "Secondary metadata text",
            "GrayText",
            &["peer.list", "service.activity"],
        ),
        semantic_token(
            "runtime.online",
            "Runtime Online",
            "Online runtime state",
            "#18794e",
            &["runtime.status"],
        ),
        semantic_token(
            "runtime.offline",
            "Runtime Offline",
            "Offline runtime state",
            "GrayText",
            &["runtime.status"],
        ),
        semantic_token(
            "peer.reachable",
            "Peer Reachable",
            "Reachable peer diagnostic",
            "#18794e",
            &["peer.list", "service.activity"],
        ),
        semantic_token(
            "peer.unreachable",
            "Peer Unreachable",
            "Unreachable peer diagnostic",
            "#b42318",
            &["peer.list", "service.activity"],
        ),
        semantic_token(
            "message.own.background",
            "Own Message Background",
            "Messages authored by this peer",
            "#e8f1ff",
            &["room.timeline"],
        ),
        semantic_token(
            "message.remote.background",
            "Remote Message Background",
            "Messages authored by other peers",
            "#f2f2f2",
            &["room.timeline"],
        ),
        semantic_token(
            "activity.info",
            "Activity Info",
            "Informational activity entries",
            "LinkText",
            &["service.activity"],
        ),
        semantic_token(
            "activity.error",
            "Activity Error",
            "Error activity entries",
            "#b42318",
            &["service.activity"],
        ),
    ]
}

fn default_metrics() -> Vec<UiMetric> {
    vec![
        metric("sidebar.width", "Sidebar Width", 360.0, "px", &["sidebar"]),
        metric(
            "panel.padding",
            "Panel Padding",
            12.0,
            "px",
            &["profile.summary", "peer.list", "invite.exchange"],
        ),
        metric("panel.gap", "Panel Gap", 8.0, "px", &["sidebar", "main"]),
        metric("message.gap", "Message Gap", 8.0, "px", &["room.timeline"]),
        metric(
            "message.maxWidth",
            "Message Max Width",
            720.0,
            "px",
            &["room.timeline"],
        ),
        metric(
            "avatar.size",
            "Avatar Size",
            32.0,
            "px",
            &["peer.list", "room.timeline"],
        ),
        metric(
            "activity.maxItems",
            "Activity Max Items",
            30.0,
            "count",
            &["service.activity"],
        ),
    ]
}

fn default_behaviors() -> Vec<UiBehavior> {
    vec![
        behavior(
            "timestamps.visible",
            "Show Timestamps",
            UiBehaviorValue::Bool(true),
            &["room.timeline"],
        ),
        behavior(
            "timestamps.style",
            "Timestamp Style",
            UiBehaviorValue::Text("relative".to_string()),
            &["room.timeline"],
        ),
        behavior(
            "activity.autoScroll",
            "Activity Auto Scroll",
            UiBehaviorValue::Bool(true),
            &["service.activity"],
        ),
        behavior(
            "peerList.compact",
            "Compact Peer List",
            UiBehaviorValue::Bool(false),
            &["peer.list"],
        ),
        behavior(
            "sync.autoAfterImport",
            "Sync After Import",
            UiBehaviorValue::Bool(false),
            &["invite.exchange", "peer.list"],
        ),
        behavior(
            "runtime.startOnlineOnLaunch",
            "Start Online On Launch",
            UiBehaviorValue::Bool(false),
            &["runtime.status"],
        ),
    ]
}

fn default_renderers() -> Vec<UiRenderer> {
    vec![
        renderer(
            "message.renderer",
            "Message Renderer",
            "message",
            "message.standard",
        ),
        renderer("peer.renderer", "Peer Renderer", "peer", "peer.standard"),
        renderer(
            "activity.renderer",
            "Activity Renderer",
            "activity",
            "activity.standard",
        ),
    ]
}

fn ui_place(id: &str, label: &str, description: &str) -> UiPlace {
    UiPlace {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        editable: true,
        editing_surface: "layout/place editor".to_string(),
    }
}

fn ui_view(id: &str, label: &str, place_id: &str, order: usize, description: &str) -> UiView {
    UiView {
        id: id.to_string(),
        label: label.to_string(),
        default_place_id: place_id.to_string(),
        place_id: place_id.to_string(),
        order,
        visible: true,
        description: description.to_string(),
        editable: true,
        editing_surface: "layout/place editor".to_string(),
    }
}

fn shell_command(
    id: &str,
    label: &str,
    description: &str,
    shortcut: Option<&str>,
    palette: bool,
) -> UiCommand {
    ui_command(
        id,
        label,
        description,
        UiCommandScope::Shell,
        shortcut,
        palette,
    )
}

fn frontend_command(
    id: &str,
    label: &str,
    description: &str,
    shortcut: Option<&str>,
    palette: bool,
) -> UiCommand {
    ui_command(
        id,
        label,
        description,
        UiCommandScope::Frontend,
        shortcut,
        palette,
    )
}

fn ui_command(
    id: &str,
    label: &str,
    description: &str,
    scope: UiCommandScope,
    shortcut: Option<&str>,
    palette: bool,
) -> UiCommand {
    UiCommand {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        scope,
        shortcut: shortcut.map(ToOwned::to_owned),
        palette,
        editable: false,
        editing_surface: "command palette".to_string(),
    }
}

fn semantic_token(
    id: &str,
    label: &str,
    description: &str,
    default_value: &str,
    used_by: &[&str],
) -> SemanticToken {
    SemanticToken {
        id: id.to_string(),
        label: label.to_string(),
        default_value: default_value.to_string(),
        current_value: default_value.to_string(),
        used_by: used_by.iter().map(|value| value.to_string()).collect(),
        editable: true,
        editing_surface: format!("appearance/token editor: {description}"),
    }
}

fn metric(id: &str, label: &str, default_value: f64, unit: &str, used_by: &[&str]) -> UiMetric {
    UiMetric {
        id: id.to_string(),
        label: label.to_string(),
        default_value,
        current_value: default_value,
        unit: unit.to_string(),
        used_by: used_by.iter().map(|value| value.to_string()).collect(),
        editable: true,
        editing_surface: "layout/place editor".to_string(),
    }
}

fn behavior(id: &str, label: &str, default_value: UiBehaviorValue, used_by: &[&str]) -> UiBehavior {
    UiBehavior {
        id: id.to_string(),
        label: label.to_string(),
        default_value: default_value.clone(),
        current_value: default_value,
        used_by: used_by.iter().map(|value| value.to_string()).collect(),
        editable: true,
        editing_surface: "behavior settings".to_string(),
    }
}

fn renderer(id: &str, label: &str, renders: &str, default_renderer: &str) -> UiRenderer {
    UiRenderer {
        id: id.to_string(),
        label: label.to_string(),
        renders: renders.to_string(),
        default_renderer: default_renderer.to_string(),
        current_renderer: default_renderer.to_string(),
        editable: false,
        editing_surface: "renderer settings".to_string(),
    }
}

fn validate_ui_preferences(preferences: &UiPreferences) -> Result<()> {
    for (id, value) in &preferences.semantic_tokens {
        validate_ui_preference_id(UiPreferenceKind::SemanticToken, id)?;
        if value.trim().is_empty() {
            anyhow::bail!("semantic token {id} value is empty");
        }
    }
    for (id, value) in &preferences.metrics {
        validate_ui_preference_id(UiPreferenceKind::Metric, id)?;
        if !value.is_finite() || *value < 0.0 {
            anyhow::bail!("UI metric {id} value must be a finite non-negative number");
        }
    }
    for (id, value) in &preferences.behaviors {
        let default = default_behaviors()
            .into_iter()
            .find(|behavior| behavior.id == *id)
            .with_context(|| format!("unknown UI behavior {id}"))?;
        if !same_behavior_value_kind(&default.default_value, value) {
            anyhow::bail!("UI behavior {id} value has the wrong kind");
        }
    }
    if !preferences.view_placements.is_empty() {
        for (id, placement) in &preferences.view_placements {
            if id != &placement.view_id {
                anyhow::bail!(
                    "workbench placement key does not match view {}",
                    placement.view_id
                );
            }
        }
        validate_workbench_layout(
            &preferences
                .view_placements
                .values()
                .cloned()
                .collect::<Vec<_>>(),
        )?;
    }
    Ok(())
}

fn validate_workbench_layout(placements: &[UiViewPlacement]) -> Result<()> {
    let views = default_views();
    let places = default_places();
    if placements.len() != views.len() {
        anyhow::bail!(
            "workbench layout must place every view exactly once (expected {}, got {})",
            views.len(),
            placements.len()
        );
    }

    let mut by_view = BTreeMap::new();
    let mut orders_by_place: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for placement in placements {
        if !views.iter().any(|view| view.id == placement.view_id) {
            anyhow::bail!("unknown workbench view {}", placement.view_id);
        }
        if !places.iter().any(|place| place.id == placement.place_id) {
            anyhow::bail!("unknown workbench place {}", placement.place_id);
        }
        if by_view.insert(&placement.view_id, ()).is_some() {
            anyhow::bail!(
                "workbench view {} is placed more than once",
                placement.view_id
            );
        }
        orders_by_place
            .entry(&placement.place_id)
            .or_default()
            .push(placement.order);
    }
    for (place, mut orders) in orders_by_place {
        orders.sort_unstable();
        if orders.iter().copied().ne(0..orders.len()) {
            anyhow::bail!("workbench place {place} order must be contiguous from zero");
        }
    }
    Ok(())
}

fn validate_ui_preference_id(kind: UiPreferenceKind, id: &str) -> Result<()> {
    let known = match kind {
        UiPreferenceKind::SemanticToken => default_semantic_tokens()
            .into_iter()
            .any(|token| token.id == id && token.editable),
        UiPreferenceKind::Metric => default_metrics()
            .into_iter()
            .any(|metric| metric.id == id && metric.editable),
        UiPreferenceKind::Behavior => default_behaviors()
            .into_iter()
            .any(|behavior| behavior.id == id && behavior.editable),
    };
    if known {
        Ok(())
    } else {
        anyhow::bail!("unknown or non-editable UI preference {id}")
    }
}

fn same_behavior_value_kind(left: &UiBehaviorValue, right: &UiBehaviorValue) -> bool {
    matches!(
        (left, right),
        (UiBehaviorValue::Bool(_), UiBehaviorValue::Bool(_))
            | (UiBehaviorValue::Text(_), UiBehaviorValue::Text(_))
    )
}

fn typescript_module(declarations: impl IntoIterator<Item = String>) -> String {
    let mut output =
        String::from("// This file is generated from Rust shell DTOs. Do not edit by hand.\n\n");
    for declaration in declarations {
        output.push_str("export ");
        output.push_str(&declaration);
        if !declaration.ends_with('\n') {
            output.push('\n');
        }
        output.push('\n');
    }
    output
}

fn advertised_address_row(report: &LocalReachabilityReport) -> NetworkHealthRow {
    let (status, summary, action) = match report.address_scope {
        AddressScope::Global => (
            NetworkHealthStatus::Working,
            format!("Advertising global IPv6 address {}.", report.advertised_addr),
            None,
        ),
        AddressScope::UniqueLocal => (
            NetworkHealthStatus::NeedsAttention,
            format!(
                "Advertising unique-local address {}; peers must be on the same private IPv6 network.",
                report.advertised_addr
            ),
            Some("runtime.goOnline"),
        ),
        AddressScope::LinkLocal => (
            NetworkHealthStatus::NeedsAttention,
            format!(
                "Advertising link-local address {}; this usually needs an interface scope and local-network peers.",
                report.advertised_addr
            ),
            Some("runtime.goOnline"),
        ),
        AddressScope::Loopback => (
            NetworkHealthStatus::NeedsAttention,
            format!(
                "Advertising loopback address {}; only this machine can connect.",
                report.advertised_addr
            ),
            Some("runtime.goOnline"),
        ),
        AddressScope::Unspecified => (
            NetworkHealthStatus::Broken,
            "Advertising an unspecified address; peers need a concrete IPv6 address.".to_string(),
            Some("runtime.goOnline"),
        ),
        AddressScope::Ipv4 => (
            NetworkHealthStatus::Broken,
            format!(
                "Advertising IPv4 address {}; Voxelle requires IPv6.",
                report.advertised_addr
            ),
            Some("runtime.goOnline"),
        ),
    };

    let mut row = NetworkHealthRow::new("advertise", "Advertise", status, summary, action);
    for note in &report.notes {
        row = row.detail(note);
    }
    if let Some(action) = action {
        row = row.related_command(action);
    }
    row
}

fn local_ipv6_socket_available() -> Result<()> {
    UdpSocket::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0))
        .context("bind local IPv6 UDP socket")?;
    Ok(())
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn identity_encryption_secret(identity: &PeerIdentity) -> Result<X25519Secret> {
    Ok(X25519Secret::from(identity.encryption_secret_bytes()))
}

fn identity_encryption_public_b64(identity: &PeerIdentity) -> Result<String> {
    Ok(identity.encryption_public_b64())
}

fn room_key_wrap_key(shared: &[u8; 32], room_id: &str, epoch: u64, peer_id: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"voxelle/room-key-wrap/v1\0");
    digest.update(shared);
    digest.update(room_id.as_bytes());
    digest.update(epoch.to_le_bytes());
    digest.update(peer_id.as_bytes());
    digest.finalize().into()
}

fn create_room_key_packages(
    room_id: &str,
    epoch: u64,
    room_key: &[u8; 32],
    members: &std::collections::BTreeSet<String>,
    encryption_keys: &BTreeMap<String, String>,
) -> Result<Vec<serde_json::Value>> {
    let mut packages = Vec::with_capacity(members.len());
    for peer_id in members {
        let public_bytes: [u8; 32] = base64::engine::general_purpose::STANDARD
            .decode(
                encryption_keys
                    .get(peer_id)
                    .ok_or_else(|| anyhow::anyhow!("member {peer_id} has no encryption key"))?,
            )
            .context("decode member encryption public key")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("member encryption public key must be 32 bytes"))?;
        let recipient_public = X25519PublicKey::from(public_bytes);
        let ephemeral_secret = X25519Secret::random_from_rng(rand::rngs::OsRng);
        let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);
        let shared = ephemeral_secret.diffie_hellman(&recipient_public);
        let wrap_key = room_key_wrap_key(shared.as_bytes(), room_id, epoch, peer_id);
        let cipher = XChaCha20Poly1305::new((&wrap_key).into());
        let mut nonce = [0_u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let aad = format!("voxelle/room-key-package/v1\n{room_id}\n{epoch}\n{peer_id}");
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: room_key,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("encrypt room key package"))?;
        packages.push(serde_json::json!({
            "peer_id": peer_id,
            "ephemeral_pub_b64": base64::engine::general_purpose::STANDARD.encode(ephemeral_public.as_bytes()),
            "nonce_b64": base64::engine::general_purpose::STANDARD.encode(nonce),
            "ciphertext_b64": base64::engine::general_purpose::STANDARD.encode(ciphertext),
        }));
    }
    Ok(packages)
}

fn unwrap_room_key_package(
    identity: &PeerIdentity,
    room_id: &str,
    epoch: u64,
    package: &serde_json::Value,
) -> Result<[u8; 32]> {
    let decode = |field: &str| -> Result<Vec<u8>> {
        base64::engine::general_purpose::STANDARD
            .decode(
                package
                    .get(field)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("room key package {field} missing"))?,
            )
            .with_context(|| format!("decode room key package {field}"))
    };
    let ephemeral_public: [u8; 32] = decode("ephemeral_pub_b64")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("ephemeral public key must be 32 bytes"))?;
    let nonce: [u8; 24] = decode("nonce_b64")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("room key package nonce must be 24 bytes"))?;
    let ciphertext = decode("ciphertext_b64")?;
    let secret = identity_encryption_secret(identity)?;
    let shared = secret.diffie_hellman(&X25519PublicKey::from(ephemeral_public));
    let wrap_key = room_key_wrap_key(shared.as_bytes(), room_id, epoch, &identity.peer_id);
    let cipher = XChaCha20Poly1305::new((&wrap_key).into());
    let aad = format!(
        "voxelle/room-key-package/v1\n{room_id}\n{epoch}\n{}",
        identity.peer_id
    );
    cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("room key package authentication failed"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("room key must be 32 bytes"))
}

fn private_event_aad(room_id: &str, epoch: u64, author_peer_id: &str) -> String {
    format!("voxelle/private-event/v1\n{room_id}\n{epoch}\n{author_peer_id}")
}

fn room_call_id(room_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"voxelle/room-call/v1\0");
    digest.update(room_id.as_bytes());
    format!(
        "call:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest.finalize())
    )
}

fn recovery_capsule_key(card: &RecoveryCardV1) -> Result<[u8; 32]> {
    if card.v != 1 {
        anyhow::bail!("unsupported recovery card version {}", card.v);
    }
    let secret = base64::engine::general_purpose::STANDARD
        .decode(&card.recovery_secret_b64)
        .context("decode recovery secret")?;
    if secret.len() != 32 {
        anyhow::bail!("recovery secret must be 32 bytes");
    }
    let mut digest = Sha256::new();
    digest.update(b"voxelle/recovery-capsule-key/v1\0");
    digest.update(secret);
    Ok(digest.finalize().into())
}

fn encrypt_recovery_capsule(
    card: &RecoveryCardV1,
    peer_id: &str,
    payload: &RecoveryPayloadV1,
) -> Result<RecoveryCapsuleV1> {
    let key = recovery_capsule_key(card)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let mut nonce = [0_u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let plaintext = serde_json::to_vec(payload).context("serialize recovery payload")?;
    let aad = format!("voxelle/recovery-capsule/v1\n{peer_id}");
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: &plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("encrypt recovery capsule"))?;
    Ok(RecoveryCapsuleV1 {
        v: 1,
        peer_id: peer_id.to_string(),
        nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce),
        ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    })
}

fn decrypt_recovery_capsule(
    card: &RecoveryCardV1,
    capsule: &RecoveryCapsuleV1,
) -> Result<RecoveryPayloadV1> {
    if capsule.v != 1 {
        anyhow::bail!("unsupported recovery capsule version {}", capsule.v);
    }
    let expected_peer_id = voxelle_core::principal_id(&card.genesis)?;
    if capsule.peer_id != expected_peer_id {
        anyhow::bail!("recovery capsule principal does not match recovery card");
    }
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(&capsule.nonce_b64)
        .context("decode recovery capsule nonce")?;
    let nonce: [u8; 24] = nonce
        .try_into()
        .map_err(|_| anyhow::anyhow!("recovery capsule nonce must be 24 bytes"))?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(&capsule.ciphertext_b64)
        .context("decode recovery capsule ciphertext")?;
    let key = recovery_capsule_key(card)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let aad = format!("voxelle/recovery-capsule/v1\n{}", capsule.peer_id);
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("recovery capsule authentication failed"))?;
    serde_json::from_slice(&plaintext).context("parse recovery payload")
}

pub fn write_identity_vault(path: &Path, identity: &PeerIdentity) -> Result<()> {
    let key = identity_vault_key(path, true)?;
    write_json(path, &IdentityFile::encrypt(identity, &key)?)
}

pub fn read_identity_vault(path: &Path) -> Result<PeerIdentity> {
    let file: IdentityFile = read_json(path)?;
    let key = identity_vault_key(path, false)?;
    file.decrypt(&key)
}

// The explicit returns keep mutually exclusive target/test cfg branches legible.
#[allow(clippy::needless_return)]
fn identity_vault_key(path: &Path, create: bool) -> Result<[u8; 32]> {
    #[cfg(test)]
    {
        return file_identity_vault_key(path, create);
    }

    #[cfg(not(test))]
    if cfg!(debug_assertions)
        && std::env::var("VOXELLE_VAULT_BACKEND").as_deref() == Ok("test-file")
    {
        return file_identity_vault_key(path, create);
    }

    #[cfg(all(not(test), any(target_os = "macos", target_os = "windows")))]
    {
        return os_identity_vault_key(path, create);
    }

    #[cfg(all(not(test), not(any(target_os = "macos", target_os = "windows"))))]
    {
        file_identity_vault_key(path, create)
    }
}

#[cfg(all(not(test), any(target_os = "macos", target_os = "windows")))]
fn os_identity_vault_key(path: &Path, create: bool) -> Result<[u8; 32]> {
    let account_path = if path.exists() {
        path.canonicalize()
            .with_context(|| format!("resolve {}", path.display()))?
    } else {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        parent
            .canonicalize()
            .with_context(|| format!("resolve {}", parent.display()))?
            .join(path.file_name().unwrap_or_default())
    };
    let mut account_digest = Sha256::new();
    account_digest.update(b"voxelle/identity-vault-account/v1\0");
    account_digest.update(account_path.to_string_lossy().as_bytes());
    let account =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(account_digest.finalize());
    let entry = keyring::Entry::new("app.voxelle.identity-vault", &account)
        .context("open operating-system identity vault entry")?;
    match entry.get_secret() {
        Ok(secret) => decode_identity_vault_key(&secret),
        Err(keyring::Error::NoEntry) if create => {
            let mut key = [0_u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            entry
                .set_secret(&key)
                .context("store identity unlock key in operating-system credential store")?;
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            anyhow::bail!(
                "identity unlock key is missing from the operating-system credential store"
            )
        }
        Err(error) => {
            Err(error).context("read identity unlock key from operating-system credential store")
        }
    }
}

fn file_identity_vault_key(path: &Path, create: bool) -> Result<[u8; 32]> {
    let key_path = path.with_extension("test-unlock-key");
    if key_path.exists() {
        return decode_identity_vault_key(
            &fs::read(&key_path).with_context(|| format!("read {}", key_path.display()))?,
        );
    }
    if !create {
        anyhow::bail!("identity test unlock key is missing");
    }
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut key = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&key_path)
        .with_context(|| format!("create {}", key_path.display()))?;
    file.write_all(&key)
        .with_context(|| format!("write {}", key_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("protect {}", key_path.display()))?;
    }
    Ok(key)
}

fn decode_identity_vault_key(secret: &[u8]) -> Result<[u8; 32]> {
    secret
        .try_into()
        .map_err(|_| anyhow::anyhow!("identity unlock key must be 32 bytes"))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    write_new_private_json(path, value)
}

fn write_secret_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    write_new_private_json(path, value)
}

fn write_new_private_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} has no parent directory", path.display()))?;
    ensure_real_dir(parent)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    file.write_all((serde_json::to_string_pretty(value)? + "\n").as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("protect {}", path.display()))?;
    }
    Ok(())
}

fn ensure_private_dir(path: &Path) -> Result<()> {
    ensure_real_dir(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("protect {}", path.display()))?;
    }
    Ok(())
}

fn ensure_real_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    let existed = path.exists();
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!("{} must be a real directory", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !existed {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("protect {}", path.display()))?;
        }
    }
    Ok(())
}

fn short_peer_label(peer_id: &str) -> String {
    peer_id
        .strip_prefix("p:")
        .or_else(|| peer_id.strip_prefix("ed25519:"))
        .and_then(|rest| rest.get(..12))
        .map(|short| format!("Peer {short}"))
        .unwrap_or_else(|| "Peer".to_string())
}

fn retain_latest<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        items.drain(..items.len() - limit);
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv6Addr};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    #[test]
    fn visible_projection_retains_only_the_latest_bounded_items() {
        let mut items = vec![1, 2, 3, 4];
        retain_latest(&mut items, 2);
        assert_eq!(items, vec![3, 4]);
    }

    #[cfg(unix)]
    #[test]
    fn private_json_creation_is_owner_only_and_refuses_existing_targets() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let dir = tempdir().expect("tempdir");
        let private_dir = dir.path().join("private");
        fs::create_dir(&private_dir).expect("private dir");
        fs::set_permissions(&private_dir, fs::Permissions::from_mode(0o755))
            .expect("make permissive");
        let secret = private_dir.join("recovery.voxrecover");
        write_secret_json(&secret, &serde_json::json!({"secret": true})).expect("write secret");
        assert_eq!(
            fs::metadata(&private_dir)
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(&secret)
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(write_secret_json(&secret, &serde_json::json!({"secret": false})).is_err());

        let victim = dir.path().join("victim");
        fs::write(&victim, "preserve").expect("victim");
        let link = private_dir.join("linked.voxrecover");
        symlink(&victim, &link).expect("symlink");
        assert!(write_secret_json(&link, &serde_json::json!({"secret": true})).is_err());
        assert_eq!(
            fs::read_to_string(victim).expect("victim unchanged"),
            "preserve"
        );
    }

    #[test]
    fn home_root_resolution_preserves_override_precedence_and_portable_default() {
        let explicit = PathBuf::from("explicit-home");
        let configured = PathBuf::from("configured-home");
        let platform_home = PathBuf::from("platform-home");

        assert_eq!(
            resolve_home_root_from(
                Some(explicit.clone()),
                Some(configured.clone()),
                Some(platform_home.clone()),
            ),
            explicit
        );
        assert_eq!(
            resolve_home_root_from(None, Some(configured.clone()), Some(platform_home.clone())),
            configured
        );
        assert_eq!(
            resolve_home_root_from(None, None, Some(platform_home.clone())),
            platform_home.join(".voxelle")
        );
        assert_eq!(
            resolve_home_root_from(None, None, None),
            PathBuf::from(".").join(".voxelle")
        );
    }

    #[test]
    fn home_init_send_and_read_are_app_actions() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));

        let profile = home.init(DEFAULT_ROOM_ID).expect("init");
        assert!(profile.default_room.ends_with(":channel:general"));
        assert_eq!(profile.peer_id, profile.authority_peer_id);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&home.root)
                    .expect("home metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(home.path("quic-cert.json"))
                    .expect("certificate metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let event = home
            .send_message("hello from app layer", None)
            .expect("send");
        assert_eq!(event.kind, "MSG_POST");

        let messages = home.read_messages(None).expect("read");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "hello from app layer");
    }

    #[test]
    fn redaction_projects_even_when_the_target_has_a_later_wall_clock_time() {
        let identity = PeerIdentity::generate().expect("identity");
        let post = create_event(
            &identity,
            create_delegation(&identity, 0, i64::MAX, vec!["room:post".to_string()])
                .expect("delegation"),
            "room:test",
            8_640_000_000_000_000,
            "MSG_POST",
            vec![],
            serde_json::json!({"text":"future","mentions":[]}),
        )
        .expect("post");
        let redact = create_event(
            &identity,
            create_delegation(&identity, 0, i64::MAX, vec!["room:post".to_string()])
                .expect("delegation"),
            "room:test",
            1_000,
            "MSG_REDACT",
            vec![post.event_id.clone()],
            serde_json::json!({"target_event_id":post.event_id}),
        )
        .expect("redact");
        let projected = project_messages(vec![post, redact]);
        assert_eq!(projected.len(), 1);
        assert!(projected[0].redacted);
        assert_eq!(projected[0].text, "Message removed");
    }

    #[test]
    fn home_init_is_idempotent_and_preserves_identity() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));

        let first = home.init(DEFAULT_ROOM_ID).expect("first init");
        let second = home.init("room:ignored").expect("second init");

        assert_eq!(first.peer_id, second.peer_id);
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(second.default_room, first.default_room);
    }

    #[test]
    fn home_identity_secrets_are_only_persisted_in_authenticated_ciphertext() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));
        home.init(DEFAULT_ROOM_ID).expect("init");
        let identity = home.load_identity().expect("identity");
        let raw = fs::read_to_string(home.path("identity.json")).expect("vault file");

        assert!(raw.contains("ciphertext_b64"));
        assert!(!raw.contains("root_secret_b64"));
        assert!(!raw.contains("device_secret_b64"));
        assert!(!raw.contains("recovery_secret_b64"));
        assert!(!raw.contains(&identity.peer.secret_key_b64()));
        assert!(!raw.contains(&identity.device.secret_key_b64()));
        assert!(!raw.contains(&identity.recovery.secret_key_b64()));
    }

    #[test]
    fn home_consolidates_local_state_without_rolling_recovery_cache() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));
        home.init(DEFAULT_ROOM_ID).expect("init");
        home.set_ui_preference(SetUiPreferenceRequest::Metric {
            id: "sidebar.width".to_string(),
            value: 420.0,
        })
        .expect("persist preference");

        for retired in [
            "config.json",
            "known-peers.json",
            "read-state.json",
            "room-keys.json",
            "ui-preferences.json",
            "recovery-capsule.json",
        ] {
            assert!(!home.path(retired).exists(), "retired state file {retired}");
        }
        assert!(home.path("identity.json").exists());
        assert!(home.path("quic-cert.json").exists());
        assert!(home.path("store.sqlite3").exists());
        let selection: HomeSelectionV1 = home
            .local_state(HOME_SELECTION_STATE)
            .expect("load home selection")
            .expect("home selection");
        assert!(home
            .open_store()
            .expect("store")
            .get_event(&selection.space_genesis_event_id)
            .expect("load selected genesis")
            .is_some());
        assert!(home
            .local_state::<serde_json::Value>("home.config")
            .expect("old config state")
            .is_none());

        let reopened = VoxelleHome::new(home.root.clone());
        assert_eq!(
            reopened.profile_summary().expect("profile").peer_id,
            home.profile_summary().expect("original profile").peer_id
        );
        assert_eq!(
            reopened
                .ui_preferences()
                .expect("preferences")
                .metrics
                .get("sidebar.width"),
            Some(&420.0)
        );
        assert!(!reopened
            .recovery_kit()
            .expect("on-demand recovery kit")
            .capsule
            .ciphertext_b64
            .is_empty());
    }

    #[test]
    fn recovery_capsule_is_authenticated_and_bound_to_its_card() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));
        home.init(DEFAULT_ROOM_ID).expect("init");
        let kit = home.recovery_kit().expect("recovery kit");
        let kit_path = dir.path().join("alice.voxrecover");
        home.write_recovery_kit(&kit_path).expect("write kit");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&kit_path)
                    .expect("kit metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let payload = decrypt_recovery_capsule(&kit.card, &kit.capsule).expect("decrypt");
        assert_eq!(payload.space.authority_peer_id, kit.capsule.peer_id);

        let mut tampered = kit.clone();
        tampered.capsule.ciphertext_b64.push('A');
        assert!(decrypt_recovery_capsule(&tampered.card, &tampered.capsule).is_err());

        let other = VoxelleHome::new(dir.path().join("other"));
        other.init(DEFAULT_ROOM_ID).expect("other init");
        let wrong_card = other.recovery_kit().expect("other kit").card;
        assert!(decrypt_recovery_capsule(&wrong_card, &kit.capsule).is_err());
    }

    #[tokio::test]
    async fn fresh_home_recovery_resyncs_history_and_revokes_the_lost_device() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice-lost"));
        let bob = VoxelleHome::new(dir.path().join("bob-router"));
        let recovered = VoxelleHome::new(dir.path().join("alice-recovered"));
        let alice_profile = alice.init(DEFAULT_ROOM_ID).expect("alice init");
        alice.send_message("before loss", None).expect("message");

        let alice_service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("alice service");
        let invite = alice
            .create_space_invite(alice_service.online(), now_ms() + 60_000)
            .expect("invite bob");
        let initial_replication = bob
            .join_space_from_invite(&invite, 64)
            .await
            .expect("bob joins and stores alice history");
        assert_eq!(initial_replication.peers_reached, 1);
        assert!(initial_replication.events_received >= 2);
        alice_service.stop().expect("alice offline");

        let bob_service = bob
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("bob service");
        let bob_router = bob_service
            .online()
            .peer_record(Some("Bob recovery peer".to_string()), None)
            .expect("bob record");
        alice
            .import_peer_record(bob_router)
            .expect("save recovery peer");
        alice.mark_read(None).expect("persist read cursor");
        let lost_read_state = alice.read_state().expect("lost read state");
        let kit = alice.recovery_kit().expect("recovery kit");
        let old_device_id = alice_profile.device_id;

        let report = recovered
            .recover_from_kit(&kit, 64)
            .await
            .expect("recover fresh home");
        assert_eq!(report.profile.peer_id, alice_profile.peer_id);
        assert_ne!(report.profile.device_id, old_device_id);
        assert_eq!(report.peers_reached, 1);
        assert!(report.events_recovered >= 2);
        assert!(report.events_pushed >= 1, "{report:?}");
        assert_eq!(
            recovered.read_messages(None).expect("recovered messages")[0].text,
            "before loss"
        );
        assert_eq!(
            recovered.read_state().expect("recovered read state"),
            lost_read_state
        );
        bob_service.stop().expect("bob service stop");

        alice
            .send_message("from lost device", None)
            .expect("partitioned lost device can still sign locally");
        let lost_service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("lost service");
        let lost_record = lost_service
            .online()
            .peer_record(Some("Lost Alice".to_string()), None)
            .expect("lost record");
        let rejected = bob
            .sync_peer(&lost_record, 64)
            .await
            .expect("sync reports rejection");
        assert_eq!(rejected.room.rejected, 1);
        assert!(bob
            .read_messages(None)
            .expect("bob messages")
            .iter()
            .all(|message| message.text != "from lost device"));
        lost_service.stop().expect("lost service stop");
    }

    #[tokio::test]
    async fn signed_invite_onboards_a_fresh_home_and_pushes_membership_and_messages() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob-fresh"));
        let alice_profile = alice.init(DEFAULT_ROOM_ID).expect("alice init");
        alice
            .send_message("welcome history", None)
            .expect("history");
        let service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("alice service");
        let invite = alice
            .create_space_invite(service.online(), now_ms() + 60_000)
            .expect("signed invite");

        invite.validate_at(now_ms()).expect("invite validates");
        let joined = bob
            .join_space_from_invite(&invite, 64)
            .await
            .expect("join from invite");
        let bootstrap = invite.bootstrap_peers().expect("bootstrap").remove(0);
        assert_eq!(joined.profile.authority_peer_id, alice_profile.peer_id);
        assert_eq!(joined.peers_reached, 1);
        assert!(joined.events_pushed >= 1, "{joined:?}");
        assert!(bob
            .read_messages(None)
            .expect("bob history")
            .iter()
            .any(|message| message.text == "welcome history"));

        bob.send_message("hello after one-action join", None)
            .expect("bob message");
        let pushed = bob.sync_peer(&bootstrap, 64).await.expect("push message");
        assert!(pushed.room.remote_accepted >= 1);
        assert!(alice
            .read_messages(None)
            .expect("alice messages")
            .iter()
            .any(|message| message.text == "hello after one-action join"));

        let mut tampered = invite.clone();
        tampered.space.name = "Mallory's Space".to_string();
        assert!(tampered.validate_at(now_ms()).is_err());
        service.stop().expect("service stop");
    }

    #[tokio::test]
    async fn signed_invite_onboards_through_an_ordinary_peer_while_inviter_is_offline() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice-authority"));
        let bob = VoxelleHome::new(dir.path().join("bob-router"));
        let charlie = VoxelleHome::new(dir.path().join("charlie-fresh"));
        alice.init(DEFAULT_ROOM_ID).expect("alice init");
        alice
            .send_message("history via Bob", None)
            .expect("history");

        let alice_service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("alice service");
        let bob_invite = alice
            .create_space_invite(alice_service.online(), now_ms() + 60_000)
            .expect("invite bob");
        bob.join_space_from_invite(&bob_invite, 64)
            .await
            .expect("bob joins");
        let bob_service = bob
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("bob service");
        let bob_record = bob_service
            .online()
            .peer_record(Some("Bob ordinary peer".to_string()), None)
            .expect("bob record");
        let charlie_invite = alice
            .create_space_invite_with_bootstraps(
                alice_service.online(),
                std::slice::from_ref(&bob_record),
                now_ms() + 60_000,
            )
            .expect("invite with ordinary peer");
        alice_service.stop().expect("inviter offline");

        let joined = charlie
            .join_space_from_invite(&charlie_invite, 64)
            .await
            .expect("join through Bob");
        assert_eq!(joined.peers_attempted, 2);
        assert_eq!(joined.peers_reached, 1);
        assert!(joined.events_pushed >= 1, "{joined:?}");
        assert!(joined
            .peer_errors
            .iter()
            .any(|error| error.contains("Inviter")));
        assert!(charlie
            .read_messages(None)
            .expect("charlie history")
            .iter()
            .any(|message| message.text == "history via Bob"));

        charlie
            .send_message("Charlie through Bob", None)
            .expect("charlie message");
        let pushed = charlie
            .sync_peer(&bob_record, 64)
            .await
            .expect("push to Bob");
        assert!(pushed.room.remote_accepted >= 1, "{pushed:?}");

        let pulled = alice
            .sync_peer(&bob_record, 64)
            .await
            .expect("Alice catches up");
        assert!(pulled.governance.accepted >= 1, "{pulled:?}");
        assert!(pulled.room.accepted >= 1, "{pulled:?}");
        assert!(alice
            .read_messages(None)
            .expect("alice messages")
            .iter()
            .any(|message| message.text == "Charlie through Bob"));
        bob_service.stop().expect("bob stop");
    }

    #[tokio::test]
    async fn private_room_is_ciphertext_for_members_only_and_survives_recovery() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob"));
        let charlie = VoxelleHome::new(dir.path().join("charlie"));
        let recovered = VoxelleHome::new(dir.path().join("alice-recovered"));
        let alice_profile = alice.init(DEFAULT_ROOM_ID).expect("alice init");
        let alice_service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("alice service");
        let alice_record = alice_service
            .online()
            .peer_record(Some("Alice".to_string()), None)
            .expect("alice record");

        let bob_invite = alice
            .create_space_invite(alice_service.online(), now_ms() + 60_000)
            .expect("bob invite");
        let bob_joined = bob
            .join_space_from_invite(&bob_invite, 4096)
            .await
            .expect("bob joins");
        let bob_peer_id = bob_joined.profile.peer_id;
        let charlie_invite = alice
            .create_space_invite(alice_service.online(), now_ms() + 60_000)
            .expect("charlie invite");
        charlie
            .join_space_from_invite(&charlie_invite, 4096)
            .await
            .expect("charlie joins");

        let channel_event = alice
            .create_channel(&CreateChannelRequest {
                name: "Alice and Bob".to_string(),
                topic: "Direct conversation".to_string(),
                private_members: vec![bob_peer_id.clone()],
            })
            .expect("private channel");
        let room_id = channel_event
            .body
            .get("room_id")
            .and_then(serde_json::Value::as_str)
            .expect("room id")
            .to_string();
        let sent = alice
            .send_message("e2e secret phrase", Some(&room_id))
            .expect("private send");
        assert_eq!(sent.kind, "ROOM_ENCRYPTED");
        assert!(!serde_json::to_string(&sent)
            .expect("event json")
            .contains("e2e secret phrase"));
        let raw_room = alice
            .open_store()
            .expect("alice store")
            .room_events(&room_id)
            .expect("raw room");
        assert_eq!(raw_room.len(), 1);
        assert_eq!(raw_room[0].kind, "ROOM_ENCRYPTED");
        assert!(!alice.path("room-keys.json").exists());
        let encrypted_keys: EncryptedRoomKeysFile = alice
            .local_state(ROOM_KEYS_STATE)
            .expect("load encrypted room keys")
            .expect("encrypted room keys");
        assert!(!serde_json::to_string(&encrypted_keys)
            .expect("serialize encrypted room keys")
            .contains("e2e secret phrase"));

        bob.sync_peer(&alice_record, 4096).await.expect("bob sync");
        assert_eq!(
            bob.read_messages(Some(&room_id)).expect("bob decrypts")[0].text,
            "e2e secret phrase"
        );
        alice
            .rotate_channel_key(&RotateChannelKeyRequest {
                room_id: room_id.clone(),
            })
            .expect("rotate key epoch");
        alice
            .send_message("secret after rotation", Some(&room_id))
            .expect("send under new epoch");
        bob.sync_peer(&alice_record, 4096)
            .await
            .expect("bob syncs rotation");
        let bob_messages = bob
            .read_messages(Some(&room_id))
            .expect("both epochs decrypt");
        assert_eq!(bob_messages.len(), 2);
        assert_eq!(bob_messages[1].text, "secret after rotation");
        charlie
            .sync_peer(&alice_record, 4096)
            .await
            .expect("charlie governance sync");
        assert!(charlie
            .channels(None)
            .expect("charlie channels")
            .iter()
            .all(|channel| channel.room_id != room_id));
        assert!(charlie
            .open_store()
            .expect("charlie store")
            .room_events(&room_id)
            .expect("charlie raw room")
            .is_empty());

        let bob_service = bob
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("bob service");
        alice
            .import_peer_record(
                bob_service
                    .online()
                    .peer_record(Some("Bob recovery peer".to_string()), None)
                    .expect("bob record"),
            )
            .expect("save recovery peer");
        let kit = alice.recovery_kit().expect("kit with current room key");
        alice_service.stop().expect("alice offline");
        let report = recovered
            .recover_from_kit(&kit, 4096)
            .await
            .expect("fresh recovery");
        assert_eq!(report.profile.peer_id, alice_profile.peer_id);
        assert_eq!(
            recovered
                .read_messages(Some(&room_id))
                .expect("recovered decrypts")[1]
                .text,
            "secret after rotation"
        );
        bob_service.stop().expect("bob offline");
    }

    #[test]
    fn ui_ontology_exposes_first_customization_primitives() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.init(DEFAULT_ROOM_ID).expect("init");

        let ontology = home.ui_ontology().expect("ontology");

        assert!(ontology.places.iter().any(|place| place.id == "sidebar"));
        assert!(ontology.views.iter().any(|view| view.id == "room.timeline"));
        assert!(ontology
            .commands
            .iter()
            .any(|command| command.id == "peer.sync"));
        assert_eq!(semantic_token_value(&ontology, "peer.reachable"), "#18794e");
        assert_eq!(metric_value(&ontology, "sidebar.width"), 360.0);
        assert_eq!(
            behavior_value(&ontology, "timestamps.visible"),
            UiBehaviorValue::Bool(true)
        );
        assert!(ontology
            .renderers
            .iter()
            .any(|renderer| renderer.id == "message.renderer"));
    }

    #[test]
    fn ui_preferences_persist_merge_and_reset() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.init(DEFAULT_ROOM_ID).expect("init");

        home.set_ui_preference(SetUiPreferenceRequest::SemanticToken {
            id: "peer.reachable".to_string(),
            value: "#00ff00".to_string(),
        })
        .expect("set token");
        home.set_ui_preference(SetUiPreferenceRequest::Metric {
            id: "sidebar.width".to_string(),
            value: 420.0,
        })
        .expect("set metric");
        home.set_ui_preference(SetUiPreferenceRequest::Behavior {
            id: "timestamps.style".to_string(),
            value: UiBehaviorValue::Text("absolute".to_string()),
        })
        .expect("set behavior");
        let mut placements: Vec<UiViewPlacement> = home
            .ui_ontology()
            .expect("ontology")
            .views
            .into_iter()
            .map(|view| UiViewPlacement {
                view_id: view.id,
                place_id: view.place_id,
                order: view.order,
                visible: view.visible,
            })
            .collect();
        for placement in placements
            .iter_mut()
            .filter(|placement| placement.place_id == "inspector")
        {
            placement.order += 1;
        }
        let profile = placements
            .iter_mut()
            .find(|placement| placement.view_id == "profile.summary")
            .expect("profile placement");
        profile.place_id = "inspector".to_string();
        profile.order = 0;
        for placement in placements
            .iter_mut()
            .filter(|placement| placement.place_id == "sidebar")
        {
            placement.order -= 1;
        }
        placements
            .iter_mut()
            .find(|placement| placement.view_id == "field.test")
            .expect("field placement")
            .visible = false;
        home.set_workbench_layout(SetWorkbenchLayoutRequest { placements })
            .expect("set layout");

        let reopened = VoxelleHome::new(dir.path().join("home"));
        let preferences = reopened.ui_preferences().expect("preferences");
        assert_eq!(
            preferences.semantic_tokens.get("peer.reachable"),
            Some(&"#00ff00".to_string())
        );
        assert_eq!(preferences.metrics.get("sidebar.width"), Some(&420.0));
        assert_eq!(
            preferences.behaviors.get("timestamps.style"),
            Some(&UiBehaviorValue::Text("absolute".to_string()))
        );
        assert_eq!(
            preferences
                .view_placements
                .get("profile.summary")
                .expect("saved profile")
                .place_id,
            "inspector"
        );

        let ontology = reopened.ui_ontology().expect("ontology");
        assert_eq!(semantic_token_value(&ontology, "peer.reachable"), "#00ff00");
        assert_eq!(metric_value(&ontology, "sidebar.width"), 420.0);
        assert_eq!(
            behavior_value(&ontology, "timestamps.style"),
            UiBehaviorValue::Text("absolute".to_string())
        );
        assert_eq!(
            ontology
                .views
                .iter()
                .find(|view| view.id == "profile.summary")
                .expect("profile")
                .place_id,
            "inspector"
        );
        assert!(
            !ontology
                .views
                .iter()
                .find(|view| view.id == "field.test")
                .expect("field test")
                .visible
        );

        reopened
            .reset_ui_preference(UiPreferenceKind::Metric, "sidebar.width")
            .expect("reset metric");
        let reset = reopened.ui_ontology().expect("reset ontology");
        assert_eq!(metric_value(&reset, "sidebar.width"), 360.0);
        assert_eq!(semantic_token_value(&reset, "peer.reachable"), "#00ff00");

        reopened
            .reset_all_ui_preferences()
            .expect("reset all preferences");
        let defaults = reopened.ui_ontology().expect("default ontology");
        assert_eq!(semantic_token_value(&defaults, "peer.reachable"), "#18794e");
        assert_eq!(
            behavior_value(&defaults, "timestamps.style"),
            UiBehaviorValue::Text("relative".to_string())
        );
        assert_eq!(
            defaults
                .views
                .iter()
                .find(|view| view.id == "profile.summary")
                .expect("profile")
                .place_id,
            "sidebar"
        );
    }

    #[test]
    fn ui_preferences_reject_unknown_ids_and_wrong_behavior_kind() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.init(DEFAULT_ROOM_ID).expect("init");

        assert!(home
            .set_ui_preference(SetUiPreferenceRequest::SemanticToken {
                id: "unknown.token".to_string(),
                value: "#fff".to_string(),
            })
            .is_err());
        assert!(home
            .set_ui_preference(SetUiPreferenceRequest::Metric {
                id: "sidebar.width".to_string(),
                value: -1.0,
            })
            .is_err());
        assert!(home
            .set_ui_preference(SetUiPreferenceRequest::Behavior {
                id: "timestamps.visible".to_string(),
                value: UiBehaviorValue::Text("yes".to_string()),
            })
            .is_err());
    }

    #[test]
    fn command_host_snapshot_is_safe_before_home_init() {
        let dir = tempdir().expect("tempdir");
        let mut host = VoxelleCommandHost::new(dir.path().join("home"));

        let snapshot = host.snapshot().expect("snapshot");

        assert_eq!(snapshot.home_root, dir.path().join("home"));
        assert!(snapshot.home.is_none());
        assert!(snapshot.home_error.is_some());
        assert_eq!(
            network_health_status(&snapshot.network_health, "home"),
            NetworkHealthStatus::NeedsAttention
        );
        assert!(snapshot
            .ui_ontology
            .views
            .iter()
            .any(|view| view.id == "network.health"));
    }

    #[tokio::test]
    async fn command_host_drives_tauri_style_network_workflow() {
        let dir = tempdir().expect("tempdir");
        let invalidations = Arc::new(AtomicUsize::new(0));
        let observed_invalidations = invalidations.clone();
        let mut alice = VoxelleCommandHost::new_with_notifier(
            dir.path().join("alice"),
            Arc::new(move || {
                observed_invalidations.fetch_add(1, Ordering::SeqCst);
            }),
        );
        let mut bob = VoxelleCommandHost::new(dir.path().join("bob"));

        alice
            .init_home(InitHomeRequest { default_room: None })
            .expect("alice init");
        alice
            .send_message(SendMessageRequest {
                text: "from command host".to_string(),
                room: None,
                mentions: Vec::new(),
                thread_root_event_id: None,
            })
            .await
            .expect("send");

        let alice_online = alice
            .start_service(StartServiceRequest {
                bind: None,
                advertise: None,
            })
            .expect("alice online");
        assert_eq!(
            network_health_status(&alice_online.network_health, "service"),
            NetworkHealthStatus::Working
        );
        let invite_snapshot = alice
            .create_space_invite(CreateSpaceInviteRequest {
                expires_minutes: Some(60),
            })
            .expect("create invite");
        let space_invite_json = invite_snapshot
            .home
            .as_ref()
            .expect("home view")
            .invite
            .as_ref()
            .expect("invite")
            .space_invite_json
            .as_ref()
            .expect("signed invite")
            .clone();

        let bob_imported = bob
            .join_space(JoinSpaceRequest {
                space_invite_json,
                max_events: Some(64),
            })
            .await
            .expect("join");
        assert_eq!(
            network_health_status(&bob_imported.network_health, "peers"),
            NetworkHealthStatus::Working
        );
        let peer = &bob_imported.home.as_ref().expect("home view").peers[0];
        let request = PeerCommandRequest {
            peer_id: peer.peer_id.clone(),
            device_id: peer.device_id.clone(),
            max_events: Some(64),
        };

        let diagnosed = bob.diagnose_peer(request.clone()).await.expect("diagnose");
        assert!(diagnosed
            .service_activity
            .iter()
            .any(|item| item.summary.starts_with("diagnostic reached")));

        assert_eq!(
            bob_imported.home.expect("home").room.messages[0].text,
            "from command host"
        );

        let alice_after_serving = alice.snapshot().expect("alice snapshot");
        assert!(invalidations.load(Ordering::SeqCst) > 0);
        assert!(alice_after_serving
            .service_activity
            .iter()
            .any(|item| item.summary.starts_with("served sync:")));
        alice.stop_service().expect("stop");
    }

    #[test]
    fn network_health_view_handles_uninitialized_home() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));

        let health = home.network_health_view(None).expect("health");

        assert_eq!(
            network_health_status(&health, "home"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_status(&health, "identity"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_status(&health, "certificate"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_status(&health, "ipv6"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "service"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_row(&health, "home")
                .primary_action
                .as_deref(),
            Some("home.init")
        );
    }

    #[test]
    fn customizing_before_initialization_keeps_home_recoverable() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.set_ui_preference(SetUiPreferenceRequest::Metric {
            id: "sidebar.width".to_string(),
            value: 512.0,
        })
        .expect("save preference");

        let health = home.network_health_view(None).expect("health");

        assert_eq!(
            network_health_status(&health, "home"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_row(&health, "home")
                .primary_action
                .as_deref(),
            Some("home.init")
        );
    }

    #[test]
    fn network_health_view_shapes_initialized_offline_state() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.init(DEFAULT_ROOM_ID).expect("init");

        let health = home.network_health_view(None).expect("health");

        assert_eq!(
            network_health_status(&health, "home"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "identity"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "certificate"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "service"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_status(&health, "bind"),
            NetworkHealthStatus::Unknown
        );
        assert_eq!(
            network_health_status(&health, "advertise"),
            NetworkHealthStatus::Unknown
        );
        assert_eq!(
            network_health_status(&health, "invite"),
            NetworkHealthStatus::Unknown
        );
        assert_eq!(
            network_health_status(&health, "peers"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_row(&health, "service")
                .related_commands
                .as_slice(),
            &["runtime.goOnline".to_string()]
        );
    }

    #[tokio::test]
    async fn network_health_view_shapes_online_service_state() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.init(DEFAULT_ROOM_ID).expect("init");
        let service = home
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("service");

        let health = home
            .network_health_view(Some(service.online()))
            .expect("health");

        assert_eq!(
            network_health_status(&health, "service"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "bind"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "advertise"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_status(&health, "invite"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_row(&health, "invite")
                .primary_action
                .as_deref(),
            Some("invite.copy")
        );
        service.stop().expect("stop service");
    }

    #[tokio::test]
    async fn network_health_view_tracks_known_peer_prerequisites() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        let peer = VoxelleHome::new(dir.path().join("peer"));
        home.init(DEFAULT_ROOM_ID).expect("home init");
        peer.init(DEFAULT_ROOM_ID).expect("peer init");
        let peer_service = peer
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("peer service");
        let peer_record = peer_service
            .online()
            .peer_record(Some("Peer".to_string()), None)
            .expect("peer record");
        home.import_peer_record(peer_record).expect("import");

        let health = home.network_health_view(None).expect("health");

        assert_eq!(
            network_health_status(&health, "peers"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            network_health_status(&health, "reachability"),
            NetworkHealthStatus::NeedsAttention
        );
        assert_eq!(
            network_health_row(&health, "reachability")
                .primary_action
                .as_deref(),
            Some("peer.diagnose")
        );
        assert_eq!(
            network_health_row(&health, "sync")
                .primary_action
                .as_deref(),
            Some("peer.sync")
        );
        peer_service.stop().expect("stop peer service");
    }

    #[tokio::test]
    async fn peer_record_export_import_drives_diagnostics_and_sync() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob"));

        alice.init(DEFAULT_ROOM_ID).expect("alice init");
        bob.init(DEFAULT_ROOM_ID).expect("bob init");
        alice
            .send_message("from imported peer", None)
            .expect("send");

        let service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("service");
        let alice_record = service
            .online()
            .peer_record(Some("Alice".to_string()), None)
            .expect("peer record");
        bob.import_peer_record(alice_record.clone())
            .expect("import peer");

        let known = bob.known_peers().expect("known peers");
        assert_eq!(known, vec![alice_record.clone()]);

        let mut renamed_record = alice_record.clone();
        renamed_record.label = Some("Alice renamed".to_string());
        bob.import_peer_record(renamed_record.clone())
            .expect("update peer");
        assert_eq!(
            bob.known_peers().expect("updated peers"),
            vec![renamed_record]
        );

        let diagnostic = bob.diagnose_peer(&alice_record).await.expect("diagnose");
        assert!(diagnostic.reachable);

        let sync_error = bob
            .sync_peer(&alice_record, 64)
            .await
            .expect_err("an endpoint record is not a membership capability");
        assert!(sync_error.to_string().contains("active home authority"));
        service.stop().expect("stop service");
    }

    #[tokio::test]
    async fn service_keeps_home_online_for_diagnostics_and_sync() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob"));

        alice.init(DEFAULT_ROOM_ID).expect("alice init");
        alice
            .send_message("first service message", None)
            .expect("send");

        let service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("service");
        let invite = alice
            .create_space_invite(service.online(), now_ms() + 60_000)
            .expect("invite");
        let record = invite.bootstrap_peers().expect("bootstrap").remove(0);

        let joined = bob.join_space_from_invite(&invite, 64).await.expect("join");
        assert!(joined.events_received >= 1);
        let diagnostic = bob.diagnose_peer(&record).await.expect("diagnose");
        assert!(diagnostic.reachable);

        alice
            .send_message("second service message", None)
            .expect("send second");
        let second = bob.sync_peer(&record, 64).await.expect("second sync");
        assert_eq!(second.governance.offered, 0);
        assert_eq!(second.room.accepted, 1);

        let messages = bob.read_messages(None).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "first service message");
        assert_eq!(messages[1].text, "second service message");

        let Some(event) = service.try_recv_event() else {
            panic!("expected service event");
        };
        assert!(matches!(event, VoxelleServiceEvent::Served(_)));
        assert!(event.summary().starts_with("served "));
        service.stop().expect("stop service");
    }

    #[tokio::test]
    async fn malformed_peer_request_does_not_terminate_the_service() {
        let dir = tempdir().expect("tempdir");
        let server_home = VoxelleHome::new(dir.path().join("server"));
        let client_home = VoxelleHome::new(dir.path().join("client"));
        server_home.init(DEFAULT_ROOM_ID).expect("server init");
        client_home.init(DEFAULT_ROOM_ID).expect("client init");
        let service = server_home
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("service");
        let endpoint = service.online().endpoint.clone();

        let client_node = QuicNode::bind_ipv6_loopback_with_certificate(
            client_home.load_identity().expect("identity"),
            client_home.load_certificate().expect("certificate"),
        )
        .expect("client node");
        let authenticated = client_node
            .connect(
                endpoint.addr,
                endpoint.certificate_der().expect("server cert"),
                &endpoint.peer_id,
                &endpoint.device_id,
            )
            .await
            .expect("connect");
        let (mut send, _recv) = authenticated.connection.open_bi().await.expect("stream");
        send.write_all(b"{}")
            .await
            .expect("malformed request bytes");
        send.finish().expect("finish malformed request");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let diagnostic = client_node.diagnose_peer(&endpoint).await;
        assert!(diagnostic.reachable);
        service.stop().expect("stop service");
    }

    #[tokio::test]
    async fn home_screen_view_shapes_first_gui_state() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        let peer_home = VoxelleHome::new(dir.path().join("peer"));

        home.init(DEFAULT_ROOM_ID).expect("home init");
        peer_home.init(DEFAULT_ROOM_ID).expect("peer init");
        home.send_message("visible message", None).expect("send");

        let offline = home.home_screen_view(None).expect("offline view");
        assert_eq!(offline.runtime.state, RuntimeState::Offline);
        assert!(offline.invite.is_none());
        assert!(offline.peers.is_empty());
        assert_eq!(offline.room.room_id, offline.profile.default_room);
        assert_eq!(offline.room.messages[0].text, "visible message");

        let service = home
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("service");
        let peer_service = peer_home
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("peer service");
        let peer_record = peer_service
            .online()
            .peer_record(Some("Peer One".to_string()), None)
            .expect("peer record");
        home.import_peer_record(peer_record.clone())
            .expect("import peer");

        let online = home
            .home_screen_view(Some(service.online()))
            .expect("online view");
        assert_eq!(online.runtime.state, RuntimeState::Online);
        assert!(online.runtime.listen_addr.is_some());
        assert!(online.invite.is_some());
        assert!(online
            .invite
            .as_ref()
            .expect("invite")
            .peer_record_json
            .contains(":channel:general"));
        assert_eq!(online.peers.len(), 1);
        assert_eq!(online.peers[0].label, "Peer One");
        assert_eq!(online.peers[0].peer_id, peer_record.endpoint.peer_id);
        service.stop().expect("stop service");
        peer_service.stop().expect("stop peer service");
    }

    fn semantic_token_value(ontology: &UiOntologyView, id: &str) -> String {
        ontology
            .semantic_tokens
            .iter()
            .find(|token| token.id == id)
            .unwrap_or_else(|| panic!("missing semantic token {id}"))
            .current_value
            .clone()
    }

    fn metric_value(ontology: &UiOntologyView, id: &str) -> f64 {
        ontology
            .metrics
            .iter()
            .find(|metric| metric.id == id)
            .unwrap_or_else(|| panic!("missing metric {id}"))
            .current_value
    }

    fn behavior_value(ontology: &UiOntologyView, id: &str) -> UiBehaviorValue {
        ontology
            .behaviors
            .iter()
            .find(|behavior| behavior.id == id)
            .unwrap_or_else(|| panic!("missing behavior {id}"))
            .current_value
            .clone()
    }

    fn network_health_row<'a>(health: &'a NetworkHealthView, id: &str) -> &'a NetworkHealthRow {
        health
            .rows
            .iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("missing network health row {id}"))
    }

    fn network_health_status(health: &NetworkHealthView, id: &str) -> NetworkHealthStatus {
        network_health_row(health, id).status
    }
}
