use crate::{OriginContext, ServiceActivityItem, ShellSnapshotView, VoxelleCommandHost};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use ts_rs::TS;
use voxelle_core::OriginSurfaceProtocolV1;
use voxelle_update::TrustedReleaseKey;

pub struct ShellState {
    host: Mutex<VoxelleCommandHost>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellCommandPayload {
    Empty,
    Typed(&'static str),
}

pub fn shell_command_payload(command_id: &str) -> Option<ShellCommandPayload> {
    use ShellCommandPayload::{Empty, Typed};

    Some(match command_id {
        "shell.refresh"
        | "home.archiveForRecovery"
        | "runtime.goOffline"
        | "ui.preferences.reset"
        | "workbench.layout.reset"
        | "product.update.check"
        | "product.update.stageAvailable"
        | "product.update.activateStaged"
        | "product.update.discardStaged"
        | "product.update.rollback" => Empty,
        "home.init" => Typed("InitHomeRequest"),
        "runtime.goOnline" => Typed("StartServiceRequest"),
        "space.invite.create" => Typed("CreateSpaceInviteRequest"),
        "space.invite.revoke" => Typed("RevokeSpaceInviteRequest"),
        "space.join" => Typed("JoinSpaceRequest"),
        "identity.recovery.export" => Typed("ExportRecoveryKitRequest"),
        "identity.recovery.restore" => Typed("RestoreRecoveryKitRequest"),
        "message.send" => Typed("SendMessageRequest"),
        "message.acknowledge" => Typed("AcknowledgeMessageRequest"),
        "message.continuation.update" => Typed("UpdateMessageContinuationRequest"),
        "resident.observation.open" => Typed("OpenResidentObservationRequest"),
        "resident.observation.page" => Typed("ResidentChangedThreadsRequest"),
        "resident.observation.commit" => Typed("CommitResidentObservationRequest"),
        "resident.observation.release" => Typed("ReleaseResidentObservationRequest"),
        "channel.select" => Typed("SelectChannelRequest"),
        "message.open" => Typed("OpenMessageRequest"),
        "channel.markRead" => Typed("MarkReadRequest"),
        "channel.create" => Typed("CreateChannelRequest"),
        "channel.rotateKey" => Typed("RotateChannelKeyRequest"),
        "call.join" => Typed("CallJoinRequest"),
        "call.signal" => Typed("CallSignalRequest"),
        "call.media" => Typed("CallMediaRequest"),
        "call.heartbeat" | "call.leave" => Typed("CallLeaveRequest"),
        "message.edit" => Typed("EditMessageRequest"),
        "message.redact" | "pin.add" | "pin.remove" => Typed("MessageTargetRequest"),
        "reaction.add" | "reaction.remove" => Typed("ReactionRequest"),
        "attachment.add" => Typed("AttachmentRequest"),
        "profile.update" => Typed("ProfileUpdateRequest"),
        "role.create" => Typed("CreateRoleRequest"),
        "role.grant" | "role.revoke" => Typed("AssignRoleRequest"),
        "member.ban" | "member.unban" => Typed("BanMemberRequest"),
        "message.search" => Typed("SearchMessagesRequest"),
        "peer.import" => Typed("ImportPeerRecordRequest"),
        "peer.diagnose" | "peer.sync" => Typed("PeerCommandRequest"),
        "ui.preference.set" => Typed("SetUiPreferenceRequest"),
        "workbench.layout.save" => Typed("SetWorkbenchLayoutRequest"),
        "product.update.install" => Typed("InstallProductUpdateRequest"),
        "product.update.rotateTrust" => Typed("InstallTrustTransitionRequest"),
        _ => return None,
    })
}

impl ShellState {
    pub async fn current_device_id(&self) -> Option<String> {
        self.host
            .lock()
            .await
            .snapshot()
            .ok()?
            .home
            .map(|home| home.profile.device_id)
    }

    pub async fn issue_inhabitant_origin_context(
        &self,
        session_capability: &[u8; 32],
        display_label: String,
        request_id: String,
    ) -> ShellResult<OriginContext> {
        self.issue_origin_context(
            session_capability,
            OriginSurfaceProtocolV1::Inhabitant,
            Some(display_label),
            request_id,
        )
        .await
    }

    pub async fn issue_native_webview_origin_context(
        &self,
        session_capability: &[u8; 32],
        request_id: String,
    ) -> ShellResult<OriginContext> {
        self.issue_origin_context(
            session_capability,
            OriginSurfaceProtocolV1::NativeWebview,
            Some("Desktop".to_string()),
            request_id,
        )
        .await
    }

    pub async fn issue_origin_context(
        &self,
        session_capability: &[u8; 32],
        surface_protocol: OriginSurfaceProtocolV1,
        display_label: Option<String>,
        request_id: String,
    ) -> ShellResult<OriginContext> {
        self.host
            .lock()
            .await
            .issue_origin_context(
                session_capability,
                surface_protocol,
                display_label,
                request_id,
            )
            .map_err(|error| {
                ShellError::internal(
                    "Voxelle could not certify this local session.",
                    format!("{error:#}"),
                )
            })
    }

    pub fn new(home_root: impl Into<PathBuf>) -> Self {
        Self {
            host: Mutex::new(VoxelleCommandHost::new(home_root)),
        }
    }

    pub fn new_with_notifier(
        home_root: impl Into<PathBuf>,
        snapshot_invalidated: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            host: Mutex::new(VoxelleCommandHost::new_with_notifier(
                home_root,
                snapshot_invalidated,
            )),
        }
    }

    pub fn new_with_update_keys(
        home_root: impl Into<PathBuf>,
        trusted_update_keys: Vec<TrustedReleaseKey>,
    ) -> Self {
        Self {
            host: Mutex::new(VoxelleCommandHost::new_with_notifier_and_update_keys(
                home_root,
                Arc::new(|| {}),
                trusted_update_keys,
            )),
        }
    }

    pub async fn activity_cursor(&self) -> u64 {
        self.host
            .lock()
            .await
            .activity
            .last()
            .map_or(0, |item| item.id)
    }

    pub async fn activity_items_after(&self, cursor: u64) -> Vec<ServiceActivityItem> {
        self.host
            .lock()
            .await
            .activity
            .iter()
            .filter(|item| item.id > cursor)
            .cloned()
            .collect()
    }

    /// Projects current local meaning without initiating peer synchronization.
    pub async fn observational_snapshot(&self) -> ShellResult<ShellSnapshotView> {
        self.host
            .lock()
            .await
            .snapshot()
            .map_err(|error| ShellError::for_command("shell.refresh", error))
    }

    pub async fn execute_resident_command(
        &self,
        command_id: &str,
        payload: serde_json::Value,
    ) -> Option<ShellResult<serde_json::Value>> {
        let mut host = self.host.lock().await;
        let result: ShellResult<serde_json::Value> = match command_id {
            "resident.observation.open" => {
                parse_request_for(command_id, payload).and_then(|request| {
                    host.open_resident_observation(request)
                        .map_err(|error| ShellError::for_command(command_id, error))
                        .and_then(serialize_resident_result)
                })
            }
            "resident.observation.page" => {
                parse_request_for(command_id, payload).and_then(|request| {
                    host.resident_changed_threads(request)
                        .map_err(|error| ShellError::for_command(command_id, error))
                        .and_then(serialize_resident_result)
                })
            }
            "resident.observation.commit" => {
                parse_request_for(command_id, payload).and_then(|request| {
                    host.commit_resident_observation(request)
                        .map_err(|error| ShellError::for_command(command_id, error))
                        .and_then(serialize_resident_result)
                })
            }
            "resident.observation.release" => {
                parse_request_for(command_id, payload).and_then(|request| {
                    host.release_resident_observation(request)
                        .map_err(|error| ShellError::for_command(command_id, error))
                        .and_then(serialize_resident_result)
                })
            }
            _ => return None,
        };
        Some(result)
    }

    pub async fn execute_serialized_command(
        &self,
        command_id: &str,
        payload: serde_json::Value,
    ) -> ShellResult<ShellSnapshotView> {
        self.execute_serialized_command_inner(command_id, payload, None)
            .await
    }

    pub async fn execute_serialized_command_with_origin(
        &self,
        command_id: &str,
        payload: serde_json::Value,
        origin: OriginContext,
    ) -> ShellResult<ShellSnapshotView> {
        self.execute_serialized_command_inner(command_id, payload, Some(origin))
            .await
    }

    async fn execute_serialized_command_inner(
        &self,
        command_id: &str,
        payload: serde_json::Value,
        origin: Option<OriginContext>,
    ) -> ShellResult<ShellSnapshotView> {
        if command_id == "product.update.check" {
            let manager = {
                let host = self.host.lock().await;
                host.update_transport_context().0
            };
            return match manager.discover_github_release().await {
                Ok(available) => self
                    .host
                    .lock()
                    .await
                    .record_available_product_update(available)
                    .map_err(|error| ShellError::for_command(command_id, error)),
                Err(error) => {
                    self.host
                        .lock()
                        .await
                        .record_product_update_failure(&format!("{error:#}"));
                    Err(ShellError::for_command(command_id, error))
                }
            };
        }
        if command_id == "product.update.stageAvailable" {
            let (manager, available) = {
                let host = self.host.lock().await;
                (
                    host.update_transport_context().0,
                    host.available_product_update()
                        .map_err(|error| ShellError::for_command(command_id, error))?,
                )
            };
            return match manager.download_github_update(available).await {
                Ok(downloaded) => self
                    .host
                    .lock()
                    .await
                    .stage_downloaded_product_update(downloaded)
                    .map_err(|error| ShellError::for_command(command_id, error)),
                Err(error) => {
                    self.host
                        .lock()
                        .await
                        .record_product_update_failure(&format!("{error:#}"));
                    Err(ShellError::for_command(command_id, error))
                }
            };
        }
        let mut host = self.host.lock().await;
        let result = match command_id {
            "shell.refresh" => host.refresh_and_sync().await,
            "home.init" => host.init_home(parse_request(payload)?),
            "home.archiveForRecovery" => host.archive_unusable_home(),
            "runtime.goOnline" => host.start_service_and_sync(parse_request(payload)?).await,
            "runtime.goOffline" => host.stop_service(),
            "space.invite.create" => host.create_space_invite(parse_request(payload)?),
            "space.invite.revoke" => host.revoke_space_invite(parse_request(payload)?).await,
            "space.join" => host.join_space(parse_request(payload)?).await,
            "identity.recovery.export" => host.export_recovery_kit(parse_request(payload)?),
            "identity.recovery.restore" => host.restore_recovery_kit(parse_request(payload)?).await,
            "message.send" => match origin.as_ref() {
                Some(origin) => {
                    host.send_message_with_origin(parse_request(payload)?, origin)
                        .await
                }
                None => host.send_message(parse_request(payload)?).await,
            },
            "message.acknowledge" => match origin.as_ref() {
                Some(origin) => {
                    host.acknowledge_message_with_origin(parse_request(payload)?, origin)
                        .await
                }
                None => host.acknowledge_message(parse_request(payload)?).await,
            },
            "message.continuation.update" => match origin.as_ref() {
                Some(origin) => {
                    host.update_message_continuation_with_origin(
                        parse_request_for(command_id, payload)?,
                        origin,
                    )
                    .await
                }
                None => {
                    host.update_message_continuation(parse_request_for(command_id, payload)?)
                        .await
                }
            },
            "channel.select" => host.select_channel(parse_request(payload)?),
            "message.open" => host.open_message(parse_request(payload)?),
            "channel.markRead" => host.mark_read(parse_request(payload)?),
            "channel.create" => host.create_channel(parse_request(payload)?).await,
            "channel.rotateKey" => host.rotate_channel_key(parse_request(payload)?).await,
            "call.join" => host.join_call(parse_request(payload)?).await,
            "call.signal" => host.signal_call(parse_request(payload)?).await,
            "call.media" => host.update_call_media(parse_request(payload)?).await,
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
            "ui.preferences.reset" => host.reset_all_ui_preferences(),
            "workbench.layout.save" => host.set_workbench_layout(parse_request(payload)?),
            "workbench.layout.reset" => host.reset_workbench_layout(),
            "product.update.install" => host.install_product_update(parse_request(payload)?),
            "product.update.rotateTrust" => {
                host.install_release_trust_transition(parse_request(payload)?)
            }
            "product.update.activateStaged" => host.activate_staged_product_update(),
            "product.update.discardStaged" => host.discard_staged_product_update(),
            "product.update.rollback" => host.rollback_product_update(),
            _ => {
                return Err(ShellError::unknown_command(command_id));
            }
        };
        result.map_err(|error| ShellError::for_command(command_id, error))
    }
}

fn parse_request<T: serde::de::DeserializeOwned>(payload: serde_json::Value) -> ShellResult<T> {
    serde_json::from_value(payload).map_err(|error| ShellError {
        message: "Voxelle could not understand that action.".to_string(),
        recovery: ShellRecovery::InternalError,
        recovery_message:
            "Refresh the workspace and try once more. If it repeats, retain the technical details for a bug report."
                .to_string(),
        detail: format!("invalid command payload: {error}"),
    })
}

fn parse_request_for<T: serde::de::DeserializeOwned>(
    command_id: &str,
    payload: serde_json::Value,
) -> ShellResult<T> {
    serde_json::from_value(payload).map_err(|error| {
        ShellError::for_command(
            command_id,
            anyhow::anyhow!("invalid command payload: {error}"),
        )
    })
}

fn serialize_resident_result<T: serde::Serialize>(value: T) -> ShellResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|error| {
        ShellError::internal(
            "Voxelle could not project resident observation state.",
            format!("serialize resident observation result: {error}"),
        )
    })
}

pub type ShellResult<T> = Result<T, ShellError>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, TS)]
pub struct ShellError {
    pub message: String,
    pub recovery: ShellRecovery,
    pub recovery_message: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShellRecovery {
    NeedsHome,
    NeedsServiceOnline,
    NeedsPeerRecord,
    NeedsReachability,
    NeedsSync,
    NeedsInput,
    NeedsHuman,
    InternalError,
}

impl ShellError {
    pub fn internal(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recovery: ShellRecovery::InternalError,
            recovery_message:
                "Try once more. If it repeats, retain the technical details for a bug report."
                    .to_string(),
            detail: detail.into(),
        }
    }

    fn unknown_command(command_id: &str) -> Self {
        Self::internal(
            "This Voxelle surface requested an unsupported action.",
            format!("unknown command {command_id}"),
        )
    }

    pub(crate) fn for_command(command_id: &str, error: anyhow::Error) -> Self {
        let detail = format!("{error:#}");
        let lower = detail.to_ascii_lowercase();
        let (message, recovery, recovery_message) = command_error_presentation(command_id, &lower);
        Self {
            message: message.to_string(),
            recovery,
            recovery_message: recovery_message.to_string(),
            detail,
        }
    }
}

impl From<anyhow::Error> for ShellError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(
            "Voxelle could not complete that action.",
            format!("{error:#}"),
        )
    }
}

fn command_error_presentation(
    command_id: &str,
    detail: &str,
) -> (&'static str, ShellRecovery, &'static str) {
    if command_id == "shell.refresh" {
        return (
            "Voxelle could not open this local home.",
            ShellRecovery::NeedsHome,
            "Prepare this device for recovery to archive the unusable local state without deleting it, then use your offline recovery kit to preserve the same identity.",
        );
    }
    if command_id == "home.init" {
        return (
            "Voxelle could not create a new local home.",
            ShellRecovery::NeedsHuman,
            "Check that this device has writable storage, then try again. Existing identity files are never overwritten.",
        );
    }
    if detail.contains("identity.json") || detail.contains("active home authority") {
        return (
            "This action needs a healthy local Voxelle home.",
            ShellRecovery::NeedsHome,
            "Finish setup first. If local state was lost or damaged, start with a fresh Voxelle home and use your offline recovery kit.",
        );
    }
    if command_id.starts_with("runtime.") {
        return (
            "Voxelle could not change the connection service.",
            ShellRecovery::NeedsReachability,
            "Open Connection & sync, review the local address state, and try again.",
        );
    }
    if command_id == "space.join" {
        return join_error_presentation(detail);
    }
    if command_id.starts_with("identity.recovery.") {
        return (
            "Voxelle could not complete identity recovery.",
            ShellRecovery::NeedsHuman,
            "Use the original offline recovery kit in a fresh Voxelle home. Keep the kit private and do not edit it.",
        );
    }
    if command_id == "peer.import" {
        return (
            "Voxelle could not import that connection record.",
            ShellRecovery::NeedsPeerRecord,
            "Ask the member for a fresh complete peer record, then import it again. A peer record never grants membership.",
        );
    }
    if command_id == "peer.diagnose" {
        return (
            "Voxelle could not reach that peer.",
            ShellRecovery::NeedsReachability,
            "Open Connection & sync, confirm the peer address, and retry diagnosis.",
        );
    }
    if command_id == "peer.sync" {
        return (
            "Voxelle could not synchronize with that peer.",
            ShellRecovery::NeedsSync,
            "Confirm the peer is reachable and still authorized, then retry synchronization.",
        );
    }
    if command_id == "message.acknowledge"
        && detail
            .to_ascii_lowercase()
            .contains("acknowledgement target is unavailable")
    {
        return (
            "That message is not available on this device yet.",
            ShellRecovery::NeedsSync,
            "Refresh and synchronize with a known peer, then retry. If it remains unavailable, verify the target event ID.",
        );
    }
    if command_id == "message.continuation.update"
        && (detail
            .to_ascii_lowercase()
            .contains("read target is unknown")
            || detail
                .to_ascii_lowercase()
                .contains("superseded fact does not exist"))
    {
        return (
            "That continuation context is not available on this device yet.",
            ShellRecovery::NeedsSync,
            "Refresh and synchronize, then rebuild the update from the current continuation head IDs.",
        );
    }
    if command_id.starts_with("product.update.") {
        return (
            "Voxelle could not complete the signed product update.",
            ShellRecovery::NeedsHuman,
            "Keep the current verified generation active and review Product Update before retrying.",
        );
    }
    if let Some((message, recovery_message)) = correctable_input_presentation(command_id, detail) {
        return (message, ShellRecovery::NeedsInput, recovery_message);
    }
    if detail.contains("service") || detail.contains("offline") {
        return (
            "Voxelle needs its local peer service for that action.",
            ShellRecovery::NeedsServiceOnline,
            "Go online, confirm Connection & sync is healthy, and try again.",
        );
    }
    if detail.contains("permission") || detail.contains("not authorized") {
        return (
            "Your current role does not allow that action.",
            ShellRecovery::NeedsHuman,
            "Ask a space member with the required permission to perform or authorize it.",
        );
    }
    (
        "Voxelle could not complete that action.",
        ShellRecovery::InternalError,
        "Try once more. If it repeats, retain the technical details for a bug report.",
    )
}

fn join_error_presentation(detail: &str) -> (&'static str, ShellRecovery, &'static str) {
    let detail = detail.to_ascii_lowercase();
    let detail = detail.as_str();
    if detail.contains("fresh voxelle home") {
        return (
            "This Voxelle home is already in use.",
            ShellRecovery::NeedsHome,
            "Keep this home intact. Joining another space requires a separate fresh Voxelle home.",
        );
    }
    if detail.contains("revoked by a reachable ordinary peer") {
        return (
            "That invite has been revoked.",
            ShellRecovery::NeedsInput,
            "Ask a current space member for a new signed invite, then review the new space and expiry before joining.",
        );
    }
    if detail.contains("space invite expired") {
        return (
            "That invite has expired.",
            ShellRecovery::NeedsInput,
            "Ask a current space member for a new signed invite, then review its expiry before joining.",
        );
    }
    if detail.contains("space invite")
        || detail.contains("signed bootstrap peer")
        || detail.contains("parse signed space invite json")
    {
        return (
            "That invite cannot be used.",
            ShellRecovery::NeedsInput,
            "Choose the complete signed .voxinvite file again, or ask a current space member to create a new invite.",
        );
    }
    (
        "Voxelle could not complete the join safely.",
        ShellRecovery::InternalError,
        "Your invite was not accepted. Try once more; if it repeats, retain the technical details for a bug report.",
    )
}

fn correctable_input_presentation(
    command_id: &str,
    detail: &str,
) -> Option<(&'static str, &'static str)> {
    let detail = detail.to_ascii_lowercase();
    let detail = detail.as_str();
    let matches = match command_id {
        "message.send" | "message.edit" => {
            detail.contains("message text is invalid")
                || detail.contains("msg_post text is invalid")
                || detail.contains("msg_edit text is invalid")
                || detail.contains("mentions are invalid")
                || detail.contains("thread root does not exist")
                || detail.contains("client_request_id was already used")
                || detail.contains("client_request_id must be")
        }
        "message.acknowledge" => {
            detail.contains("observed msg_ack cannot name a result")
                || detail.contains("msg_ack result must be")
                || detail.contains("msg_ack result does not exist")
                || detail.contains("handled acknowledgement is terminal")
        }
        "message.continuation.update" => {
            detail.contains("msg_continuation")
                || detail.contains("unknown variant")
                || detail.contains("continuing requires a lease")
                || detail.contains("continuation head event id is malformed")
                || detail.contains("continuation updates may supersede")
                || detail.contains("continuation supersedes_event_ids")
                || detail.contains("different continuation payload")
                || detail.contains("client_request_id must be")
        }
        command if command.starts_with("resident.observation.") => {
            detail.contains("resident observation consumer")
                || detail.contains("resident page")
                || detail.contains("resident fact high water")
                || detail.contains("resident commit token")
                || detail.contains("consumer_id")
                || detail.contains("consumer id")
                || detail.contains("consumer limit")
                || detail.contains("room limit")
                || detail.contains("fact sequence")
                || detail.contains("fact high water")
                || detail.contains("start policy")
        }
        "reaction.add" | "reaction.remove" => detail.contains("reaction emoji is invalid"),
        "attachment.add" => {
            detail.contains("decode attachment")
                || detail.contains("attachment metadata is invalid")
                || detail.contains("attachment base64 is invalid")
                || detail.contains("attachment must be 1 to 256 kib")
        }
        "profile.update" => {
            detail.contains("profile display_name is invalid")
                || detail.contains("profile about is invalid")
        }
        "channel.create" => {
            detail.contains("channel name must contain a letter or number")
                || detail.contains("private channel members must already belong to the space")
                || detail.contains("invalid channel definition")
        }
        "role.create" => {
            detail.contains("role name must contain a letter or number")
                || detail.contains("invalid role definition")
        }
        "space.invite.create" => {
            detail.contains("invite expiry must be between 1 minute and 30 days")
        }
        "message.search" => {
            detail.starts_with("search query is ")
                || detail.starts_with("search query exceeds ")
                || detail.starts_with("search query contains ")
        }
        _ => false,
    };
    if !matches {
        return None;
    }
    Some(match command_id {
        "message.send" | "message.edit" => (
            "That message needs editing.",
            "Correct the message payload. For a retry, reuse a client request ID only with its identical original message; otherwise generate a new ID.",
        ),
        "message.acknowledge" => (
            "That acknowledgement cannot use this result.",
            "Use no result for Observed. For Handled, choose your own visible message in this room that replies to the target; an existing handled result cannot be rebound.",
        ),
        "message.continuation.update" => (
            "That continuation update needs correcting.",
            "For Continuing, use a bounded lease from 1 minute through 7 days. Release and Decline have no lease. Reconcile current head IDs before replacing an earlier update.",
        ),
        command if command.starts_with("resident.observation.") => (
            "That resident observation request needs correcting.",
            "Use a stable bounded consumer ID. Open it with an explicit start policy, fetch every served page in order, and commit only the final page's matching token and fact high water. After a process restart, begin paging again rather than reusing a page token.",
        ),
        "reaction.add" | "reaction.remove" => (
            "That reaction is not valid.",
            "Choose a visible emoji or short reaction of at most 32 characters and try again.",
        ),
        "attachment.add" => (
            "That file cannot be attached.",
            "Choose a non-empty file no larger than 256 KiB with a valid filename, then try again.",
        ),
        "profile.update" => (
            "Those profile details are not valid.",
            "Use a display name of 1 to 80 characters and an About description of at most 512 characters.",
        ),
        "channel.create" => (
            "That channel cannot be created as entered.",
            "Use a name containing a letter or number and choose only current space members for a private channel.",
        ),
        "role.create" => (
            "That role cannot be created as entered.",
            "Use a name containing a letter or number and choose at least one supported permission.",
        ),
        "space.invite.create" => (
            "That invite expiry is not valid.",
            "Choose an expiry from 1 minute through 30 days, then create the signed invite again.",
        ),
        "message.search" if detail.contains("search query is empty") => (
            "Enter something to search for.",
            "Type one or more words from a message or attachment name, then search again.",
        ),
        "message.search" => (
            "That search needs editing.",
            "Use at most 1,024 characters without control characters, then search again.",
        ),
        _ => unreachable!("matched correctable input command"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builtin_product_generation, default_ui_ontology, shell_contract_typescript,
        NetworkHealthStatus, ProductGenerationV1, UiPreferences, DEFAULT_ROOM_ID,
        MAX_INVITE_EXPIRY_MINUTES, MAX_SEARCH_QUERY_CHARACTERS,
    };
    use std::collections::BTreeMap;
    use voxelle_core::Keypair;
    use voxelle_update::{
        package_signing_bytes, trust_transition_signing_bytes, ReleaseKeyRole, TrustTransitionV1,
        TrustedReleaseKey, UpdatePackageV1, TRUST_TRANSITION_FORMAT_V1, UPDATE_FORMAT_V1,
    };

    fn signed_generation_package(key: &Keypair, sequence: u64, refresh_label: &str) -> String {
        let mut ontology = default_ui_ontology(UiPreferences::default());
        ontology
            .commands
            .iter_mut()
            .find(|command| command.id == "shell.refresh")
            .expect("refresh command")
            .label = refresh_label.to_string();
        let mut component = builtin_product_generation().component;
        component
            .source
            .push_str(&format!("\n// signed generation: {refresh_label}\n"));
        let payload = serde_json::to_value(ProductGenerationV1 {
            v: 1,
            ontology,
            component,
        })
        .expect("generation payload");
        let mut package = UpdatePackageV1 {
            format: UPDATE_FORMAT_V1.to_string(),
            release_id: format!("beta-{sequence}"),
            sequence,
            channel: "beta".to_string(),
            min_kernel_version: "0.1.0".to_string(),
            payload,
            signer_key_id: key.id.clone(),
            signature_b64: String::new(),
        };
        package.signature_b64 = key.sign(&package_signing_bytes(&package).expect("signing bytes"));
        serde_json::to_string_pretty(&package).expect("package JSON")
    }

    fn trusted_key(key: &Keypair) -> TrustedReleaseKey {
        TrustedReleaseKey {
            key_id: key.id.clone(),
            spki_b64: key.spki_b64.clone(),
            role: ReleaseKeyRole::Release,
        }
    }

    fn trusted_recovery_key(key: &Keypair) -> TrustedReleaseKey {
        TrustedReleaseKey {
            key_id: key.id.clone(),
            spki_b64: key.spki_b64.clone(),
            role: ReleaseKeyRole::Recovery,
        }
    }

    fn signed_trust_rotation(old: &Keypair, new: &Keypair) -> String {
        let mut transition = TrustTransitionV1 {
            format: TRUST_TRANSITION_FORMAT_V1.to_string(),
            sequence: 1,
            add: vec![trusted_key(new)],
            remove_key_ids: vec![old.id.clone()],
            signer_key_id: old.id.clone(),
            signature_b64: String::new(),
        };
        transition.signature_b64 = old.sign(
            &trust_transition_signing_bytes(&transition).expect("trust transition signing bytes"),
        );
        serde_json::to_string_pretty(&transition).expect("trust transition JSON")
    }

    #[tokio::test]
    async fn shell_state_returns_pre_init_snapshot_for_web_shell() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));

        let snapshot = shell
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("snapshot");

        assert!(snapshot.home.is_none());
        assert!(snapshot.home_error.is_none());
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
    async fn damaged_home_can_be_archived_then_recover_the_same_principal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kit_path = dir.path().join("identity.voxrecover");
        let original = ShellState::new(dir.path().join("original"));
        let initialized = original
            .execute_serialized_command("home.init", serde_json::json!({"default_room": null}))
            .await
            .expect("initialize source");
        let original_home = initialized.home.expect("initialized home");
        original
            .execute_serialized_command(
                "identity.recovery.export",
                serde_json::json!({"path": kit_path}),
            )
            .await
            .expect("export recovery kit");
        original
            .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("stop healthy source");
        assert!(original
            .execute_serialized_command("home.archiveForRecovery", serde_json::json!({}))
            .await
            .is_err());
        assert!(dir.path().join("original/identity.json").exists());

        let damaged_root = dir.path().join("damaged");
        std::fs::create_dir_all(&damaged_root).expect("damaged root");
        std::fs::write(damaged_root.join("identity.json"), b"{not-json").expect("corrupt identity");
        std::fs::create_dir_all(damaged_root.join("product-updates"))
            .expect("product update state");
        std::fs::write(
            damaged_root.join("product-updates/trust-marker"),
            b"preserved",
        )
        .expect("product update marker");
        let damaged = ShellState::new(&damaged_root);
        let before = damaged
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("damaged snapshot");
        let error = before.home_error.expect("structured damage error");
        assert_eq!(error.recovery, ShellRecovery::NeedsHome);
        assert!(!error.detail.is_empty());
        assert_ne!(error.detail, error.message);

        let prepared = damaged
            .execute_serialized_command("home.archiveForRecovery", serde_json::json!({}))
            .await
            .expect("archive damaged local state");
        assert!(prepared.home.is_none());
        assert!(prepared.home_error.is_none());
        assert!(!damaged_root.join("identity.json").exists());
        let archive = std::fs::read_dir(&damaged_root)
            .expect("archive listing")
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".unusable-home-")
            })
            .expect("private archive");
        assert!(archive.path().join("identity.json").exists());
        assert_eq!(
            std::fs::read(damaged_root.join("product-updates/trust-marker"))
                .expect("preserved product update state"),
            b"preserved"
        );

        let recovered = damaged
            .execute_serialized_command(
                "identity.recovery.restore",
                serde_json::json!({"path": kit_path, "max_events_per_peer": 4096}),
            )
            .await
            .expect("restore same identity");
        let recovered_home = recovered.home.expect("recovered home");
        assert_eq!(
            recovered_home.profile.peer_id,
            original_home.profile.peer_id
        );
        assert_ne!(
            recovered_home.profile.device_id,
            original_home.profile.device_id
        );
    }

    #[tokio::test]
    async fn serialized_recovery_commands_preserve_principal_and_rotate_device() {
        let dir = tempfile::tempdir().expect("tempdir");
        let original = ShellState::new(dir.path().join("original"));
        let initialized = original
            .execute_serialized_command("home.init", serde_json::json!({"default_room": null}))
            .await
            .expect("initialize original home");
        let initialized_home = initialized.home.expect("original home");
        assert_eq!(initialized_home.runtime.state, crate::RuntimeState::Online);
        let original_profile = initialized_home.profile;
        let kit_path = dir.path().join("offline.voxrecover");

        let exported = original
            .execute_serialized_command(
                "identity.recovery.export",
                serde_json::json!({"path": kit_path}),
            )
            .await
            .expect("export recovery kit");
        let exported_health = exported.home.expect("exported home").recovery;
        assert!(exported_health.kit_exported);
        assert!(exported_health.last_exported_ms.is_some());

        let reopened = ShellState::new(dir.path().join("original"))
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("reopen recovery health");
        assert!(reopened.home.expect("reopened home").recovery.kit_exported);

        let recovered = ShellState::new(dir.path().join("recovered"))
            .execute_serialized_command(
                "identity.recovery.restore",
                serde_json::json!({
                    "path": kit_path,
                    "max_events_per_peer": 64,
                }),
            )
            .await
            .expect("recover through serialized shell command");
        let recovered_home = recovered.home.expect("recovered home");
        assert_eq!(recovered_home.profile.peer_id, original_profile.peer_id);
        assert_ne!(recovered_home.profile.device_id, original_profile.device_id);
        assert_eq!(recovered_home.runtime.state, crate::RuntimeState::Online);
        assert!(!recovered_home.recovery.kit_exported);
        assert!(recovered
            .service_activity
            .iter()
            .any(|item| item.summary.contains("recovered identity onto device")));
    }

    #[tokio::test]
    async fn serialized_shell_reopens_an_initialized_home_without_stalling() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let original = ShellState::new(&home);
        original
            .execute_serialized_command("home.init", serde_json::json!({"default_room": null}))
            .await
            .expect("initialize home");
        original
            .execute_serialized_command(
                "message.send",
                serde_json::json!({"text": "persists through restart", "room": null}),
            )
            .await
            .expect("send message");
        drop(original);

        let reopened = ShellState::new(&home);
        let snapshot = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            reopened.execute_serialized_command("shell.refresh", serde_json::json!({})),
        )
        .await
        .expect("reopened shell refresh should not stall")
        .expect("refresh reopened home");

        let reopened_home = snapshot.home.expect("reopened home");
        assert_eq!(reopened_home.runtime.state, crate::RuntimeState::Offline);
        assert_eq!(
            reopened_home.room.messages[0].text,
            "persists through restart"
        );
    }

    #[tokio::test]
    async fn signed_product_generation_activates_live_persists_and_rolls_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let key = Keypair::generate().expect("release key");
        let shell = ShellState::new_with_update_keys(&home, vec![trusted_key(&key)]);

        shell
            .execute_serialized_command("home.init", serde_json::json!({"default_room": null}))
            .await
            .expect("initialize home");
        shell
            .execute_serialized_command(
                "message.send",
                serde_json::json!({"text": "survives live generation", "room": null}),
            )
            .await
            .expect("send message");
        shell
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({"bind": "[::1]:0", "advertise": null}),
            )
            .await
            .expect("start service");

        let first = shell
            .execute_serialized_command(
                "product.update.install",
                serde_json::json!({
                    "package_json": signed_generation_package(&key, 1, "Refresh Live")
                }),
            )
            .await
            .expect("activate first generation");
        assert_eq!(first.product_generation.active_sequence, 1);
        assert!(first
            .product_component
            .source
            .contains("signed generation: Refresh Live"));
        assert_eq!(
            first
                .ui_ontology
                .commands
                .iter()
                .find(|command| command.id == "shell.refresh")
                .expect("refresh command")
                .label,
            "Refresh Live"
        );
        let home_view = first.home.expect("initialized home");
        assert_eq!(home_view.runtime.state, crate::RuntimeState::Online);
        assert!(home_view
            .room
            .messages
            .iter()
            .any(|message| message.text == "survives live generation"));
        shell
            .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("stop service");
        drop(shell);

        let restarted = ShellState::new_with_update_keys(&home, vec![trusted_key(&key)]);
        let persisted = restarted
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("reload persisted generation");
        assert_eq!(persisted.product_generation.active_sequence, 1);
        assert!(persisted
            .product_component
            .source
            .contains("signed generation: Refresh Live"));
        assert_eq!(
            persisted
                .ui_ontology
                .commands
                .iter()
                .find(|command| command.id == "shell.refresh")
                .expect("refresh command")
                .label,
            "Refresh Live"
        );

        restarted
            .execute_serialized_command(
                "product.update.install",
                serde_json::json!({
                    "package_json": signed_generation_package(&key, 2, "Refresh Beta Two")
                }),
            )
            .await
            .expect("activate second generation");
        let rolled_back = restarted
            .execute_serialized_command("product.update.rollback", serde_json::json!({}))
            .await
            .expect("rollback generation");
        assert_eq!(rolled_back.product_generation.active_sequence, 1);
        assert!(rolled_back
            .product_component
            .source
            .contains("signed generation: Refresh Live"));
        assert_eq!(
            rolled_back
                .ui_ontology
                .commands
                .iter()
                .find(|command| command.id == "shell.refresh")
                .expect("refresh command")
                .label,
            "Refresh Live"
        );
    }

    #[tokio::test]
    async fn signed_release_trust_rotation_persists_and_changes_update_authority() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let old = Keypair::generate().expect("old release key");
        let new = Keypair::generate().expect("new release key");
        let recovery = Keypair::generate().expect("recovery key");
        let roots = vec![trusted_key(&old), trusted_recovery_key(&recovery)];
        let shell = ShellState::new_with_update_keys(&home, roots.clone());
        let rotated = shell
            .execute_serialized_command(
                "product.update.rotateTrust",
                serde_json::json!({"transition_json": signed_trust_rotation(&old, &new)}),
            )
            .await
            .expect("rotate release trust");
        assert_eq!(rotated.product_generation.trust_sequence, 1);
        assert_eq!(rotated.product_generation.trusted_update_key_count, 2);
        drop(shell);

        let restarted = ShellState::new_with_update_keys(&home, roots);
        let rejected = restarted
            .execute_serialized_command(
                "product.update.install",
                serde_json::json!({
                    "package_json": signed_generation_package(&old, 1, "Old signer")
                }),
            )
            .await
            .expect_err("retired signer rejected");
        assert_eq!(rejected.recovery, ShellRecovery::NeedsHuman);
        assert!(rejected.detail.contains("not trusted"));
        let accepted = restarted
            .execute_serialized_command(
                "product.update.install",
                serde_json::json!({
                    "package_json": signed_generation_package(&new, 1, "New signer")
                }),
            )
            .await
            .expect("new signer accepted");
        assert_eq!(accepted.product_generation.active_sequence, 1);
        assert_eq!(accepted.product_generation.trust_sequence, 1);
    }

    #[tokio::test]
    async fn generation_activation_serializes_with_an_in_flight_product_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let key = Keypair::generate().expect("release key");
        let shell = Arc::new(ShellState::new_with_update_keys(
            &home,
            vec![trusted_key(&key)],
        ));
        shell
            .execute_serialized_command("home.init", serde_json::json!({"default_room": null}))
            .await
            .expect("initialize home");

        let sending = {
            let shell = Arc::clone(&shell);
            async move {
                shell
                    .execute_serialized_command(
                        "message.send",
                        serde_json::json!({"text": "concurrent fact", "room": null}),
                    )
                    .await
            }
        };
        let activating = {
            let shell = Arc::clone(&shell);
            async move {
                shell
                    .execute_serialized_command(
                        "product.update.install",
                        serde_json::json!({
                            "package_json": signed_generation_package(
                                &key,
                                1,
                                "Concurrent Generation",
                            )
                        }),
                    )
                    .await
            }
        };
        let (sent, activated) = tokio::join!(sending, activating);
        sent.expect("message command");
        activated.expect("activation command");
        let final_snapshot = shell
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("final snapshot");
        assert_eq!(final_snapshot.product_generation.active_sequence, 1);
        assert!(final_snapshot
            .home
            .expect("home")
            .room
            .messages
            .iter()
            .any(|message| message.text == "concurrent fact"));
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
        let joined_home = bob_joined.home.expect("home");
        assert_eq!(joined_home.room.messages[0].text, "hello through shell");
        let initial_event_id = joined_home.room.messages[0].event_id.clone();
        bob.execute_serialized_command(
            "message.acknowledge",
            serde_json::json!({
                "target_event_id": initial_event_id,
                "room": null,
                "state": "handled"
            }),
        )
        .await
        .expect("bob acknowledges");
        let acknowledged = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice receives acknowledgement");
        assert_eq!(
            acknowledged.home.expect("alice home").room.messages[0].acknowledgements[0].state,
            crate::MessageAcknowledgementState::Handled
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

        bob.execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("bob offline");
        alice
            .execute_serialized_command(
                "message.send",
                serde_json::json!({ "text": "catch up on reconnect", "room": null }),
            )
            .await
            .expect("send while bob offline");
        let bob_reconnected = bob
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({ "bind": null, "advertise": null }),
            )
            .await
            .expect("reconnect catches up");
        assert!(bob_reconnected
            .home
            .expect("bob home")
            .room
            .messages
            .iter()
            .any(|message| message.text == "catch up on reconnect"));

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
    async fn serialized_join_reports_revoked_invite_without_creating_a_home() {
        let dir = tempfile::tempdir().expect("tempdir");
        let alice = ShellState::new(dir.path().join("alice"));
        let charlie = ShellState::new(dir.path().join("charlie"));

        alice
            .execute_serialized_command("home.init", serde_json::json!({ "default_room": null }))
            .await
            .expect("alice init");
        alice
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({ "bind": null, "advertise": null }),
            )
            .await
            .expect("alice online");
        let created = alice
            .execute_serialized_command(
                "space.invite.create",
                serde_json::json!({ "expires_minutes": 60 }),
            )
            .await
            .expect("create invite");
        let invite_json = created
            .home
            .expect("home")
            .invite
            .expect("invite exchange")
            .space_invite_json
            .expect("invite JSON");
        let invite: crate::SpaceInviteFileV1 =
            serde_json::from_str(&invite_json).expect("parse invite");
        alice
            .execute_serialized_command(
                "space.invite.revoke",
                serde_json::json!({ "invite_id": invite.invite_event.event_id }),
            )
            .await
            .expect("revoke invite");

        let error = charlie
            .execute_serialized_command(
                "space.join",
                serde_json::json!({
                    "space_invite_json": invite_json,
                    "max_events": 64
                }),
            )
            .await
            .expect_err("revoked invite refused");
        assert_eq!(error.recovery, ShellRecovery::NeedsInput);
        assert_eq!(error.message, "That invite has been revoked.");
        assert!(error.recovery_message.contains("new signed invite"));

        let fresh = charlie
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("joiner remains fresh");
        assert!(fresh.home.is_none());
        assert!(fresh.home_error.is_none());
    }

    #[tokio::test]
    async fn shell_invite_uses_an_ordinary_peer_while_inviter_is_offline() {
        let dir = tempfile::tempdir().expect("tempdir");
        let alice = ShellState::new(dir.path().join("alice-inviter"));
        let bob = ShellState::new(dir.path().join("bob-ordinary-peer"));
        let charlie = ShellState::new(dir.path().join("charlie-fresh"));

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
                serde_json::json!({ "text": "history retained by Bob", "room": null }),
            )
            .await
            .expect("alice history");
        alice
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({ "bind": null, "advertise": null }),
            )
            .await
            .expect("alice online");

        let bob_invite = alice
            .execute_serialized_command(
                "space.invite.create",
                serde_json::json!({ "expires_minutes": 60 }),
            )
            .await
            .expect("invite Bob")
            .home
            .expect("alice home")
            .invite
            .expect("alice invite")
            .space_invite_json
            .expect("signed Bob invite");
        let bob_joined = bob
            .execute_serialized_command(
                "space.join",
                serde_json::json!({
                    "space_invite_json": bob_invite,
                    "max_events": 64
                }),
            )
            .await
            .expect("Bob joins");
        let bob_record_json = bob_joined
            .home
            .expect("bob home")
            .invite
            .expect("bob online invite exchange")
            .peer_record_json;
        alice
            .execute_serialized_command(
                "peer.import",
                serde_json::json!({ "peer_record_json": bob_record_json }),
            )
            .await
            .expect("Alice imports Bob availability");

        let charlie_invite = alice
            .execute_serialized_command(
                "space.invite.create",
                serde_json::json!({ "expires_minutes": 60 }),
            )
            .await
            .expect("invite Charlie with ordinary fallback")
            .home
            .expect("alice home")
            .invite
            .expect("alice invite")
            .space_invite_json
            .expect("signed Charlie invite");
        let parsed: crate::SpaceInviteFileV1 =
            serde_json::from_str(&charlie_invite).expect("parse Charlie invite");
        assert_eq!(parsed.bootstrap_peers().expect("bootstrap peers").len(), 2);

        alice
            .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("inviter offline");
        let charlie_joined = charlie
            .execute_serialized_command(
                "space.join",
                serde_json::json!({
                    "space_invite_json": charlie_invite,
                    "max_events": 64
                }),
            )
            .await
            .expect("Charlie joins through Bob");
        assert!(charlie_joined
            .home
            .expect("charlie home")
            .room
            .messages
            .iter()
            .any(|message| message.text == "history retained by Bob"));

        bob.execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("Bob offline");
        charlie
            .execute_serialized_command("runtime.goOffline", serde_json::json!({}))
            .await
            .expect("Charlie offline");
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
        assert_eq!(bob_call.participant_video.get(&alice_peer_id), Some(&false));
        assert_eq!(bob_call.participant_video.get(&bob_peer_id), Some(&true));
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
        let alice_after_heartbeat = alice
            .execute_serialized_command(
                "call.heartbeat",
                serde_json::json!({ "room": null, "call_id": call_id.clone() }),
            )
            .await
            .expect("alice heartbeat");
        let heartbeat_call = alice_after_heartbeat.home.expect("alice home").call;
        assert_eq!(
            heartbeat_call.participant_video.get(&alice_peer_id),
            Some(&false)
        );
        assert_eq!(
            heartbeat_call.participant_video.get(&bob_peer_id),
            Some(&true)
        );
        bob.execute_serialized_command(
            "call.media",
            serde_json::json!({
                "room": null,
                "call_id": call_id.clone(),
                "video": false
            }),
        )
        .await
        .expect("bob turns camera off");
        let alice_after_media = alice
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("alice sees bob camera state");
        assert_eq!(
            alice_after_media
                .home
                .expect("alice home")
                .call
                .participant_video
                .get(&bob_peer_id),
            Some(&false)
        );
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
        let call_after_leave = alice_after_leave.home.expect("alice home").call;
        assert_eq!(call_after_leave.participants, vec![alice_peer_id.clone()]);
        assert_eq!(
            call_after_leave.participant_video,
            BTreeMap::from([(alice_peer_id.clone(), false)])
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
        let selected_unread = bob
            .execute_serialized_command(
                "channel.select",
                serde_json::json!({ "room_id": channel_id }),
            )
            .await
            .expect("open notification channel");
        assert_eq!(
            selected_unread
                .home
                .expect("home after selection")
                .channels
                .iter()
                .find(|channel| channel.room_id == channel_id)
                .expect("engineering channel")
                .unread_count,
            1
        );
        let marked_read = bob
            .execute_serialized_command(
                "channel.markRead",
                serde_json::json!({ "room_id": channel_id }),
            )
            .await
            .expect("mark notification channel read");
        let marked_read_home = marked_read.home.expect("home");
        assert_eq!(
            marked_read_home
                .channels
                .iter()
                .find(|channel| channel.room_id == channel_id)
                .expect("engineering channel")
                .unread_count,
            0
        );
        assert!(marked_read_home.notifications.is_empty());

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
                    "role_id": role_id.clone()
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
        assert!(final_home
            .room
            .messages
            .iter()
            .any(|message| message
                .attachments
                .iter()
                .any(|attachment| attachment.filename == "notes.txt"
                    && attachment.sha256.starts_with("sha256:")
                    && attachment.size_bytes == 11)));
        assert!(final_home
            .profiles
            .iter()
            .any(|profile| profile.display_name == "Bob Builder"));
        let bob_profile = final_home
            .profiles
            .iter()
            .find(|profile| profile.display_name == "Bob Builder")
            .expect("Bob profile");
        assert!(!bob_profile.banned);
        assert!(bob_profile.role_ids.contains(&role_id));

        let revoked = alice
            .execute_serialized_command(
                "role.revoke",
                serde_json::json!({
                    "peer_id": bob_peer_id,
                    "role_id": role_id
                }),
            )
            .await
            .expect("revoke role");
        assert!(!revoked
            .home
            .expect("home after revoke")
            .profiles
            .iter()
            .find(|profile| profile.peer_id == bob_peer_id)
            .expect("Bob after revoke")
            .role_ids
            .contains(&role_id));
        alice
            .execute_serialized_command(
                "role.grant",
                serde_json::json!({
                    "peer_id": bob_peer_id,
                    "role_id": role_id
                }),
            )
            .await
            .expect("regrant role before ban");

        let banned = alice
            .execute_serialized_command(
                "member.ban",
                serde_json::json!({
                    "peer_id": bob_peer_id,
                    "reason": "serialized governance test"
                }),
            )
            .await
            .expect("ban member");
        let banned_home = banned.home.expect("home");
        let banned_bob = banned_home
            .profiles
            .iter()
            .find(|profile| profile.display_name == "Bob Builder")
            .expect("banned Bob profile");
        assert!(banned_bob.banned);
        assert!(banned_bob.role_ids.is_empty());
        let unbanned = alice
            .execute_serialized_command(
                "member.unban",
                serde_json::json!({
                    "peer_id": bob_peer_id,
                    "reason": "serialized governance test complete"
                }),
            )
            .await
            .expect("unban member");
        assert!(!unbanned
            .home
            .expect("home")
            .profiles
            .iter()
            .any(|profile| profile.peer_id == bob_peer_id));

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
        let search_event_id = search.search_results[0].message.event_id.clone();
        let opened = alice
            .execute_serialized_command(
                "message.open",
                serde_json::json!({
                    "room_id": channel_id,
                    "event_id": search_event_id
                }),
            )
            .await
            .expect("open retained search result");
        let opened_home = opened.home.expect("home after opening search result");
        assert_eq!(opened_home.room.room_id, channel_id);
        assert!(opened_home
            .room
            .messages
            .iter()
            .any(|message| message.event_id == search_event_id));

        let attachment_redacted = alice
            .execute_serialized_command(
                "message.redact",
                serde_json::json!({
                    "target_event_id": search_event_id,
                    "room": channel_id
                }),
            )
            .await
            .expect("attachment tombstone");
        let redacted_attachment = attachment_redacted
            .home
            .expect("home after attachment tombstone")
            .room
            .messages
            .into_iter()
            .find(|message| message.event_id == search_event_id)
            .expect("redacted attachment projection");
        assert!(redacted_attachment.redacted);
        assert!(redacted_attachment.attachments.is_empty());

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

        assert_eq!(error.recovery, ShellRecovery::NeedsHome);
        assert!(!error.message.contains("identity.json"));
        assert!(error.detail.contains("identity.json"));
        let encoded = serde_json::to_string(&error).expect("serialize");
        assert!(encoded.contains("identity.json"));
        assert!(encoded.contains("needs_home"));
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

        let home = snapshot.home.expect("home");
        assert_eq!(home.room.messages[0].text, "serialized shell command");
        let local_peer_id = home.profile.peer_id;
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
        let reset = shell
            .execute_serialized_command("ui.preferences.reset", serde_json::json!({}))
            .await
            .expect("reset customization");
        assert_eq!(metric_value(&reset, "sidebar.width"), 360.0);

        let private = shell
            .execute_serialized_command(
                "channel.create",
                serde_json::json!({
                    "name": "Private notes",
                    "topic": "Rotation contract",
                    "private_members": [local_peer_id]
                }),
            )
            .await
            .expect("create private channel");
        let private_room_id = private
            .home
            .expect("home")
            .channels
            .into_iter()
            .find(|channel| channel.name == "Private notes")
            .expect("private channel")
            .room_id;
        let rotated = shell
            .execute_serialized_command(
                "channel.rotateKey",
                serde_json::json!({ "room_id": private_room_id }),
            )
            .await
            .expect("rotate through semantic command");
        let rotated_channel = rotated
            .home
            .expect("home")
            .channels
            .into_iter()
            .find(|channel| channel.name == "Private notes")
            .expect("rotated private channel");
        assert_eq!(rotated_channel.key_epoch, 2);
        assert_eq!(rotated_channel.private_member_count, 1);

        let reopened = ShellState::new(dir.path().join("home"));
        assert_eq!(
            metric_value(
                &reopened
                    .execute_serialized_command("shell.refresh", serde_json::json!({}))
                    .await
                    .expect("reopened snapshot"),
                "sidebar.width"
            ),
            360.0
        );
        let reopened_snapshot = reopened
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("reopened channel projection");
        let reopened_channel = reopened_snapshot
            .home
            .expect("home")
            .channels
            .into_iter()
            .find(|channel| channel.name == "Private notes")
            .expect("reopened private channel");
        assert_eq!(reopened_channel.key_epoch, 2);
        assert_eq!(reopened_channel.private_member_count, 1);
        let unknown = shell
            .execute_serialized_command("not_a_command", serde_json::json!({}))
            .await
            .expect_err("unknown command");
        assert_eq!(unknown.recovery, ShellRecovery::InternalError);
        assert_eq!(unknown.detail, "unknown command not_a_command");
        let invalid = shell
            .execute_serialized_command("message.send", serde_json::json!({}))
            .await
            .expect_err("invalid payload");
        assert_eq!(invalid.recovery, ShellRecovery::InternalError);
        assert!(invalid.detail.starts_with("invalid command payload:"));

        let empty_search = shell
            .execute_serialized_command(
                "message.search",
                serde_json::json!({ "query": "   ", "room": null, "limit": 10 }),
            )
            .await
            .expect_err("empty search rejected");
        assert_eq!(empty_search.recovery, ShellRecovery::NeedsInput);
        assert_eq!(empty_search.message, "Enter something to search for.");
        assert!(empty_search.recovery_message.contains("one or more words"));
        assert!(empty_search.detail.contains("search query is empty"));

        let oversized_search = shell
            .execute_serialized_command(
                "message.search",
                serde_json::json!({
                    "query": "😀".repeat(MAX_SEARCH_QUERY_CHARACTERS + 1),
                    "room": null,
                    "limit": 10
                }),
            )
            .await
            .expect_err("oversized search rejected");
        assert_eq!(oversized_search.recovery, ShellRecovery::NeedsInput);
        assert_eq!(oversized_search.message, "That search needs editing.");
        assert!(oversized_search
            .recovery_message
            .contains("1,024 characters"));
        assert!(oversized_search.detail.contains("search query exceeds"));

        let control_search = shell
            .execute_serialized_command(
                "message.search",
                serde_json::json!({ "query": "hello\nworld", "room": null, "limit": 10 }),
            )
            .await
            .expect_err("control-character search rejected");
        assert_eq!(control_search.recovery, ShellRecovery::NeedsInput);
        assert_eq!(control_search.message, "That search needs editing.");
        assert!(control_search
            .recovery_message
            .contains("control characters"));
        assert!(control_search.detail.contains("search query contains"));

        shell
            .execute_serialized_command(
                "runtime.goOnline",
                serde_json::json!({ "bind": null, "advertise": null }),
            )
            .await
            .expect("service online for invite validation");
        for invalid_minutes in [0, MAX_INVITE_EXPIRY_MINUTES + 1] {
            let invalid_invite = shell
                .execute_serialized_command(
                    "space.invite.create",
                    serde_json::json!({ "expires_minutes": invalid_minutes }),
                )
                .await
                .expect_err("out-of-range invite expiry rejected");
            assert_eq!(invalid_invite.recovery, ShellRecovery::NeedsInput);
            assert_eq!(invalid_invite.message, "That invite expiry is not valid.");
            assert!(invalid_invite
                .recovery_message
                .contains("1 minute through 30 days"));
            assert!(invalid_invite
                .detail
                .contains("invite expiry must be between"));
        }
        let after_invalid_invites = shell
            .execute_serialized_command("shell.refresh", serde_json::json!({}))
            .await
            .expect("snapshot after invalid invites");
        assert!(after_invalid_invites
            .home
            .expect("home")
            .active_invites
            .is_empty());
    }

    #[test]
    fn correctable_input_classification_does_not_absorb_authority_or_internal_errors() {
        assert!(correctable_input_presentation(
            "channel.create",
            "channel name must contain a letter or number"
        )
        .is_some());
        assert!(correctable_input_presentation(
            "attachment.add",
            "event rejected: attachment must be 1 to 256 KiB"
        )
        .is_some());
        assert!(correctable_input_presentation(
            "message.send",
            "client_request_id was already used for a different message payload"
        )
        .is_some());
        assert!(correctable_input_presentation(
            "message.acknowledge",
            "event rejected: Invalid(\"observed MSG_ACK cannot name a result\")"
        )
        .is_some());
        assert!(correctable_input_presentation(
            "message.continuation.update",
            "unknown variant `working`, expected one of `continuing`, `released`, `declined`"
        )
        .is_some());
        assert!(correctable_input_presentation(
            "message.continuation.update",
            "continuation head event ID is malformed"
        )
        .is_some());
        assert!(correctable_input_presentation("member.ban", "not authorized").is_none());
        assert!(correctable_input_presentation("message.send", "database is locked").is_none());
    }

    #[tokio::test]
    async fn invalid_continuation_state_is_correctable_input() {
        let temp = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(temp.path());
        let error = shell
            .execute_serialized_command(
                "message.continuation.update",
                serde_json::json!({
                    "target_event_id": "e:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "room": null,
                    "state": "working",
                    "lease_ms": 60_000,
                    "supersedes_event_ids": [],
                    "client_request_id": "invalid-state-test"
                }),
            )
            .await
            .expect_err("unknown state must be rejected");
        assert_eq!(error.recovery, ShellRecovery::NeedsInput);
        assert!(error.detail.contains("unknown variant"));
    }

    #[test]
    fn join_failures_name_the_recovery_that_matches_the_authoritative_cause() {
        assert_eq!(
            join_error_presentation(
                "space invite was revoked by a reachable ordinary peer; no local home was created"
            ),
            (
                "That invite has been revoked.",
                ShellRecovery::NeedsInput,
                "Ask a current space member for a new signed invite, then review the new space and expiry before joining.",
            )
        );
        assert_eq!(
            join_error_presentation("space invite expired").0,
            "That invite has expired."
        );
        assert_eq!(
            join_error_presentation("parse signed space invite JSON: expected value").0,
            "That invite cannot be used."
        );
        assert_eq!(
            join_error_presentation("joining a space requires a fresh Voxelle home").1,
            ShellRecovery::NeedsHome
        );
        assert_eq!(
            join_error_presentation("database write failed").1,
            ShellRecovery::InternalError
        );
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

    #[test]
    fn every_shell_command_names_an_exported_or_empty_payload_contract() {
        let contract = shell_contract_typescript();
        for command_id in crate::shell_command_ids() {
            let payload = shell_command_payload(&command_id)
                .unwrap_or_else(|| panic!("missing payload contract for {command_id}"));
            if let ShellCommandPayload::Typed(name) = payload {
                assert!(
                    contract.contains(&format!("export type {name} =")),
                    "{command_id} references missing request type {name}"
                );
            }
        }
        assert_eq!(shell_command_payload("frontend-only"), None);
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
