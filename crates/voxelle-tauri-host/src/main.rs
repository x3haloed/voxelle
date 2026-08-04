use serde_json::Value;
use tauri::State;
use voxelle_app::{ShellError, ShellSnapshotView, ShellState};

fn main() {
    tauri::Builder::default()
        .manage(ShellState::new(voxelle_app::resolve_home_root(None)))
        .invoke_handler(tauri::generate_handler![execute_shell_command])
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}

#[tauri::command]
async fn execute_shell_command(
    state: State<'_, ShellState>,
    command_id: String,
    payload: Value,
) -> Result<ShellSnapshotView, ShellError> {
    state.execute_serialized_command(&command_id, payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_desktop_commands_initialize_and_send_a_message() {
        let dir = tempfile::tempdir().expect("temporary home");
        let state = ShellState::new(dir.path().join("home"));

        tauri::async_runtime::block_on(async {
            state
                .execute_serialized_command(
                    "home.init",
                    serde_json::json!({ "default_room": null }),
                )
                .await
                .expect("initialize home");
            let snapshot = state
                .execute_serialized_command(
                    "message.send",
                    serde_json::json!({ "text": "through desktop bridge", "room": null }),
                )
                .await
                .expect("send message");
            assert_eq!(
                snapshot.home.expect("initialized home").room.messages[0].text,
                "through desktop bridge"
            );

            let online = state
                .execute_serialized_command(
                    "runtime.goOnline",
                    serde_json::json!({ "bind": "[::1]:0", "advertise": null }),
                )
                .await
                .expect("start IPv6 service");
            assert_eq!(
                online.home.expect("online home").runtime.state,
                voxelle_app::RuntimeState::Online
            );
            state
                .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
                .await
                .expect("stop service");
        });
    }
}
