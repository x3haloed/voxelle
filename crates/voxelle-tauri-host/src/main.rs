#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use voxelle_app::{ShellError, ShellSnapshotView, ShellState};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            app.manage(ShellState::new_with_notifier(
                voxelle_app::resolve_home_root(None),
                Arc::new(move || {
                    let _ = app_handle.emit("voxelle://snapshot-invalidated", ());
                }),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            execute_shell_command,
            choose_recovery_kit_path
        ])
        .run(tauri::generate_context!())
        .expect("run Voxelle Tauri host");
}

#[tauri::command]
async fn choose_recovery_kit_path(mode: String) -> Result<Option<String>, ShellError> {
    let dialog = rfd::AsyncFileDialog::new()
        .add_filter("Voxelle recovery kit", &["voxrecover"])
        .set_title(if mode == "save" {
            "Save Voxelle Recovery Kit"
        } else {
            "Choose Voxelle Recovery Kit"
        });
    let selection = match mode.as_str() {
        "save" => {
            dialog
                .set_file_name("voxelle-identity.voxrecover")
                .save_file()
                .await
        }
        "open" => dialog.pick_file().await,
        _ => {
            return Err(ShellError {
                message: "Voxelle could not open the recovery file chooser.".to_string(),
                recovery: voxelle_app::ShellRecovery::InternalError,
                recovery_message: "Close this window, reopen Voxelle, and try again.".to_string(),
                detail: format!("unknown recovery file dialog mode {mode}"),
            })
        }
    };
    Ok(selection.map(|file| file.path().to_string_lossy().into_owned()))
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
