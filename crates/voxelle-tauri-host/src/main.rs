use tauri::State;
use voxelle_app::{
    ImportPeerRecordRequest, InitHomeRequest, PeerCommandRequest, SendMessageRequest,
    ShellSnapshotView, StartServiceRequest,
};
use voxelle_shell::{ShellError, ShellState};

fn main() {
    tauri::Builder::default()
        .manage(ShellState::new(voxelle_app::resolve_home_root(None)))
        .invoke_handler(tauri::generate_handler![
            snapshot,
            init_home,
            start_service,
            stop_service,
            send_message,
            import_peer_record,
            diagnose_peer,
            sync_peer
        ])
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}

#[tauri::command]
fn snapshot(state: State<'_, ShellState>) -> Result<ShellSnapshotView, ShellError> {
    state.snapshot()
}

#[tauri::command]
fn init_home(
    state: State<'_, ShellState>,
    request: InitHomeRequest,
) -> Result<ShellSnapshotView, ShellError> {
    state.init_home(request)
}

#[tauri::command]
async fn start_service(
    state: State<'_, ShellState>,
    request: StartServiceRequest,
) -> Result<ShellSnapshotView, ShellError> {
    state.start_service(request)
}

#[tauri::command]
fn stop_service(state: State<'_, ShellState>) -> Result<ShellSnapshotView, ShellError> {
    state.stop_service()
}

#[tauri::command]
fn send_message(
    state: State<'_, ShellState>,
    request: SendMessageRequest,
) -> Result<ShellSnapshotView, ShellError> {
    state.send_message(request)
}

#[tauri::command]
fn import_peer_record(
    state: State<'_, ShellState>,
    request: ImportPeerRecordRequest,
) -> Result<ShellSnapshotView, ShellError> {
    state.import_peer_record(request)
}

#[tauri::command]
fn diagnose_peer(
    state: State<'_, ShellState>,
    request: PeerCommandRequest,
) -> Result<ShellSnapshotView, ShellError> {
    tauri::async_runtime::block_on(state.diagnose_peer(request))
}

#[tauri::command]
fn sync_peer(
    state: State<'_, ShellState>,
    request: PeerCommandRequest,
) -> Result<ShellSnapshotView, ShellError> {
    tauri::async_runtime::block_on(state.sync_peer(request))
}
