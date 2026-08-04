use crate::{ShellSnapshotView, VoxelleCommandHost};
use std::path::PathBuf;
use tokio::sync::Mutex;
use ts_rs::TS;

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

    pub async fn execute_serialized_command(
        &self,
        command_id: &str,
        payload: serde_json::Value,
    ) -> ShellResult<ShellSnapshotView> {
        let mut host = self.host.lock().await;
        let result = match command_id {
            "shell.refresh" => host.snapshot(),
            "home.init" => host.init_home(parse_request(payload)?),
            "runtime.goOnline" => host.start_service(parse_request(payload)?),
            "runtime.goOffline" => host.stop_service(),
            "space.invite.create" => host.create_space_invite(parse_request(payload)?),
            "space.join" => host.join_space(parse_request(payload)?).await,
            "message.send" => host.send_message(parse_request(payload)?),
            "peer.import" => host.import_peer_record(parse_request(payload)?),
            "peer.diagnose" => host.diagnose_peer(parse_request(payload)?).await,
            "peer.sync" => host.sync_peer(parse_request(payload)?).await,
            "ui.preference.set" => host.set_ui_preference(parse_request(payload)?),
            _ => {
                return Err(ShellError {
                    message: format!("unknown command {command_id}"),
                })
            }
        };
        result.map_err(ShellError::from)
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

    #[tokio::test]
    async fn shell_state_returns_pre_init_snapshot_for_web_shell() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));

        let snapshot = shell
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("snapshot");

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
            .execute_serialized_command(
                "home.init",
                serde_json::json!({ "default_room": DEFAULT_ROOM_ID }),
            )
            .await
            .expect("alice init");
        alice
            .execute_serialized_command(
                "message.send",
                serde_json::json!({ "text": "hello through shell", "room": null }),
            )
            .await
            .expect("send");

        let alice_online = alice
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({ "bind": null, "advertise": null }),
            )
            .await
            .expect("alice online");
        assert_eq!(
            health_status(&alice_online, "service"),
            NetworkHealthStatus::Working
        );
        let invite_snapshot = alice
            .execute_serialized_command(
                "space.invite.create",
                serde_json::json!({ "expires_minutes": 60 }),
            )
            .await
            .expect("create invite");
        let space_invite_json = invite_snapshot
            .home
            .as_ref()
            .expect("home")
            .invite
            .as_ref()
            .expect("invite")
            .space_invite_json
            .as_ref()
            .expect("signed invite")
            .clone();
        let bob_joined = bob
            .execute_serialized_command(
                "space.join",
                serde_json::json!({
                    "space_invite_json": space_invite_json,
                    "max_events": 64
                }),
            )
            .await
            .expect("join");
        assert_eq!(
            health_status(&bob_joined, "peers"),
            NetworkHealthStatus::Working
        );
        assert_eq!(
            bob_joined.home.expect("home").room.messages[0].text,
            "hello through shell"
        );

        let alice_after_serving = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice snapshot");
        assert!(alice_after_serving
            .service_activity
            .iter()
            .any(|item| item.summary.starts_with("served sync:")));
        alice
            .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("stop");
    }

    #[tokio::test]
    async fn shell_state_returns_serializable_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));

        let error = shell
            .execute_serialized_command(
                "message.send",
                serde_json::json!({ "text": "not initialized", "room": null }),
            )
            .await
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
            .execute_serialized_command("home.init", serde_json::json!({ "default_room": null }))
            .await
            .expect("init");
        let snapshot = shell
            .execute_serialized_command(
                "message.send",
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
                "ui.preference.set",
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
                &reopened
                    .execute_serialized_command("shell.refresh", serde_json::json!({}))
                    .await
                    .expect("reopened snapshot"),
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
            .execute_serialized_command("message.send", serde_json::json!({}))
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
