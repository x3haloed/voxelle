use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use voxelle_core::{
    accept_event, create_delegation, create_event, EventV1, PeerIdentity, RoomContext,
    GOVERNANCE_ROOM_ID,
};
use voxelle_net::{
    AddressScope, LocalReachabilityReport, PeerEndpoint, PeerReachabilityReport, QuicCertificate,
    QuicNode, ServedPeerRequest, ServedRoomSync,
};
use voxelle_store::Store;
use voxelle_sync::{SyncLimits, SyncStats};

pub const DEFAULT_ROOM_ID: &str = "room:general";

#[derive(Debug, Clone)]
pub struct VoxelleHome {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeConfig {
    pub v: u8,
    pub default_room: String,
    pub authority_peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityFile {
    pub v: u8,
    pub peer_secret_b64: String,
    pub device_secret_b64: String,
    pub peer_id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummary {
    pub home: PathBuf,
    pub peer_id: String,
    pub device_id: String,
    pub default_room: String,
    pub authority_peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageView {
    pub event_id: String,
    pub created_ms: i64,
    pub author_peer_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerRecord {
    pub v: u8,
    pub label: Option<String>,
    pub default_room: String,
    pub endpoint: PeerEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct KnownPeersFile {
    v: u8,
    peers: Vec<PeerRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiOntologyView {
    pub places: Vec<UiPlace>,
    pub views: Vec<UiView>,
    pub commands: Vec<UiCommand>,
    pub semantic_tokens: Vec<SemanticToken>,
    pub metrics: Vec<UiMetric>,
    pub behaviors: Vec<UiBehavior>,
    pub renderers: Vec<UiRenderer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPlace {
    pub id: String,
    pub label: String,
    pub description: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiView {
    pub id: String,
    pub label: String,
    pub place_id: String,
    pub description: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiCommand {
    pub id: String,
    pub label: String,
    pub description: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticToken {
    pub id: String,
    pub label: String,
    pub default_value: String,
    pub current_value: String,
    pub used_by: Vec<String>,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiBehavior {
    pub id: String,
    pub label: String,
    pub default_value: UiBehaviorValue,
    pub current_value: UiBehaviorValue,
    pub used_by: Vec<String>,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiRenderer {
    pub id: String,
    pub label: String,
    pub renders: String,
    pub default_renderer: String,
    pub current_renderer: String,
    pub editable: bool,
    pub editing_surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            v: 1,
            semantic_tokens: BTreeMap::new(),
            metrics: BTreeMap::new(),
            behaviors: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct VoxelleRuntime {
    home: VoxelleHome,
    node: QuicNode,
    endpoint: PeerEndpoint,
    local_report: LocalReachabilityReport,
}

#[derive(Debug)]
pub struct VoxelleService {
    summary: ListenSummary,
    default_room: String,
    events: mpsc::Receiver<VoxelleServiceEvent>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
pub struct VoxelleCommandHost {
    home: VoxelleHome,
    service: Option<VoxelleService>,
    activity: Vec<ServiceActivityItem>,
    next_activity_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelleServiceEvent {
    Served(ServedPeerRequest),
    Failed(String),
    Stopped,
}

impl VoxelleServiceEvent {
    pub fn summary(&self) -> String {
        match self {
            VoxelleServiceEvent::Served(ServedPeerRequest::Diagnostic(report)) => {
                if report.reachable {
                    let remote = report
                        .remote
                        .as_ref()
                        .map(|remote| short_peer_label(&remote.peer_id))
                        .unwrap_or_else(|| "peer".to_string());
                    format!("served diagnostic: {remote} reached this home")
                } else {
                    format!(
                        "served diagnostic: unreachable ({})",
                        report.error.as_deref().unwrap_or("no error detail")
                    )
                }
            }
            VoxelleServiceEvent::Served(ServedPeerRequest::RoomSync(sync)) => {
                let truncated = if sync.truncated { ", truncated" } else { "" };
                format!(
                    "served sync: room {}, offered {} event(s){}",
                    sync.room_id, sync.offered, truncated
                )
            }
            VoxelleServiceEvent::Failed(error) => format!("service error: {error}"),
            VoxelleServiceEvent::Stopped => "service stopped".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListenSummary {
    pub endpoint: PeerEndpoint,
    pub local_report: LocalReachabilityReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSyncReport {
    pub governance: SyncStats,
    pub room: SyncStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellSnapshotView {
    pub home_root: PathBuf,
    pub home: Option<HomeScreenView>,
    pub home_error: Option<String>,
    pub network_health: NetworkHealthView,
    pub ui_ontology: UiOntologyView,
    pub service_activity: Vec<ServiceActivityItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceActivityItem {
    pub id: u64,
    pub level: ServiceActivityLevel,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceActivityLevel {
    Info,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitHomeRequest {
    pub default_room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartServiceRequest {
    pub bind: Option<SocketAddr>,
    pub advertise: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendMessageRequest {
    pub text: String,
    pub room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportPeerRecordRequest {
    pub peer_record_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerCommandRequest {
    pub peer_id: String,
    pub device_id: String,
    pub max_events: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeScreenView {
    pub profile: ProfileSummary,
    pub runtime: RuntimeStatusView,
    pub invite: Option<InviteExchangeView>,
    pub peers: Vec<PeerListItemView>,
    pub room: RoomTimelineView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkHealthView {
    pub rows: Vec<NetworkHealthRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkHealthStatus {
    Unknown,
    Working,
    NeedsAttention,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatusView {
    pub state: RuntimeState,
    pub listen_addr: Option<SocketAddr>,
    pub advertised_addr: Option<SocketAddr>,
    pub reachability_notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Offline,
    Online,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InviteExchangeView {
    pub peer_record: PeerRecord,
    pub peer_record_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerListItemView {
    pub label: String,
    pub peer_id: String,
    pub device_id: String,
    pub addr: SocketAddr,
    pub default_room: String,
    pub diagnostic_state: PeerActionState,
    pub sync_state: PeerActionState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerActionState {
    NotRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomTimelineView {
    pub room_id: String,
    pub messages: Vec<MessageView>,
}

impl VoxelleHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn identity_path(&self) -> PathBuf {
        self.root.join("identity.json")
    }

    pub fn certificate_path(&self) -> PathBuf {
        self.root.join("quic-cert.json")
    }

    pub fn store_path(&self) -> PathBuf {
        self.root.join("store.sqlite3")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn known_peers_path(&self) -> PathBuf {
        self.root.join("known-peers.json")
    }

    pub fn ui_preferences_path(&self) -> PathBuf {
        self.root.join("ui-preferences.json")
    }

    pub fn init(&self, default_room: impl Into<String>) -> Result<ProfileSummary> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create {}", self.root.display()))?;
        let identity = self.load_or_create_identity()?;
        self.load_or_create_certificate()?;

        let default_room = default_room.into();
        let config = if self.config_path().exists() {
            self.load_config()?
        } else {
            let config = HomeConfig {
                v: 1,
                default_room,
                authority_peer_id: identity.peer.id.clone(),
            };
            write_json(&self.config_path(), &config)?;
            config
        };

        let store = self.open_store()?;
        self.ensure_member_join(&store, &identity, &config)?;

        Ok(ProfileSummary {
            home: self.root.clone(),
            peer_id: identity.peer.id,
            device_id: identity.device.id,
            default_room: config.default_room,
            authority_peer_id: config.authority_peer_id,
        })
    }

    pub fn profile_summary(&self) -> Result<ProfileSummary> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        Ok(ProfileSummary {
            home: self.root.clone(),
            peer_id: identity.peer.id,
            device_id: identity.device.id,
            default_room: config.default_room,
            authority_peer_id: config.authority_peer_id,
        })
    }

    pub fn home_screen_view(&self, runtime: Option<&VoxelleRuntime>) -> Result<HomeScreenView> {
        let config = self.load_config()?;
        let invite = runtime
            .map(|runtime| runtime.invite_view(None, None))
            .transpose()?;
        let runtime = runtime
            .map(RuntimeStatusView::online)
            .unwrap_or_else(RuntimeStatusView::offline);
        Ok(HomeScreenView {
            profile: self.profile_summary()?,
            runtime,
            invite,
            peers: self
                .known_peers()?
                .into_iter()
                .map(PeerListItemView::from_peer_record)
                .collect(),
            room: RoomTimelineView {
                room_id: config.default_room,
                messages: self.read_messages(None)?,
            },
        })
    }

    pub fn home_screen_view_service(
        &self,
        service: Option<&VoxelleService>,
    ) -> Result<HomeScreenView> {
        let config = self.load_config()?;
        let invite = service
            .map(|service| service.invite_view(None, None))
            .transpose()?;
        let runtime = service
            .map(RuntimeStatusView::from_service)
            .unwrap_or_else(RuntimeStatusView::offline);
        Ok(HomeScreenView {
            profile: self.profile_summary()?,
            runtime,
            invite,
            peers: self
                .known_peers()?
                .into_iter()
                .map(PeerListItemView::from_peer_record)
                .collect(),
            room: RoomTimelineView {
                room_id: config.default_room,
                messages: self.read_messages(None)?,
            },
        })
    }

    pub fn network_health_view(
        &self,
        service: Option<&VoxelleService>,
    ) -> Result<NetworkHealthView> {
        let home_status = match self.load_config() {
            Ok(config) => NetworkHealthRow::working(
                "home",
                "Home",
                format!("Home is initialized for {}.", config.default_room),
            )
            .detail(format!("root: {}", self.root.display())),
            Err(error) if self.root.exists() => NetworkHealthRow::broken(
                "home",
                "Home",
                "Home exists but cannot be read.",
                Some("home.init"),
            )
            .detail(format!("{error:#}"))
            .related_command("home.init"),
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
                    short_peer_label(&identity.peer.id)
                ),
            )
            .detail(format!("device: {}", short_peer_label(&identity.device.id))),
            Err(error) if self.identity_path().exists() => NetworkHealthRow::broken(
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
            Err(error) if self.certificate_path().exists() => NetworkHealthRow::broken(
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

        let service_status = match service {
            Some(service) => NetworkHealthRow::working(
                "service",
                "Service",
                format!("Resident service is online at {}.", service.endpoint().addr),
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

        let bind_status = match service {
            Some(service) if service.local_report().listen_addr.is_ipv6() => {
                NetworkHealthRow::working(
                    "bind",
                    "Bind",
                    format!("Listening on {}.", service.local_report().listen_addr),
                )
                .related_command("runtime.goOffline")
            }
            Some(service) => NetworkHealthRow::broken(
                "bind",
                "Bind",
                format!(
                    "Listener is not IPv6: {}.",
                    service.local_report().listen_addr
                ),
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

        let advertise_status = match service {
            Some(service) => advertised_address_row(service.local_report()),
            None => NetworkHealthRow::unknown(
                "advertise",
                "Advertise",
                "No advertised address until the service is online.",
                Some("runtime.goOnline"),
            )
            .related_command("runtime.goOnline"),
        }
        .related_view("runtime.status");

        let invite_status = match service {
            Some(service) => match service.invite_view(None, None) {
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
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let room = room.unwrap_or(&config.default_room);
        let context = RoomContext::new(config.authority_peer_id);
        let governance = store.room_events(GOVERNANCE_ROOM_ID)?;
        let event = create_event(
            &identity,
            create_delegation(
                &identity.peer,
                &identity.device,
                now_ms() - 60_000,
                now_ms() + 30 * 24 * 60 * 60_000,
                vec!["room:post".to_string()],
            )?,
            room,
            now_ms(),
            "MSG_POST",
            store.room_heads(room)?,
            serde_json::json!({ "text": text }),
        )?;
        let accepted = accept_event(&event, &governance, &context, now_ms())
            .map_err(|e| anyhow::anyhow!("message rejected: {e:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(event)
    }

    pub fn read_messages(&self, room: Option<&str>) -> Result<Vec<MessageView>> {
        let config = self.load_config()?;
        let store = self.open_store()?;
        let room = room.unwrap_or(&config.default_room);
        let mut messages = Vec::new();
        for event in store.room_events(room)? {
            if event.kind != "MSG_POST" {
                continue;
            }
            let text = event
                .body
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            messages.push(MessageView {
                event_id: event.event_id,
                created_ms: event.created_ms,
                author_peer_id: event.author_peer_id,
                text,
            });
        }
        Ok(messages)
    }

    pub fn export_endpoint(&self, advertised_addr: SocketAddr) -> Result<PeerEndpoint> {
        if !advertised_addr.is_ipv6() {
            anyhow::bail!("advertised address must be IPv6");
        }
        let identity = self.load_identity()?;
        let certificate = self.load_certificate()?;
        Ok(PeerEndpoint {
            v: 1,
            addr: advertised_addr,
            peer_id: identity.peer.id,
            device_id: identity.device.id,
            quic_cert_der_b64: certificate.cert_der_b64,
            quic_cert_fingerprint: certificate.fingerprint,
        })
    }

    pub fn export_peer_record(
        &self,
        advertised_addr: SocketAddr,
        label: Option<String>,
        room: Option<&str>,
    ) -> Result<PeerRecord> {
        let config = self.load_config()?;
        let default_room = room.unwrap_or(&config.default_room).to_string();
        let record = PeerRecord {
            v: 1,
            label,
            default_room,
            endpoint: self.export_endpoint(advertised_addr)?,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn import_peer_record(&self, record: PeerRecord) -> Result<()> {
        record.validate()?;
        let mut peers = self.known_peers()?;
        if let Some(existing) = peers.iter_mut().find(|peer| peer.same_peer(&record)) {
            *existing = record;
        } else {
            peers.push(record);
        }
        peers.sort_by(|a, b| {
            a.label
                .cmp(&b.label)
                .then_with(|| a.endpoint.peer_id.cmp(&b.endpoint.peer_id))
                .then_with(|| a.endpoint.device_id.cmp(&b.endpoint.device_id))
        });
        write_json(&self.known_peers_path(), &KnownPeersFile { v: 1, peers })
    }

    pub fn known_peers(&self) -> Result<Vec<PeerRecord>> {
        if !self.known_peers_path().exists() {
            return Ok(Vec::new());
        }
        let file: KnownPeersFile = read_json(&self.known_peers_path())?;
        if file.v != 1 {
            anyhow::bail!("unsupported known peers version {}", file.v);
        }
        for record in &file.peers {
            record.validate()?;
        }
        Ok(file.peers)
    }

    pub fn ui_ontology(&self) -> Result<UiOntologyView> {
        Ok(default_ui_ontology(self.ui_preferences()?))
    }

    pub fn ui_preferences(&self) -> Result<UiPreferences> {
        if !self.ui_preferences_path().exists() {
            return Ok(UiPreferences::default());
        }
        let preferences: UiPreferences = read_json(&self.ui_preferences_path())?;
        if preferences.v != 1 {
            anyhow::bail!("unsupported UI preferences version {}", preferences.v);
        }
        validate_ui_preferences(&preferences)?;
        Ok(preferences)
    }

    pub fn set_semantic_token(&self, id: &str, value: impl Into<String>) -> Result<()> {
        let default = default_semantic_tokens()
            .into_iter()
            .find(|token| token.id == id)
            .with_context(|| format!("unknown semantic token {id}"))?;
        if !default.editable {
            anyhow::bail!("semantic token {id} is not editable");
        }
        let value = value.into();
        if value.trim().is_empty() {
            anyhow::bail!("semantic token value is empty");
        }
        let mut preferences = self.ui_preferences()?;
        preferences.semantic_tokens.insert(id.to_string(), value);
        self.write_ui_preferences(&preferences)
    }

    pub fn set_metric(&self, id: &str, value: f64) -> Result<()> {
        let default = default_metrics()
            .into_iter()
            .find(|metric| metric.id == id)
            .with_context(|| format!("unknown UI metric {id}"))?;
        if !default.editable {
            anyhow::bail!("UI metric {id} is not editable");
        }
        if !value.is_finite() || value < 0.0 {
            anyhow::bail!("UI metric value must be a finite non-negative number");
        }
        let mut preferences = self.ui_preferences()?;
        preferences.metrics.insert(id.to_string(), value);
        self.write_ui_preferences(&preferences)
    }

    pub fn set_behavior(&self, id: &str, value: UiBehaviorValue) -> Result<()> {
        let default = default_behaviors()
            .into_iter()
            .find(|behavior| behavior.id == id)
            .with_context(|| format!("unknown UI behavior {id}"))?;
        if !default.editable {
            anyhow::bail!("UI behavior {id} is not editable");
        }
        if !same_behavior_value_kind(&default.default_value, &value) {
            anyhow::bail!("UI behavior {id} value has the wrong kind");
        }
        let mut preferences = self.ui_preferences()?;
        preferences.behaviors.insert(id.to_string(), value);
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

    pub fn listen(
        &self,
        bind: SocketAddr,
        advertise: Option<SocketAddr>,
    ) -> Result<VoxelleRuntime> {
        VoxelleRuntime::start(self.clone(), bind, advertise)
    }

    pub fn start_service(
        &self,
        bind: SocketAddr,
        advertise: Option<SocketAddr>,
    ) -> Result<VoxelleService> {
        VoxelleService::start(self.clone(), bind, advertise)
    }

    pub async fn diagnose_peer(&self, peer: &PeerRecord) -> Result<PeerReachabilityReport> {
        peer.validate()?;
        self.diagnose_endpoint(&peer.endpoint).await
    }

    pub async fn diagnose_endpoint(
        &self,
        endpoint: &PeerEndpoint,
    ) -> Result<PeerReachabilityReport> {
        let identity = self.load_identity()?;
        let certificate = self.load_certificate()?;
        let node = QuicNode::bind_ipv6_loopback_with_certificate(identity, certificate)?;
        Ok(node.diagnose_peer(endpoint).await)
    }

    pub async fn sync_peer(&self, peer: &PeerRecord, max_events: usize) -> Result<PeerSyncReport> {
        peer.validate()?;
        self.sync_endpoint(&peer.endpoint, Some(&peer.default_room), max_events)
            .await
    }

    pub async fn sync_endpoint(
        &self,
        endpoint: &PeerEndpoint,
        room: Option<&str>,
        max_events: usize,
    ) -> Result<PeerSyncReport> {
        endpoint.validate()?;
        if max_events == 0 {
            anyhow::bail!("max_events must be positive");
        }

        let identity = self.load_identity()?;
        let certificate = self.load_certificate()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let node = QuicNode::bind_ipv6_loopback_with_certificate(identity, certificate)?;
        let context = RoomContext::new(endpoint.peer_id.clone());
        let limits = SyncLimits {
            max_events_per_batch: max_events,
        };
        let governance = node
            .sync_room_once(
                &store,
                endpoint.addr,
                endpoint.certificate_der()?,
                &endpoint.device_id,
                GOVERNANCE_ROOM_ID,
                &context,
                now_ms(),
                limits,
            )
            .await?;
        let room_id = room.unwrap_or(&config.default_room);
        let room = node
            .sync_room_once(
                &store,
                endpoint.addr,
                endpoint.certificate_der()?,
                &endpoint.device_id,
                room_id,
                &context,
                now_ms(),
                limits,
            )
            .await?;

        Ok(PeerSyncReport { governance, room })
    }

    pub fn load_identity(&self) -> Result<PeerIdentity> {
        let file: IdentityFile = read_json(&self.identity_path())?;
        if file.v != 1 {
            anyhow::bail!("unsupported identity version {}", file.v);
        }
        PeerIdentity::from_secret_keys_b64(&file.peer_secret_b64, &file.device_secret_b64)
    }

    pub fn load_certificate(&self) -> Result<QuicCertificate> {
        read_json(&self.certificate_path())
    }

    pub fn load_config(&self) -> Result<HomeConfig> {
        let config: HomeConfig = read_json(&self.config_path())?;
        if config.v != 1 {
            anyhow::bail!("unsupported home config version {}", config.v);
        }
        Ok(config)
    }

    pub fn open_store(&self) -> Result<Store> {
        Store::open(self.store_path())
    }

    fn write_ui_preferences(&self, preferences: &UiPreferences) -> Result<()> {
        validate_ui_preferences(preferences)?;
        write_json(&self.ui_preferences_path(), preferences)
    }

    fn load_or_create_identity(&self) -> Result<PeerIdentity> {
        if self.identity_path().exists() {
            return self.load_identity();
        }
        let identity = PeerIdentity::generate()?;
        let file = IdentityFile {
            v: 1,
            peer_secret_b64: identity.peer.secret_key_b64(),
            device_secret_b64: identity.device.secret_key_b64(),
            peer_id: identity.peer.id.clone(),
            device_id: identity.device.id.clone(),
        };
        write_json(&self.identity_path(), &file)?;
        Ok(identity)
    }

    fn load_or_create_certificate(&self) -> Result<QuicCertificate> {
        if self.certificate_path().exists() {
            return self.load_certificate();
        }
        let certificate = QuicCertificate::generate()?;
        write_json(&self.certificate_path(), &certificate)?;
        Ok(certificate)
    }

    fn ensure_member_join(
        &self,
        store: &Store,
        identity: &PeerIdentity,
        config: &HomeConfig,
    ) -> Result<()> {
        let existing_join = store
            .room_events(GOVERNANCE_ROOM_ID)?
            .into_iter()
            .any(|event| {
                event.kind == "MEMBER_JOIN"
                    && event.author_peer_id == identity.peer.id
                    && event.body.get("peer_id").and_then(|value| value.as_str())
                        == Some(identity.peer.id.as_str())
            });
        if existing_join {
            return Ok(());
        }

        let context = RoomContext::new(config.authority_peer_id.clone());
        let join = create_event(
            identity,
            create_delegation(
                &identity.peer,
                &identity.device,
                now_ms() - 60_000,
                now_ms() + 30 * 24 * 60 * 60_000,
                vec!["room:join".to_string()],
            )?,
            GOVERNANCE_ROOM_ID,
            now_ms(),
            "MEMBER_JOIN",
            vec![],
            serde_json::json!({
                "peer_id": identity.peer.id,
                "peer_pub": identity.peer.spki_b64,
            }),
        )?;
        let accepted = accept_event(&join, &[], &context, now_ms())
            .map_err(|e| anyhow::anyhow!("member join rejected: {e:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(())
    }
}

impl VoxelleCommandHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            home: VoxelleHome::new(root),
            service: None,
            activity: Vec::new(),
            next_activity_id: 1,
        }
    }

    pub fn home(&self) -> &VoxelleHome {
        &self.home
    }

    pub fn is_online(&self) -> bool {
        self.service.is_some()
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
        let service = self.home.start_service(bind, request.advertise)?;
        let addr = service.endpoint().addr;
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

    pub fn send_message(&mut self, request: SendMessageRequest) -> Result<ShellSnapshotView> {
        let event = self
            .home
            .send_message(&request.text, request.room.as_deref())?;
        self.push_activity(
            ServiceActivityLevel::Info,
            format!("sent message {}", event.event_id),
        );
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

    fn snapshot_without_drain(&self) -> Result<ShellSnapshotView> {
        let (home, home_error) = match self.home.home_screen_view_service(self.service.as_ref()) {
            Ok(home) => (Some(home), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        Ok(ShellSnapshotView {
            home_root: self.home.root().to_path_buf(),
            home,
            home_error,
            network_health: self.home.network_health_view(self.service.as_ref())?,
            ui_ontology: self.home.ui_ontology()?,
            service_activity: self.activity.clone(),
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
        self.endpoint.validate()
    }

    pub fn same_peer(&self, other: &Self) -> bool {
        self.endpoint.peer_id == other.endpoint.peer_id
            && self.endpoint.device_id == other.endpoint.device_id
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

    fn online(runtime: &VoxelleRuntime) -> Self {
        Self {
            state: RuntimeState::Online,
            listen_addr: Some(runtime.local_report.listen_addr),
            advertised_addr: Some(runtime.local_report.advertised_addr),
            reachability_notes: runtime.local_report.notes.clone(),
        }
    }

    fn from_service(service: &VoxelleService) -> Self {
        Self {
            state: RuntimeState::Online,
            listen_addr: Some(service.summary.local_report.listen_addr),
            advertised_addr: Some(service.summary.local_report.advertised_addr),
            reachability_notes: service.summary.local_report.notes.clone(),
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
            diagnostic_state: PeerActionState::NotRun,
            sync_state: PeerActionState::NotRun,
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

impl VoxelleRuntime {
    pub fn start(
        home: VoxelleHome,
        bind: SocketAddr,
        advertise: Option<SocketAddr>,
    ) -> Result<Self> {
        let identity = home.load_identity()?;
        let certificate = home.load_certificate()?;
        let node = QuicNode::bind_with_certificate(identity, certificate, bind)?;
        let advertised_addr = advertise.unwrap_or(node.local_addr()?);
        let endpoint = node.peer_endpoint(advertised_addr)?;
        let local_report = node.local_reachability_report(advertised_addr)?;
        Ok(Self {
            home,
            node,
            endpoint,
            local_report,
        })
    }

    pub fn home(&self) -> &VoxelleHome {
        &self.home
    }

    pub fn endpoint(&self) -> &PeerEndpoint {
        &self.endpoint
    }

    pub fn local_report(&self) -> &LocalReachabilityReport {
        &self.local_report
    }

    pub fn summary(&self) -> ListenSummary {
        ListenSummary {
            endpoint: self.endpoint.clone(),
            local_report: self.local_report.clone(),
        }
    }

    pub fn peer_record(&self, label: Option<String>, room: Option<&str>) -> Result<PeerRecord> {
        let default_room = match room {
            Some(room) => room.to_string(),
            None => self.home.load_config()?.default_room,
        };
        let record = PeerRecord {
            v: 1,
            label,
            default_room,
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
        })
    }

    pub async fn serve_sync_once(&self, home: &VoxelleHome) -> Result<ServedRoomSync> {
        let store = home.open_store()?;
        self.node.serve_room_sync_once(&store).await
    }

    pub async fn serve_sync_requests(
        &self,
        home: &VoxelleHome,
        count: usize,
    ) -> Result<Vec<ServedRoomSync>> {
        let mut served = Vec::with_capacity(count);
        for _ in 0..count {
            served.push(self.serve_sync_once(home).await?);
        }
        Ok(served)
    }

    pub async fn serve_diagnostic_once(&self) -> Result<PeerReachabilityReport> {
        self.node.serve_diagnostic_once().await
    }

    pub async fn serve_next_request(&self) -> Result<ServedPeerRequest> {
        let store = self.home.open_store()?;
        self.node.serve_peer_request_once(&store).await
    }

    pub async fn serve_requests(&self, count: usize) -> Result<Vec<ServedPeerRequest>> {
        let mut served = Vec::with_capacity(count);
        for _ in 0..count {
            served.push(self.serve_next_request().await?);
        }
        Ok(served)
    }

    pub async fn stop(self) {
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
    ) -> Result<Self> {
        let runtime = VoxelleRuntime::start(home, bind, advertise)?;
        let summary = runtime.summary();
        let default_room = runtime.home.load_config()?.default_room;
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let (event_tx, events) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("voxelle-service".to_string())
            .spawn(move || run_service_thread(runtime, stop_rx, event_tx))
            .context("spawn voxelle service thread")?;

        Ok(Self {
            summary,
            default_room,
            events,
            stop: Some(stop_tx),
            thread: Some(thread),
        })
    }

    pub fn summary(&self) -> &ListenSummary {
        &self.summary
    }

    pub fn endpoint(&self) -> &PeerEndpoint {
        &self.summary.endpoint
    }

    pub fn local_report(&self) -> &LocalReachabilityReport {
        &self.summary.local_report
    }

    pub fn peer_record(&self, label: Option<String>, room: Option<&str>) -> Result<PeerRecord> {
        let default_room = room
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.default_room.clone());
        let record = PeerRecord {
            v: 1,
            label,
            default_room,
            endpoint: self.summary.endpoint.clone(),
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
        })
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
    runtime: VoxelleRuntime,
    stop_rx: tokio::sync::oneshot::Receiver<()>,
    event_tx: mpsc::Sender<VoxelleServiceEvent>,
) {
    let Ok(task_runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let _ = event_tx.send(VoxelleServiceEvent::Failed(
            "failed to create service runtime".to_string(),
        ));
        return;
    };
    task_runtime.block_on(run_service_loop(runtime, stop_rx, event_tx));
}

async fn run_service_loop(
    runtime: VoxelleRuntime,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    event_tx: mpsc::Sender<VoxelleServiceEvent>,
) {
    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            result = runtime.serve_next_request() => {
                match result {
                    Ok(served) => {
                        let _ = event_tx.send(VoxelleServiceEvent::Served(served));
                    }
                    Err(error) => {
                        let _ = event_tx.send(VoxelleServiceEvent::Failed(format!("{error:#}")));
                        break;
                    }
                }
            }
        }
    }
    runtime.stop().await;
    let _ = event_tx.send(VoxelleServiceEvent::Stopped);
}

fn default_ui_ontology(preferences: UiPreferences) -> UiOntologyView {
    let mut semantic_tokens = default_semantic_tokens();
    for token in &mut semantic_tokens {
        if let Some(value) = preferences.semantic_tokens.get(&token.id) {
            token.current_value = value.clone();
        }
    }

    let mut metrics = default_metrics();
    for metric in &mut metrics {
        if let Some(value) = preferences.metrics.get(&metric.id) {
            metric.current_value = *value;
        }
    }

    let mut behaviors = default_behaviors();
    for behavior in &mut behaviors {
        if let Some(value) = preferences.behaviors.get(&behavior.id) {
            behavior.current_value = value.clone();
        }
    }

    UiOntologyView {
        places: default_places(),
        views: default_views(),
        commands: default_commands(),
        semantic_tokens,
        metrics,
        behaviors,
        renderers: default_renderers(),
    }
}

fn default_places() -> Vec<UiPlace> {
    vec![
        ui_place(
            "sidebar",
            "Sidebar",
            "Navigation and secondary app surfaces",
            true,
            "layout/place editor",
        ),
        ui_place(
            "main",
            "Main",
            "Primary room and message surfaces",
            true,
            "layout/place editor",
        ),
        ui_place(
            "inspector",
            "Inspector",
            "Future selected peer or message details",
            true,
            "layout/place editor",
        ),
        ui_place(
            "activity",
            "Activity",
            "Service, diagnostic, and sync activity",
            true,
            "layout/place editor",
        ),
        ui_place(
            "status",
            "Status",
            "Runtime and reachability state",
            true,
            "layout/place editor",
        ),
    ]
}

fn default_views() -> Vec<UiView> {
    vec![
        ui_view(
            "profile.summary",
            "Profile Summary",
            "sidebar",
            "Local peer and device identity",
        ),
        ui_view(
            "runtime.status",
            "Runtime Status",
            "status",
            "Online/offline and reachability state",
        ),
        ui_view(
            "network.health",
            "Network Health",
            "status",
            "Re-entrant checklist for setup, reachability, and repair",
        ),
        ui_view(
            "invite.exchange",
            "Invite Exchange",
            "sidebar",
            "Copyable peer record and peer import",
        ),
        ui_view(
            "peer.list",
            "Peer List",
            "sidebar",
            "Known peers and peer actions",
        ),
        ui_view(
            "room.timeline",
            "Room Timeline",
            "main",
            "Messages in the selected room",
        ),
        ui_view(
            "message.composer",
            "Message Composer",
            "main",
            "Message entry and send command",
        ),
        ui_view(
            "service.activity",
            "Service Activity",
            "activity",
            "Served requests, diagnostics, and sync events",
        ),
    ]
}

fn default_commands() -> Vec<UiCommand> {
    vec![
        ui_command("home.init", "Initialize Home", "Create local app state"),
        ui_command(
            "runtime.goOnline",
            "Go Online",
            "Start resident peer serving",
        ),
        ui_command(
            "runtime.goOffline",
            "Go Offline",
            "Stop resident peer serving",
        ),
        ui_command(
            "message.send",
            "Send Message",
            "Send a message to the current room",
        ),
        ui_command("invite.copy", "Copy Invite", "Copy the current peer record"),
        ui_command("peer.import", "Import Peer", "Import a peer record"),
        ui_command("peer.diagnose", "Diagnose Peer", "Check peer reachability"),
        ui_command(
            "peer.sync",
            "Sync Peer",
            "Sync governance and room events with a peer",
        ),
    ]
}

fn default_semantic_tokens() -> Vec<SemanticToken> {
    vec![
        semantic_token(
            "app.background",
            "App Background",
            "Canvas",
            "system.canvas",
            &["profile.summary", "room.timeline"],
        ),
        semantic_token(
            "panel.background",
            "Panel Background",
            "Panel surface",
            "system.panel",
            &["peer.list", "invite.exchange", "service.activity"],
        ),
        semantic_token(
            "panel.border",
            "Panel Border",
            "Panel boundary",
            "system.border",
            &["sidebar", "inspector"],
        ),
        semantic_token(
            "text.primary",
            "Primary Text",
            "Primary readable text",
            "system.text",
            &["profile.summary", "room.timeline", "message.composer"],
        ),
        semantic_token(
            "text.secondary",
            "Secondary Text",
            "Secondary metadata text",
            "system.text.secondary",
            &["peer.list", "service.activity"],
        ),
        semantic_token(
            "runtime.online",
            "Runtime Online",
            "Online runtime state",
            "system.success",
            &["runtime.status"],
        ),
        semantic_token(
            "runtime.offline",
            "Runtime Offline",
            "Offline runtime state",
            "system.muted",
            &["runtime.status"],
        ),
        semantic_token(
            "peer.reachable",
            "Peer Reachable",
            "Reachable peer diagnostic",
            "system.success",
            &["peer.list", "service.activity"],
        ),
        semantic_token(
            "peer.unreachable",
            "Peer Unreachable",
            "Unreachable peer diagnostic",
            "system.error",
            &["peer.list", "service.activity"],
        ),
        semantic_token(
            "message.own.background",
            "Own Message Background",
            "Messages authored by this peer",
            "system.message.own",
            &["room.timeline"],
        ),
        semantic_token(
            "message.remote.background",
            "Remote Message Background",
            "Messages authored by other peers",
            "system.message.remote",
            &["room.timeline"],
        ),
        semantic_token(
            "activity.info",
            "Activity Info",
            "Informational activity entries",
            "system.info",
            &["service.activity"],
        ),
        semantic_token(
            "activity.error",
            "Activity Error",
            "Error activity entries",
            "system.error",
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

fn ui_place(
    id: &str,
    label: &str,
    description: &str,
    editable: bool,
    editing_surface: &str,
) -> UiPlace {
    UiPlace {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        editable,
        editing_surface: editing_surface.to_string(),
    }
}

fn ui_view(id: &str, label: &str, place_id: &str, description: &str) -> UiView {
    UiView {
        id: id.to_string(),
        label: label.to_string(),
        place_id: place_id.to_string(),
        description: description.to_string(),
        editable: true,
        editing_surface: "layout/place editor".to_string(),
    }
}

fn ui_command(id: &str, label: &str, description: &str) -> UiCommand {
    UiCommand {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        editable: true,
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
        editable: true,
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

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}

fn short_peer_label(peer_id: &str) -> String {
    peer_id
        .strip_prefix("ed25519:")
        .and_then(|rest| rest.get(..12))
        .map(|short| format!("Peer {short}"))
        .unwrap_or_else(|| "Peer".to_string())
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
    use tempfile::tempdir;

    #[test]
    fn home_init_send_read_and_endpoint_export_are_app_actions() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));

        let profile = home.init(DEFAULT_ROOM_ID).expect("init");
        assert!(home.identity_path().exists());
        assert!(home.certificate_path().exists());
        assert!(home.store_path().exists());
        assert_eq!(profile.default_room, DEFAULT_ROOM_ID);
        assert_eq!(profile.peer_id, profile.authority_peer_id);

        let event = home
            .send_message("hello from app layer", None)
            .expect("send");
        assert_eq!(event.kind, "MSG_POST");

        let messages = home.read_messages(None).expect("read");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "hello from app layer");

        let endpoint = home
            .export_endpoint(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 4040))
            .expect("endpoint");
        endpoint.validate().expect("valid endpoint");
        assert_eq!(endpoint.peer_id, profile.peer_id);
        assert_eq!(endpoint.device_id, profile.device_id);
    }

    #[test]
    fn home_init_is_idempotent_and_preserves_identity() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));

        let first = home.init(DEFAULT_ROOM_ID).expect("first init");
        let second = home.init("room:ignored").expect("second init");

        assert_eq!(first.peer_id, second.peer_id);
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(second.default_room, DEFAULT_ROOM_ID);
        assert_eq!(
            home.open_store()
                .expect("store")
                .room_event_count(GOVERNANCE_ROOM_ID)
                .expect("count"),
            1
        );
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
        assert_eq!(
            semantic_token_value(&ontology, "peer.reachable"),
            "system.success"
        );
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

        home.set_semantic_token("peer.reachable", "#00ff00")
            .expect("set token");
        home.set_metric("sidebar.width", 420.0).expect("set metric");
        home.set_behavior(
            "timestamps.style",
            UiBehaviorValue::Text("absolute".to_string()),
        )
        .expect("set behavior");

        let reopened = VoxelleHome::new(home.root().to_path_buf());
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

        let ontology = reopened.ui_ontology().expect("ontology");
        assert_eq!(semantic_token_value(&ontology, "peer.reachable"), "#00ff00");
        assert_eq!(metric_value(&ontology, "sidebar.width"), 420.0);
        assert_eq!(
            behavior_value(&ontology, "timestamps.style"),
            UiBehaviorValue::Text("absolute".to_string())
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
        assert_eq!(
            semantic_token_value(&defaults, "peer.reachable"),
            "system.success"
        );
        assert_eq!(
            behavior_value(&defaults, "timestamps.style"),
            UiBehaviorValue::Text("relative".to_string())
        );
    }

    #[test]
    fn ui_preferences_reject_unknown_ids_and_wrong_behavior_kind() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("home"));
        home.init(DEFAULT_ROOM_ID).expect("init");

        assert!(home.set_semantic_token("unknown.token", "#fff").is_err());
        assert!(home.set_metric("sidebar.width", -1.0).is_err());
        assert!(home
            .set_behavior(
                "timestamps.visible",
                UiBehaviorValue::Text("yes".to_string())
            )
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
        let mut alice = VoxelleCommandHost::new(dir.path().join("alice"));
        let mut bob = VoxelleCommandHost::new(dir.path().join("bob"));

        alice
            .init_home(InitHomeRequest { default_room: None })
            .expect("alice init");
        bob.init_home(InitHomeRequest { default_room: None })
            .expect("bob init");
        alice
            .send_message(SendMessageRequest {
                text: "from command host".to_string(),
                room: None,
            })
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
        let peer_record_json = alice_online
            .home
            .as_ref()
            .expect("home view")
            .invite
            .as_ref()
            .expect("invite")
            .peer_record_json
            .clone();

        let bob_imported = bob
            .import_peer_record(ImportPeerRecordRequest { peer_record_json })
            .expect("import");
        assert_eq!(
            network_health_status(&bob_imported.network_health, "peers"),
            NetworkHealthStatus::Working
        );
        let peer = bob.home().known_peers().expect("known peers")[0].clone();
        let request = PeerCommandRequest {
            peer_id: peer.endpoint.peer_id.clone(),
            device_id: peer.endpoint.device_id.clone(),
            max_events: Some(64),
        };

        let diagnosed = bob.diagnose_peer(request.clone()).await.expect("diagnose");
        assert!(diagnosed
            .service_activity
            .iter()
            .any(|item| item.summary.starts_with("diagnostic reached")));

        let synced = bob.sync_peer(request).await.expect("sync");
        assert!(synced
            .service_activity
            .iter()
            .any(|item| item.summary.contains("room accepted 1")));
        assert_eq!(
            synced.home.expect("home").room.messages[0].text,
            "from command host"
        );

        let alice_after_serving = alice.snapshot().expect("alice snapshot");
        assert!(alice_after_serving
            .service_activity
            .iter()
            .any(|item| item.summary.starts_with("served diagnostic:")));
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

        let health = home.network_health_view(Some(&service)).expect("health");

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
        let peer_runtime = peer
            .listen(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("peer runtime");
        let peer_record = peer_runtime
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
    }

    #[tokio::test]
    async fn two_homes_sync_messages_over_ipv6_loopback() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob"));

        alice.init(DEFAULT_ROOM_ID).expect("alice init");
        bob.init(DEFAULT_ROOM_ID).expect("bob init");
        alice.send_message("hello over quic", None).expect("send");

        let listener = alice
            .listen(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("listen");
        let endpoint = listener.endpoint().clone();

        let (diagnostic_served, report) = tokio::join!(
            listener.serve_diagnostic_once(),
            bob.diagnose_endpoint(&endpoint)
        );
        let report = report.expect("diagnose");
        assert!(report.reachable);
        diagnostic_served.expect("diagnostic served");

        let listener = alice
            .listen(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("listen");
        let endpoint = listener.endpoint().clone();
        let (served, report) = tokio::join!(
            listener.serve_sync_requests(&alice, 2),
            bob.sync_endpoint(&endpoint, None, 64)
        );
        let served = served.expect("sync served");
        let report = report.expect("sync peer");

        assert_eq!(served[0].room_id, GOVERNANCE_ROOM_ID);
        assert_eq!(served[1].room_id, DEFAULT_ROOM_ID);
        assert_eq!(report.governance.accepted, 1);
        assert_eq!(report.room.accepted, 1);
        assert_eq!(
            bob.read_messages(None).expect("bob messages")[0].text,
            "hello over quic"
        );
    }

    #[tokio::test]
    async fn runtime_serves_repeated_diagnostics_and_sync() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob"));

        alice.init(DEFAULT_ROOM_ID).expect("alice init");
        bob.init(DEFAULT_ROOM_ID).expect("bob init");
        alice.send_message("first", None).expect("first send");

        let runtime = VoxelleRuntime::start(
            alice.clone(),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
            None,
        )
        .expect("runtime");
        let endpoint = runtime.endpoint().clone();

        let client = async {
            let diagnostic = bob.diagnose_endpoint(&endpoint).await.expect("diagnose");
            assert!(diagnostic.reachable);

            let first = bob
                .sync_endpoint(&endpoint, None, 64)
                .await
                .expect("first sync");
            assert_eq!(first.governance.accepted, 1);
            assert_eq!(first.room.accepted, 1);

            alice.send_message("second", None).expect("second send");

            let second = bob
                .sync_endpoint(&endpoint, None, 64)
                .await
                .expect("second sync");
            assert_eq!(second.governance.offered, 0);
            assert_eq!(second.room.accepted, 1);
        };

        let (served, _) = tokio::join!(runtime.serve_requests(5), client);
        let served = served.expect("served requests");
        assert!(matches!(served[0], ServedPeerRequest::Diagnostic(_)));
        assert!(matches!(served[1], ServedPeerRequest::RoomSync(_)));
        assert!(matches!(served[2], ServedPeerRequest::RoomSync(_)));
        assert!(matches!(served[3], ServedPeerRequest::RoomSync(_)));
        assert!(matches!(served[4], ServedPeerRequest::RoomSync(_)));

        let messages = bob.read_messages(None).expect("bob messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "first");
        assert_eq!(messages[1].text, "second");

        runtime.stop().await;
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

        let runtime = alice
            .listen(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("runtime");
        let alice_record = runtime
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

        let client = async {
            let diagnostic = bob.diagnose_peer(&alice_record).await.expect("diagnose");
            assert!(diagnostic.reachable);

            let sync = bob.sync_peer(&alice_record, 64).await.expect("sync");
            assert_eq!(sync.governance.accepted, 1);
            assert_eq!(sync.room.accepted, 1);
        };

        let (served, _) = tokio::join!(runtime.serve_requests(3), client);
        let served = served.expect("served requests");
        assert!(matches!(served[0], ServedPeerRequest::Diagnostic(_)));
        assert!(matches!(served[1], ServedPeerRequest::RoomSync(_)));
        assert!(matches!(served[2], ServedPeerRequest::RoomSync(_)));
        assert_eq!(
            bob.read_messages(None).expect("bob messages")[0].text,
            "from imported peer"
        );

        runtime.stop().await;
    }

    #[tokio::test]
    async fn service_keeps_home_online_for_diagnostics_and_sync() {
        let dir = tempdir().expect("tempdir");
        let alice = VoxelleHome::new(dir.path().join("alice"));
        let bob = VoxelleHome::new(dir.path().join("bob"));

        alice.init(DEFAULT_ROOM_ID).expect("alice init");
        bob.init(DEFAULT_ROOM_ID).expect("bob init");
        alice
            .send_message("first service message", None)
            .expect("send");

        let service = alice
            .start_service(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("service");
        let record = service
            .peer_record(Some("Alice".to_string()), None)
            .expect("record");

        let diagnostic = bob.diagnose_peer(&record).await.expect("diagnose");
        assert!(diagnostic.reachable);

        let first = bob.sync_peer(&record, 64).await.expect("first sync");
        assert_eq!(first.governance.accepted, 1);
        assert_eq!(first.room.accepted, 1);

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
        assert!(matches!(
            event,
            VoxelleServiceEvent::Served(ServedPeerRequest::Diagnostic(_))
        ));
        assert!(event.summary().starts_with("served diagnostic:"));
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
        assert_eq!(offline.room.room_id, DEFAULT_ROOM_ID);
        assert_eq!(offline.room.messages[0].text, "visible message");

        let runtime = home
            .listen(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("runtime");
        let peer_runtime = peer_home
            .listen(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), None)
            .expect("peer runtime");
        let peer_record = peer_runtime
            .peer_record(Some("Peer One".to_string()), None)
            .expect("peer record");
        home.import_peer_record(peer_record.clone())
            .expect("import peer");

        let online = home.home_screen_view(Some(&runtime)).expect("online view");
        assert_eq!(online.runtime.state, RuntimeState::Online);
        assert!(online.runtime.listen_addr.is_some());
        assert!(online.invite.is_some());
        assert!(online
            .invite
            .as_ref()
            .expect("invite")
            .peer_record_json
            .contains("\"default_room\": \"room:general\""));
        assert_eq!(online.peers.len(), 1);
        assert_eq!(online.peers[0].label, "Peer One");
        assert_eq!(online.peers[0].peer_id, peer_record.endpoint.peer_id);
        assert_eq!(online.peers[0].diagnostic_state, PeerActionState::NotRun);
        assert_eq!(online.peers[0].sync_state, PeerActionState::NotRun);
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
