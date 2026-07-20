use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use ts_rs::TS;
use voxelle_app::{
    ImportPeerRecordRequest, InitHomeRequest, PeerCommandRequest, SendMessageRequest,
    ShellSnapshotView, StartServiceRequest, VoxelleCommandHost,
};

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

    fn host(&self) -> ShellResult<MutexGuard<'_, VoxelleCommandHost>> {
        self.host.lock().map_err(|_| ShellError {
            message: "shell state lock poisoned".to_string(),
        })
    }
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

pub fn shell_contract_typescript() -> String {
    let mut output = voxelle_app::shell_contract_typescript();
    let cfg = ts_rs::Config::default();
    output.push_str("export ");
    output.push_str(&ShellError::decl(&cfg));
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

pub fn write_shell_contract(path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, shell_contract_typescript())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxelle_app::{NetworkHealthStatus, DEFAULT_ROOM_ID};

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
}
