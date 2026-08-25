#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rand::RngCore;
use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use voxelle_app::{ShellError, ShellSnapshotView, ShellState};

struct DesktopShellState {
    shell: ShellState,
    session_capability: [u8; 32],
}

impl DesktopShellState {
    fn new(shell: ShellState) -> Self {
        let mut session_capability = [0_u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut session_capability);
        Self {
            shell,
            session_capability,
        }
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            app.manage(DesktopShellState::new(ShellState::new_with_notifier(
                voxelle_app::resolve_home_root(None),
                Arc::new(move || {
                    let _ = app_handle.emit("voxelle://snapshot-invalidated", ());
                }),
            )));
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
    state: State<'_, DesktopShellState>,
    command_id: String,
    payload: Value,
) -> Result<ShellSnapshotView, ShellError> {
    execute_desktop_command(&state, &command_id, payload).await
}

async fn execute_desktop_command(
    state: &DesktopShellState,
    command_id: &str,
    payload: Value,
) -> Result<ShellSnapshotView, ShellError> {
    let origin_command = matches!(
        command_id,
        "message.send" | "message.acknowledge" | "message.continuation.update"
    );
    if origin_command && state.shell.current_device_id().await.is_some() {
        let mut request_nonce = [0_u8; 18];
        rand::rngs::OsRng.fill_bytes(&mut request_nonce);
        let request_id = format!(
            "desktop-{}",
            request_nonce
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let origin = state
            .shell
            .issue_native_webview_origin_context(&state.session_capability, request_id)
            .await?;
        state
            .shell
            .execute_serialized_command_with_origin(command_id, payload, origin)
            .await
    } else {
        state
            .shell
            .execute_serialized_command(command_id, payload)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_desktop_commands_initialize_and_send_a_message() {
        let dir = tempfile::tempdir().expect("temporary home");
        let state = DesktopShellState::new(ShellState::new(dir.path().join("home")));

        tauri::async_runtime::block_on(async {
            let pre_init = execute_desktop_command(
                &state,
                "message.send",
                serde_json::json!({ "text": "too early", "room": null }),
            )
            .await
            .expect_err("pre-init send remains an ordinary home error");
            assert_eq!(pre_init.recovery, voxelle_app::ShellRecovery::NeedsHome);

            execute_desktop_command(
                &state,
                "home.init",
                serde_json::json!({ "default_room": null }),
            )
            .await
            .expect("initialize home");
            let snapshot = execute_desktop_command(
                &state,
                "message.send",
                serde_json::json!({ "text": "through desktop bridge", "room": null }),
            )
            .await
            .expect("send message");
            let message = &snapshot
                .home
                .as_ref()
                .expect("initialized home")
                .room
                .messages[0];
            assert_eq!(message.text, "through desktop bridge");
            assert_eq!(
                message.origin.surface_protocol,
                Some(voxelle_app::OriginSurfaceProtocolView::NativeWebview)
            );
            assert_eq!(message.origin.display_label.as_deref(), Some("Desktop"));
            assert!(message
                .origin
                .request_id
                .as_deref()
                .is_some_and(|request_id| request_id.starts_with("desktop-")));
            let admitted_origin = message.origin.clone();

            let online = execute_desktop_command(
                &state,
                "runtime.goOnline",
                serde_json::json!({ "bind": "[::1]:0", "advertise": null }),
            )
            .await
            .expect("start IPv6 service");
            assert_eq!(
                online.home.as_ref().expect("online home").runtime.state,
                voxelle_app::RuntimeState::Online
            );
            assert_eq!(
                online.home.expect("online home").room.messages[0].origin,
                admitted_origin
            );
            execute_desktop_command(&state, "runtime.goOffline", serde_json::json!({}))
                .await
                .expect("stop service");
        });
    }
}
