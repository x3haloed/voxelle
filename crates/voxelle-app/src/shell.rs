use crate::{
    ImportPeerRecordRequest, InitHomeRequest, PeerCommandRequest, SendMessageRequest,
    SetUiPreferenceRequest, ShellSnapshotView, StartServiceRequest, VoxelleCommandHost,
};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use ts_rs::TS;

pub const SHELL_COMMAND_IDS: [&str; 9] = [
    "snapshot",
    "init_home",
    "start_service",
    "stop_service",
    "send_message",
    "import_peer_record",
    "diagnose_peer",
    "sync_peer",
    "set_ui_preference",
];

#[derive(Debug)]
enum ShellCommand {
    Snapshot,
    InitHome(InitHomeRequest),
    StartService(StartServiceRequest),
    StopService,
    SendMessage(SendMessageRequest),
    ImportPeerRecord(ImportPeerRecordRequest),
    DiagnosePeer(PeerCommandRequest),
    SyncPeer(PeerCommandRequest),
    SetUiPreference(SetUiPreferenceRequest),
}

impl ShellCommand {
    fn from_json(command_id: &str, payload: serde_json::Value) -> ShellResult<Self> {
        match command_id {
            "snapshot" => Ok(Self::Snapshot),
            "init_home" => Ok(Self::InitHome(parse_request(payload)?)),
            "start_service" => Ok(Self::StartService(parse_request(payload)?)),
            "stop_service" => Ok(Self::StopService),
            "send_message" => Ok(Self::SendMessage(parse_request(payload)?)),
            "import_peer_record" => Ok(Self::ImportPeerRecord(parse_request(payload)?)),
            "diagnose_peer" => Ok(Self::DiagnosePeer(parse_request(payload)?)),
            "sync_peer" => Ok(Self::SyncPeer(parse_request(payload)?)),
            "set_ui_preference" => Ok(Self::SetUiPreference(parse_request(payload)?)),
            _ => Err(ShellError {
                message: format!("unknown command {command_id}"),
            }),
        }
    }
}

#[derive(Debug)]
pub struct ShellState {
    host: Mutex<VoxelleCommandHost>,
}

impl ShellState {
    pub fn new(home_root: impl Into<PathBuf>) -> Self {
        Self {
            host: Mutex::new(VoxelleCommandHost::new(home_root)),
        }
    }

    pub fn snapshot(&self) -> ShellResult<ShellSnapshotView> {
        self.host()?.snapshot().map_err(ShellError::from)
    }

    pub fn init_home(&self, request: InitHomeRequest) -> ShellResult<ShellSnapshotView> {
        self.host()?.init_home(request).map_err(ShellError::from)
    }

    pub fn start_service(&self, request: StartServiceRequest) -> ShellResult<ShellSnapshotView> {
        self.host()?
            .start_service(request)
            .map_err(ShellError::from)
    }

    pub fn stop_service(&self) -> ShellResult<ShellSnapshotView> {
        self.host()?.stop_service().map_err(ShellError::from)
    }

    pub fn send_message(&self, request: SendMessageRequest) -> ShellResult<ShellSnapshotView> {
        self.host()?.send_message(request).map_err(ShellError::from)
    }

    pub fn import_peer_record(
        &self,
        request: ImportPeerRecordRequest,
    ) -> ShellResult<ShellSnapshotView> {
        self.host()?
            .import_peer_record(request)
            .map_err(ShellError::from)
    }

    pub fn set_ui_preference(
        &self,
        request: SetUiPreferenceRequest,
    ) -> ShellResult<ShellSnapshotView> {
        self.host()?
            .set_ui_preference(request)
            .map_err(ShellError::from)
    }

    pub async fn diagnose_peer(
        &self,
        request: PeerCommandRequest,
    ) -> ShellResult<ShellSnapshotView> {
        let mut host = self.host()?;
        host.diagnose_peer(request).await.map_err(ShellError::from)
    }

    pub async fn sync_peer(&self, request: PeerCommandRequest) -> ShellResult<ShellSnapshotView> {
        let mut host = self.host()?;
        host.sync_peer(request).await.map_err(ShellError::from)
    }

    pub async fn execute_serialized_command(
        &self,
        command_id: &str,
        payload: serde_json::Value,
    ) -> ShellResult<ShellSnapshotView> {
        let command = ShellCommand::from_json(command_id, payload)?;
        match command {
            ShellCommand::Snapshot => self.snapshot(),
            ShellCommand::InitHome(request) => self.init_home(request),
            ShellCommand::StartService(request) => self.start_service(request),
            ShellCommand::StopService => self.stop_service(),
            ShellCommand::SendMessage(request) => self.send_message(request),
            ShellCommand::ImportPeerRecord(request) => self.import_peer_record(request),
            ShellCommand::DiagnosePeer(request) => self.diagnose_peer(request).await,
            ShellCommand::SyncPeer(request) => self.sync_peer(request).await,
            ShellCommand::SetUiPreference(request) => self.set_ui_preference(request),
        }
    }

    fn host(&self) -> ShellResult<MutexGuard<'_, VoxelleCommandHost>> {
        self.host.lock().map_err(|_| ShellError {
            message: "shell state lock poisoned".to_string(),
        })
    }
}

fn parse_request<T: serde::de::DeserializeOwned>(payload: serde_json::Value) -> ShellResult<T> {
    serde_json::from_value(payload).map_err(|error| ShellError {
        message: format!("invalid command payload: {error}"),
    })
}

pub type ShellResult<T> = Result<T, ShellError>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, TS)]
pub struct ShellError {
    pub message: String,
}

impl From<anyhow::Error> for ShellError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            message: format!("{error:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{shell_contract_typescript, NetworkHealthStatus, DEFAULT_ROOM_ID};

    #[test]
    fn shell_state_returns_pre_init_snapshot_for_web_shell() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));

        let snapshot = shell.snapshot().expect("snapshot");

        assert!(snapshot.home.is_none());
        assert!(snapshot.home_error.is_some());
        assert_eq!(
            health_status(&snapshot, "home"),
            NetworkHealthStatus::NeedsAttention
        );
        assert!(snapshot
            .ui_ontology
            .views
            .iter()
            .any(|view| view.id == "network.health"));
    }

    #[tokio::test]
    async fn shell_state_drives_two_home_network_workflow() {
        let dir = tempfile::tempdir().expect("tempdir");
        let alice = ShellState::new(dir.path().join("alice"));
        let bob = ShellState::new(dir.path().join("bob"));

        alice
            .init_home(InitHomeRequest {
                default_room: Some(DEFAULT_ROOM_ID.to_string()),
            })
            .expect("alice init");
        bob.init_home(InitHomeRequest { default_room: None })
            .expect("bob init");
        alice
            .send_message(SendMessageRequest {
                text: "hello through shell".to_string(),
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
            health_status(&alice_online, "service"),
            NetworkHealthStatus::Working
        );
        let peer_record_json = alice_online
            .home
            .as_ref()
            .expect("home")
            .invite
            .as_ref()
            .expect("invite")
            .peer_record_json
            .clone();

        let bob_imported = bob
            .import_peer_record(ImportPeerRecordRequest { peer_record_json })
            .expect("import");
        assert_eq!(
            health_status(&bob_imported, "peers"),
            NetworkHealthStatus::Working
        );
        let peer = &bob_imported.home.as_ref().expect("home").peers[0];
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

        let synced = bob.sync_peer(request).await.expect("sync");
        assert_eq!(
            synced.home.expect("home").room.messages[0].text,
            "hello through shell"
        );

        let alice_after_serving = alice.snapshot().expect("alice snapshot");
        assert!(alice_after_serving
            .service_activity
            .iter()
            .any(|item| item.summary.starts_with("served diagnostic:")));
        alice.stop_service().expect("stop");
    }

    #[test]
    fn shell_state_returns_serializable_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));

        let error = shell
            .send_message(SendMessageRequest {
                text: "not initialized".to_string(),
                room: None,
            })
            .expect_err("send should fail");

        assert!(error.message.contains("identity.json"));
        let encoded = serde_json::to_string(&error).expect("serialize");
        assert!(encoded.contains("identity.json"));
    }

    #[tokio::test]
    async fn serialized_commands_preserve_the_shell_action_contract() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));

        shell
            .execute_serialized_command("init_home", serde_json::json!({ "default_room": null }))
            .await
            .expect("init");
        let snapshot = shell
            .execute_serialized_command(
                "send_message",
                serde_json::json!({ "text": "serialized shell command", "room": null }),
            )
            .await
            .expect("send");

        assert_eq!(
            snapshot.home.expect("home").room.messages[0].text,
            "serialized shell command"
        );
        let updated = shell
            .execute_serialized_command(
                "set_ui_preference",
                serde_json::json!({
                    "kind": "metric",
                    "id": "sidebar.width",
                    "value": 444.0
                }),
            )
            .await
            .expect("set preference");
        assert_eq!(metric_value(&updated, "sidebar.width"), 444.0);

        let reopened = ShellState::new(dir.path().join("home"));
        assert_eq!(
            metric_value(
                &reopened.snapshot().expect("reopened snapshot"),
                "sidebar.width"
            ),
            444.0
        );
        assert_eq!(
            shell
                .execute_serialized_command("not_a_command", serde_json::json!({}))
                .await
                .expect_err("unknown command")
                .message,
            "unknown command not_a_command"
        );
        assert!(shell
            .execute_serialized_command("send_message", serde_json::json!({}))
            .await
            .expect_err("invalid payload")
            .message
            .starts_with("invalid command payload:"));
    }

    #[test]
    fn generated_shell_contract_matches_checked_in_web_contract() {
        let contract_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("web")
            .join("src")
            .join("shell-contract.ts");

        let checked_in = std::fs::read_to_string(&contract_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", contract_path.display()));

        assert_eq!(checked_in, shell_contract_typescript());
    }

    fn health_status(snapshot: &ShellSnapshotView, id: &str) -> NetworkHealthStatus {
        snapshot
            .network_health
            .rows
            .iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("missing health row {id}"))
            .status
    }

    fn metric_value(snapshot: &ShellSnapshotView, id: &str) -> f64 {
        snapshot
            .ui_ontology
            .metrics
            .iter()
            .find(|metric| metric.id == id)
            .unwrap_or_else(|| panic!("missing metric {id}"))
            .current_value
    }
}
