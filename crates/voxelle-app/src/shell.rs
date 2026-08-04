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
            "shell.refresh" => host.refresh_and_sync().await,
            "home.init" => host.init_home(parse_request(payload)?),
            "runtime.goOnline" => host.start_service(parse_request(payload)?),
            "runtime.goOffline" => host.stop_service(),
            "space.invite.create" => host.create_space_invite(parse_request(payload)?),
            "space.join" => host.join_space(parse_request(payload)?).await,
            "message.send" => host.send_message(parse_request(payload)?).await,
            "channel.select" => host.select_channel(parse_request(payload)?),
            "channel.markRead" => host.mark_read(parse_request(payload)?),
            "channel.create" => host.create_channel(parse_request(payload)?).await,
            "channel.rotateKey" => host.rotate_channel_key(parse_request(payload)?).await,
            "call.join" => host.join_call(parse_request(payload)?).await,
            "call.signal" => host.signal_call(parse_request(payload)?).await,
            "call.heartbeat" => host.heartbeat_call(parse_request(payload)?).await,
            "call.leave" => host.leave_call(parse_request(payload)?).await,
            "message.edit" => host.edit_message(parse_request(payload)?).await,
            "message.redact" => host.redact_message(parse_request(payload)?).await,
            "reaction.add" => host.set_reaction(parse_request(payload)?, true).await,
            "reaction.remove" => host.set_reaction(parse_request(payload)?, false).await,
            "pin.add" => host.set_pin(parse_request(payload)?, true).await,
            "pin.remove" => host.set_pin(parse_request(payload)?, false).await,
            "attachment.add" => host.add_attachment(parse_request(payload)?).await,
            "profile.update" => host.update_profile(parse_request(payload)?).await,
            "role.create" => host.create_role(parse_request(payload)?).await,
            "role.grant" => host.assign_role(parse_request(payload)?, true).await,
            "role.revoke" => host.assign_role(parse_request(payload)?, false).await,
            "member.ban" => host.ban_member(parse_request(payload)?, true).await,
            "member.unban" => host.ban_member(parse_request(payload)?, false).await,
            "message.search" => host.search_messages(parse_request(payload)?),
            "peer.import" => host.import_peer_record(parse_request(payload)?),
            "peer.diagnose" => host.diagnose_peer(parse_request(payload)?).await,
            "peer.sync" => host.sync_peer(parse_request(payload)?).await,
            "ui.preference.set" => host.set_ui_preference(parse_request(payload)?),
            "workbench.layout.save" => host.set_workbench_layout(parse_request(payload)?),
            "workbench.layout.reset" => host.reset_workbench_layout(),
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

        alice
            .execute_serialized_command(
                "message.send",
                serde_json::json!({ "text": "arrives without manual sync", "room": null }),
            )
            .await
            .expect("alice sends again");
        let bob_refreshed = bob
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("online refresh performs anti-entropy");
        assert!(bob_refreshed
            .home
            .expect("bob home")
            .room
            .messages
            .iter()
            .any(|message| message.text == "arrives without manual sync"));

        bob.execute_serialized_command(
            "message.send",
            serde_json::json!({ "text": "pushes automatically", "room": null }),
        )
        .await
        .expect("bob sends and pushes");
        let alice_refreshed = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice refresh");
        assert!(alice_refreshed
            .home
            .expect("alice home")
            .room
            .messages
            .iter()
            .any(|message| message.text == "pushes automatically"));

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
    async fn discord_public_families_converge_through_serialized_shell_commands() {
        let dir = tempfile::tempdir().expect("tempdir");
        let alice = ShellState::new(dir.path().join("alice"));
        let bob = ShellState::new(dir.path().join("bob"));

        let alice_init = alice
            .execute_serialized_command("home.init", serde_json::json!({ "default_room": null }))
            .await
            .expect("alice init");
        let alice_peer_id = alice_init.home.expect("alice home").profile.peer_id;
        alice
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({ "bind": null, "advertise": null }),
            )
            .await
            .expect("alice online");
        let invite = alice
            .execute_serialized_command(
                "space.invite.create",
                serde_json::json!({ "expires_minutes": 60 }),
            )
            .await
            .expect("invite")
            .home
            .expect("home")
            .invite
            .expect("invite view")
            .space_invite_json
            .expect("invite json");
        let bob_joined = bob
            .execute_serialized_command(
                "space.join",
                serde_json::json!({
                    "space_invite_json": invite,
                    "max_events": 4096
                }),
            )
            .await
            .expect("bob join");
        let bob_peer_id = bob_joined.home.expect("bob home").profile.peer_id;

        alice
            .execute_serialized_command(
                "call.join",
                serde_json::json!({ "room": null, "video": false }),
            )
            .await
            .expect("alice joins call");
        bob.execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("bob sees alice call");
        let bob_call = bob
            .execute_serialized_command(
                "call.join",
                serde_json::json!({ "room": null, "video": true }),
            )
            .await
            .expect("bob joins call")
            .home
            .expect("bob home")
            .call;
        assert_eq!(bob_call.participants.len(), 2);
        let call_id = bob_call.call_id.clone();
        alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice sees bob call");
        alice
            .execute_serialized_command(
                "call.signal",
                serde_json::json!({
                    "room": null,
                    "call_id": call_id.clone(),
                    "target_peer_id": bob_peer_id,
                    "signal_type": "offer",
                    "sdp": "{\"type\":\"offer\",\"sdp\":\"v=0\"}",
                    "candidate": null
                }),
            )
            .await
            .expect("signed offer signal");
        let bob_signaled = bob
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("bob receives offer");
        assert!(bob_signaled
            .home
            .expect("bob home")
            .call
            .signals
            .iter()
            .any(|signal| signal.kind == "CALL_OFFER" && signal.author_peer_id == alice_peer_id));
        alice
            .execute_serialized_command(
                "call.heartbeat",
                serde_json::json!({ "room": null, "call_id": call_id.clone() }),
            )
            .await
            .expect("alice heartbeat");
        bob.execute_serialized_command(
            "call.leave",
            serde_json::json!({ "room": null, "call_id": call_id }),
        )
        .await
        .expect("bob leaves call");
        let alice_after_leave = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice sees bob leave");
        assert_eq!(
            alice_after_leave
                .home
                .expect("alice home")
                .call
                .participants,
            vec![alice_peer_id.clone()]
        );

        let channel_snapshot = alice
            .execute_serialized_command(
                "channel.create",
                serde_json::json!({
                    "name": "Engineering",
                    "topic": "Build notes",
                    "private_members": []
                }),
            )
            .await
            .expect("create channel");
        let channel_id = channel_snapshot
            .home
            .expect("home")
            .channels
            .into_iter()
            .find(|channel| channel.name == "Engineering")
            .expect("new channel")
            .room_id;
        bob.execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("bob pulls channel");
        bob.execute_serialized_command(
            "channel.select",
            serde_json::json!({ "room_id": channel_id }),
        )
        .await
        .expect("bob selects channel");

        let posted = alice
            .execute_serialized_command(
                "message.send",
                serde_json::json!({
                    "text": "Design packet alpha",
                    "room": channel_id,
                    "mentions": [bob_peer_id],
                    "thread_root_event_id": null
                }),
            )
            .await
            .expect("alice post");
        let root_id = posted.home.expect("home").room.messages[0].event_id.clone();
        let bob_received = bob
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("bob pulls post");
        let bob_received_home = bob_received.home.expect("home");
        assert_eq!(
            bob_received_home.room.messages[0].mentions,
            vec![bob_peer_id.clone()]
        );
        assert_eq!(
            bob_received_home
                .channels
                .iter()
                .find(|channel| channel.room_id == channel_id)
                .expect("engineering channel")
                .unread_count,
            1
        );
        assert_eq!(bob_received_home.notifications.len(), 1);
        let marked_read = bob
            .execute_serialized_command(
                "channel.markRead",
                serde_json::json!({ "room_id": channel_id }),
            )
            .await
            .expect("mark read");
        assert_eq!(
            marked_read
                .home
                .expect("home")
                .channels
                .iter()
                .find(|channel| channel.room_id == channel_id)
                .expect("engineering channel")
                .unread_count,
            0
        );

        bob.execute_serialized_command(
            "reaction.add",
            serde_json::json!({
                "target_event_id": root_id,
                "emoji": "👍",
                "room": channel_id
            }),
        )
        .await
        .expect("bob reacts");
        bob.execute_serialized_command(
            "message.send",
            serde_json::json!({
                "text": "Thread reply",
                "room": channel_id,
                "mentions": [alice_peer_id],
                "thread_root_event_id": root_id
            }),
        )
        .await
        .expect("bob replies");
        let alice_thread = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice pulls thread");
        let root = alice_thread
            .home
            .expect("home")
            .room
            .messages
            .into_iter()
            .find(|message| message.event_id == root_id)
            .expect("root");
        assert_eq!(root.reply_count, 1);
        assert_eq!(root.reactions[0].peer_ids, vec![bob_peer_id.clone()]);

        alice
            .execute_serialized_command(
                "message.edit",
                serde_json::json!({
                    "target_event_id": root_id,
                    "text": "Design packet beta",
                    "room": channel_id,
                    "mentions": [bob_peer_id]
                }),
            )
            .await
            .expect("edit");
        alice
            .execute_serialized_command(
                "pin.add",
                serde_json::json!({
                    "target_event_id": root_id,
                    "room": channel_id
                }),
            )
            .await
            .expect("authority pin");
        alice
            .execute_serialized_command(
                "attachment.add",
                serde_json::json!({
                    "filename": "notes.txt",
                    "mime": "text/plain",
                    "data_b64": "YnVpbGQgbm90ZXM=",
                    "room": channel_id
                }),
            )
            .await
            .expect("attachment");
        bob.execute_serialized_command(
            "profile.update",
            serde_json::json!({
                "display_name": "Bob Builder",
                "about": "Distributed systems"
            }),
        )
        .await
        .expect("profile");

        let role_snapshot = alice
            .execute_serialized_command(
                "role.create",
                serde_json::json!({
                    "name": "Moderator",
                    "permissions": ["message:moderate", "message:pin"]
                }),
            )
            .await
            .expect("role create");
        let role_id = role_snapshot
            .home
            .expect("home")
            .roles
            .into_iter()
            .find(|role| role.name == "Moderator")
            .expect("role")
            .role_id;
        alice
            .execute_serialized_command(
                "role.grant",
                serde_json::json!({
                    "peer_id": bob_peer_id,
                    "role_id": role_id
                }),
            )
            .await
            .expect("grant role");
        bob.execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("bob pulls final state");
        bob.execute_serialized_command(
            "message.redact",
            serde_json::json!({
                "target_event_id": root_id,
                "room": channel_id
            }),
        )
        .await
        .expect("moderator redacts another member message");

        let final_snapshot = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice final refresh");
        let final_home = final_snapshot.home.expect("home");
        let final_root = final_home
            .room
            .messages
            .iter()
            .find(|message| message.event_id == root_id)
            .expect("root retained as tombstone");
        assert!(final_root.redacted);
        assert_eq!(final_root.text, "Message removed");
        assert!(final_home.room.messages.iter().any(|message| message
            .attachments
            .iter()
            .any(|attachment| attachment.filename == "notes.txt"
                && attachment.sha256.starts_with("sha256:"))));
        assert!(final_home
            .profiles
            .iter()
            .any(|profile| profile.display_name == "Bob Builder"));

        let search = alice
            .execute_serialized_command(
                "message.search",
                serde_json::json!({
                    "query": "notes.txt",
                    "room": channel_id,
                    "limit": 10
                }),
            )
            .await
            .expect("local search");
        assert_eq!(search.search_results.len(), 1);

        alice
            .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("alice offline");
        bob.execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("bob offline");
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
