use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use voxelle_core::{
    accept_event, create_delegation, create_event, EventV1, PeerIdentity, RoomContext,
    GOVERNANCE_ROOM_ID,
};
use voxelle_net::{
    LocalReachabilityReport, PeerEndpoint, PeerReachabilityReport, QuicCertificate, QuicNode,
    ServedPeerRequest, ServedRoomSync,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeScreenView {
    pub profile: ProfileSummary,
    pub runtime: RuntimeStatusView,
    pub invite: Option<InviteExchangeView>,
    pub peers: Vec<PeerListItemView>,
    pub room: RoomTimelineView,
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
}
