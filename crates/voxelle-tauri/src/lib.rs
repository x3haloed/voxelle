use std::path::PathBuf;
use tauri::State;
use voxelle_app::{
    ImportPeerRecordRequest, InitHomeRequest, PeerCommandRequest, SendMessageRequest,
    ShellSnapshotView, StartServiceRequest,
};
use voxelle_shell::{ShellError, ShellResult, ShellState};

pub fn shell_state(home_root: impl Into<PathBuf>) -> ShellState {
    ShellState::new(home_root)
}

pub fn snapshot_shell(shell: &ShellState) -> ShellResult<ShellSnapshotView> {
    shell.snapshot()
}

pub fn init_home_shell(
    shell: &ShellState,
    request: InitHomeRequest,
) -> ShellResult<ShellSnapshotView> {
    shell.init_home(request)
}

pub fn start_service_shell(
    shell: &ShellState,
    request: StartServiceRequest,
) -> ShellResult<ShellSnapshotView> {
    shell.start_service(request)
}

pub fn stop_service_shell(shell: &ShellState) -> ShellResult<ShellSnapshotView> {
    shell.stop_service()
}

pub fn send_message_shell(
    shell: &ShellState,
    request: SendMessageRequest,
) -> ShellResult<ShellSnapshotView> {
    shell.send_message(request)
}

pub fn import_peer_record_shell(
    shell: &ShellState,
    request: ImportPeerRecordRequest,
) -> ShellResult<ShellSnapshotView> {
    shell.import_peer_record(request)
}

pub async fn diagnose_peer_shell(
    shell: &ShellState,
    request: PeerCommandRequest,
) -> ShellResult<ShellSnapshotView> {
    shell.diagnose_peer(request).await
}

pub async fn sync_peer_shell(
    shell: &ShellState,
    request: PeerCommandRequest,
) -> ShellResult<ShellSnapshotView> {
    shell.sync_peer(request).await
}

pub fn invoke_handler<R: tauri::Runtime>() -> Box<tauri::ipc::InvokeHandler<R>> {
    Box::new(tauri::generate_handler![
        snapshot,
        init_home,
        start_service,
        stop_service,
        send_message,
        import_peer_record,
        diagnose_peer,
        sync_peer
    ])
}

#[tauri::command]
fn snapshot(state: State<'_, ShellState>) -> Result<ShellSnapshotView, ShellError> {
    snapshot_shell(&state)
}

#[tauri::command]
fn init_home(
    state: State<'_, ShellState>,
    request: InitHomeRequest,
) -> Result<ShellSnapshotView, ShellError> {
    init_home_shell(&state, request)
}

#[tauri::command]
fn start_service(
    state: State<'_, ShellState>,
    request: StartServiceRequest,
) -> Result<ShellSnapshotView, ShellError> {
    start_service_shell(&state, request)
}

#[tauri::command]
fn stop_service(state: State<'_, ShellState>) -> Result<ShellSnapshotView, ShellError> {
    stop_service_shell(&state)
}

#[tauri::command]
fn send_message(
    state: State<'_, ShellState>,
    request: SendMessageRequest,
) -> Result<ShellSnapshotView, ShellError> {
    send_message_shell(&state, request)
}

#[tauri::command]
fn import_peer_record(
    state: State<'_, ShellState>,
    request: ImportPeerRecordRequest,
) -> Result<ShellSnapshotView, ShellError> {
    import_peer_record_shell(&state, request)
}

#[tauri::command]
fn diagnose_peer(
    state: State<'_, ShellState>,
    request: PeerCommandRequest,
) -> Result<ShellSnapshotView, ShellError> {
    tauri::async_runtime::block_on(diagnose_peer_shell(&state, request))
}

#[tauri::command]
fn sync_peer(
    state: State<'_, ShellState>,
    request: PeerCommandRequest,
) -> Result<ShellSnapshotView, ShellError> {
    tauri::async_runtime::block_on(sync_peer_shell(&state, request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxelle_app::{NetworkHealthStatus, DEFAULT_ROOM_ID};

    #[test]
    fn command_wrappers_drive_shell_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = shell_state(dir.path().join("home"));

        let pre_init = snapshot_shell(&shell).expect("snapshot");
        assert_eq!(
            health_status(&pre_init, "home"),
            NetworkHealthStatus::NeedsAttention
        );

        let initialized = init_home_shell(
            &shell,
            InitHomeRequest {
                default_room: Some(DEFAULT_ROOM_ID.to_string()),
            },
        )
        .expect("init");
        assert_eq!(
            health_status(&initialized, "home"),
            NetworkHealthStatus::Working
        );

        let error = send_message_shell(
            &shell_state(dir.path().join("empty")),
            SendMessageRequest {
                text: "missing home".to_string(),
                room: None,
            },
        )
        .expect_err("send should fail");
        assert!(error.message.contains("identity.json"));
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
