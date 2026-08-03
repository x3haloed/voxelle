use serde_json::Value;
use tauri::State;
use voxelle_app::{DeferredShellCommand, ShellCommand, ShellError, ShellSnapshotView, ShellState};

fn main() {
    tauri::Builder::default()
        .manage(ShellState::new(voxelle_app::resolve_home_root(None)))
        .invoke_handler(tauri::generate_handler![execute_shell_command])
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}

#[tauri::command]
fn execute_shell_command(
    state: State<'_, ShellState>,
    command_id: String,
    payload: Value,
) -> Result<ShellSnapshotView, ShellError> {
    tauri::async_runtime::block_on(run_serialized_shell_command(&state, &command_id, payload))
}

async fn run_serialized_shell_command(
    state: &ShellState,
    command_id: &str,
    payload: Value,
) -> Result<ShellSnapshotView, ShellError> {
    let command = ShellCommand::from_json(command_id, payload)?;
    match state.execute_command(command) {
        Ok(result) => result,
        Err(DeferredShellCommand::DiagnosePeer(request)) => state.diagnose_peer(request).await,
        Err(DeferredShellCommand::SyncPeer(request)) => state.sync_peer(request).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_desktop_commands_initialize_and_send_a_message() {
        let dir = tempfile::tempdir().expect("temporary home");
        let state = ShellState::new(dir.path().join("home"));

        tauri::async_runtime::block_on(async {
            run_serialized_shell_command(
                &state,
                "init_home",
                serde_json::json!({ "default_room": null }),
            )
            .await
            .expect("initialize home");
            let snapshot = run_serialized_shell_command(
                &state,
                "send_message",
                serde_json::json!({ "text": "through desktop bridge", "room": null }),
            )
            .await
            .expect("send message");
            assert_eq!(
                snapshot.home.expect("initialized home").room.messages[0].text,
                "through desktop bridge"
            );

            let online = run_serialized_shell_command(
                &state,
                "start_service",
                serde_json::json!({ "bind": "[::1]:0", "advertise": null }),
            )
            .await
            .expect("start IPv6 service");
            assert_eq!(
                online.home.expect("online home").runtime.state,
                voxelle_app::RuntimeState::Online
            );
            run_serialized_shell_command(&state, "stop_service", serde_json::json!({}))
                .await
                .expect("stop service");
        });
    }
}
