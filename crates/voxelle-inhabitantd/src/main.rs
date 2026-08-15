use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use base64::Engine as _;
use clap::Parser;
use futures_util::stream;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    signal,
    sync::{broadcast, Mutex, Semaphore},
    time,
};
use tracing::info;
use voxelle_app::{
    resolve_home_root, shell_command_ids, shell_contract_typescript, OriginContext,
    ServiceActivityItem, ShellError, ShellRecovery, ShellSnapshotView, ShellState,
};

const ORIGIN_ID_HEADER: &str = "voxelle-origin-id";
const ORIGIN_SECRET_HEADER: &str = "voxelle-origin-secret";
const ORIGIN_REGISTRY_FILE: &str = ".voxelle-inhabitant-origins.json";
const ORIGIN_REQUIRED_COMMANDS: &[&str] = &[
    "message.send",
    "message.acknowledge",
    "message.continuation.update",
];

#[derive(Debug, Parser)]
#[command(
    name = "voxelle-inhabitantd",
    about = "Local HTTP/SSE inhabitant sidecar for Voxelle"
)]
struct Cli {
    #[arg(long, value_name = "DIR")]
    home: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1")]
    host: IpAddr,
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[arg(long, value_name = "FILE")]
    discovery_file: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    shell: Arc<ShellState>,
    discovery: DiscoveryView,
    bearer_token: Arc<str>,
    request_slots: Arc<Semaphore>,
    command_gate: Arc<Mutex<()>>,
    event_slots: Arc<Semaphore>,
    snapshot_changes: broadcast::Sender<u64>,
    origin_registry_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryView {
    surface_version: String,
    home_root: PathBuf,
    base_url: String,
    pid: u32,
    started_at_unix_ms: u128,
    snapshot_url: String,
    coordination_snapshot_url: String,
    events_url: String,
    commands_url: String,
    contract_url: String,
    authorization: String,
    capabilities: CapabilitiesView,
    command_transport: CommandTransportView,
    origin_authentication: OriginAuthenticationView,
    command_semantics: Vec<CommandSemanticsView>,
    actionability_semantics: ActionabilitySemanticsView,
    resident_observation_semantics: ResidentObservationSemanticsView,
    replay_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandTransportView {
    method: String,
    content_type: String,
    authorization_header: String,
    origin_id_header: String,
    origin_secret_header: String,
    request_body: String,
    response_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OriginAuthenticationView {
    meaning: String,
    open_command: String,
    secret_requirement: String,
    retry: String,
    required_commands: Vec<String>,
    missing_or_invalid: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenResidentOriginRequest {
    client_instance_id: String,
    secret: String,
    label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ResidentOriginSessionView {
    origin_id: String,
    client_instance_id: String,
    label: String,
    device_id: String,
    created_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OriginRegistry {
    v: u8,
    sessions: Vec<OriginSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OriginSessionRecord {
    origin_id: String,
    client_instance_id: String,
    secret_hash: String,
    label: String,
    device_id: String,
    created_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandSemanticsView {
    command_id: String,
    retry: String,
    observation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResidentObservationSemanticsView {
    delivery: String,
    consumer_id: String,
    counters: String,
    stream: String,
    page: String,
    commit: String,
    privacy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActionabilitySemanticsView {
    scope: String,
    derivation: String,
    conflict: String,
    resume: String,
    meaning: String,
    ordering: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapabilitiesView {
    commands: Vec<String>,
    events: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ActionResult {
    ok: bool,
    command_id: String,
    snapshot: Option<Value>,
    activity_items: Vec<ServiceActivityItem>,
    error: Option<ShellError>,
    recovery: Option<ShellRecovery>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    validate_host(cli.host)?;
    let home = resolve_home_root(cli.home);
    let listener = TcpListener::bind(SocketAddr::new(cli.host, cli.port))
        .await
        .context("bind inhabitant sidecar")?;
    let addr = listener.local_addr().context("read listener address")?;
    let base_url = format!("http://{addr}");
    let bearer_token = new_bearer_token();
    let discovery_view = DiscoveryView::new(home.clone(), base_url, &bearer_token);
    let origin_registry_path = home.join(ORIGIN_REGISTRY_FILE);
    write_discovery_file(cli.discovery_file.as_deref(), &home, &discovery_view)
        .context("write discovery file")?;

    let (snapshot_changes, snapshot_invalidated) = snapshot_change_channel();
    let state = Arc::new(AppState {
        shell: Arc::new(ShellState::new_with_notifier(home, snapshot_invalidated)),
        discovery: discovery_view,
        bearer_token: Arc::from(bearer_token),
        request_slots: Arc::new(Semaphore::new(8)),
        command_gate: Arc::new(Mutex::new(())),
        event_slots: Arc::new(Semaphore::new(8)),
        snapshot_changes,
        origin_registry_path,
    });
    let app = Router::new()
        .route("/inhabitant/v0/discovery", get(get_discovery))
        .route("/inhabitant/v0/snapshot", get(snapshot))
        .route(
            "/inhabitant/v0/snapshot/coordination",
            get(coordination_snapshot),
        )
        .route("/inhabitant/v0/commands/:command_id", post(command))
        .route("/inhabitant/v0/contract.ts", get(contract))
        .route("/inhabitant/v0/events", get(events))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state);

    info!("Serving {}", app_url(addr));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve inhabitant sidecar")?;
    Ok(())
}

impl DiscoveryView {
    fn new(home_root: PathBuf, base_url: String, bearer_token: &str) -> Self {
        Self {
            surface_version: "inhabitant.v0".to_string(),
            home_root,
            snapshot_url: format!("{base_url}/inhabitant/v0/snapshot"),
            coordination_snapshot_url: format!(
                "{base_url}/inhabitant/v0/snapshot/coordination"
            ),
            events_url: format!("{base_url}/inhabitant/v0/events"),
            commands_url: format!("{base_url}/inhabitant/v0/commands/{{command_id}}"),
            contract_url: format!("{base_url}/inhabitant/v0/contract.ts"),
            authorization: format!("Bearer {bearer_token}"),
            base_url,
            pid: std::process::id(),
            started_at_unix_ms: unix_ms(),
            capabilities: CapabilitiesView {
                commands: {
                    let mut commands = shell_command_ids();
                    commands.push("resident.origin.open".to_string());
                    commands
                },
                events: vec![
                    "service.ready".to_string(),
                    "snapshot.changed".to_string(),
                    "heartbeat".to_string(),
                ],
            },
            command_transport: CommandTransportView {
                method: "POST".to_string(),
                content_type: "application/json".to_string(),
                authorization_header: "Authorization: Bearer <per-launch token>".to_string(),
                origin_id_header: "Voxelle-Origin-Id: <origin_id>".to_string(),
                origin_secret_header: "Voxelle-Origin-Secret: <caller secret>".to_string(),
                request_body: "Direct JSON value matching the command payload_type; use {} for empty payloads".to_string(),
                response_body: "ActionResult JSON with ok, command_id, snapshot, activity_items, error, and recovery; snapshot is the compact coordination projection for ordinary product commands and the command-specific consumer/page/commit DTO for resident.observation commands".to_string(),
            },
            origin_authentication: OriginAuthenticationView {
                meaning: "An origin session authenticates which local inhabitant session submitted a command through this authorized device; it is not a principal, member, device authority, role, or proof of a natural person or AI".to_string(),
                open_command: "resident.origin.open".to_string(),
                secret_requirement: "caller-generated 32 random bytes encoded as unpadded base64url; Voxelle never returns or persists the plaintext secret".to_string(),
                retry: "reuse the identical client_instance_id, secret, and label to recover the same origin_id after response loss or process restart; a changed label is rejected and a wrong secret never identifies the existing session".to_string(),
                required_commands: ORIGIN_REQUIRED_COMMANDS.iter().map(|value| (*value).to_string()).collect(),
                missing_or_invalid: "missing credentials return origin_required; unknown origins and wrong secrets return the same origin_authentication_failed response".to_string(),
            },
            command_semantics: vec![
                CommandSemanticsView {
                    command_id: "resident.origin.open".to_string(),
                    retry: "idempotent only for the identical client_instance_id, caller-held 32-byte secret, and label; exact retry returns the same origin_id across response loss or sidecar restart".to_string(),
                    observation: "creates or reopens a device-local authenticated inhabitant origin; the secret is never returned or persisted in plaintext, the session grants no protocol authority, and an incorrect secret receives the same failure as an unknown origin".to_string(),
                },
                CommandSemanticsView {
                    command_id: "message.send".to_string(),
                    retry: "client_request_id is 8 to 128 non-whitespace characters; reuse the same ID only for the identical principal, device, room, and payload; a conflicting reuse is rejected".to_string(),
                    observation: "ok proves local admission; inspect the projected message by client_request_id, sync_evidence for peer-relative propagation, and signed acknowledgements for recipient observation or handling".to_string(),
                },
                CommandSemanticsView {
                    command_id: "resident.observation.open".to_string(),
                    retry: "idempotent only for the same consumer_id and immutable start policy; choose from_beginning to receive retained prior facts or from_now to begin after current local admission".to_string(),
                    observation: "returns local consumer metadata only; it does not read, acknowledge, synchronize, publish, or grant protocol authority".to_string(),
                },
                CommandSemanticsView {
                    command_id: "resident.observation.page".to_string(),
                    retry: "before commit, refetch from the first page after process restart; within one process continue only with the exact fact_high_water and next_after_fact_sequence returned by the preceding page; starting a fresh first page supersedes every prior page session and final token for that consumer".to_string(),
                    observation: "returns at-least-once changed ordinary thread projections across the exact accessible room set captured by the first page; the final page alone carries a one-use commit_token".to_string(),
                },
                CommandSemanticsView {
                    command_id: "resident.observation.commit".to_string(),
                    retry: "a successful commit is durable; if the response is lost, refetch the feed because the one-use token may already be consumed".to_string(),
                    observation: "requires the final served commit_token and matching fact_high_water; advances only this consumer in the rooms that were served, emits no global snapshot change, and does not mark read, acknowledge, handle, or prove correctness".to_string(),
                },
                CommandSemanticsView {
                    command_id: "resident.observation.release".to_string(),
                    retry: "idempotent; false means the consumer was already absent".to_string(),
                    observation: "deletes only local consumer progress; reopening requires an explicit start policy and changes no conversation or protocol fact".to_string(),
                },
                CommandSemanticsView {
                    command_id: "message.acknowledge".to_string(),
                    retry: "semantic idempotent for the same state and result_event_id; handled is monotonic, and rebinding a locally known handled result is rejected".to_string(),
                    observation: "the signed acknowledgement is an admitted participant assertion, not proof that the work was correct; optional handled result_event_id must name the handler's visible admitted reply threaded to the target, while observed must omit it; concurrent device results are retained and projected as a conflict".to_string(),
                },
                CommandSemanticsView {
                    command_id: "message.continuation.update".to_string(),
                    retry: "reuse client_request_id only for the identical target, state, lease_ms, and sorted supersedes_event_ids; renewals and conflict resolutions use a new ID".to_string(),
                    observation: "continuing is a bounded participant intention, not presence or proof of work; expiry projects unknown/overdue but emits no SSE event, so schedule a coordination snapshot fetch at home.coordination_frontier.next_projection_change_ms; that bounded frontier covers all accessible rooms, reports item-list truncation explicitly, and remains independent of room selection and read state; target_summary is only an orientation preview, and target_summary_truncated requires opening room_id/target_event_id before consequential action; coordination snapshot GET is observational and does not initiate peer sync; release and decline are explicit assertions, concurrent unsuperseded device updates project conflict, and handled remains separate completion evidence".to_string(),
                },
                CommandSemanticsView {
                    command_id: "channel.select".to_string(),
                    retry: "semantic idempotent".to_string(),
                    observation: "selection changes local context only and never marks messages read or acknowledges them".to_string(),
                },
                CommandSemanticsView {
                    command_id: "channel.markRead".to_string(),
                    retry: "semantic idempotent".to_string(),
                    observation: "advances only this home's local read cursor; it is not a signed acknowledgement visible to senders".to_string(),
                },
                CommandSemanticsView {
                    command_id: "runtime.goOnline".to_string(),
                    retry: "safe to reconcile again".to_string(),
                    observation: "starts the local runtime on the last successful automatic binding unless explicit addresses replace it, then attempts known peers; sync_evidence is peer-relative and never claims global currency".to_string(),
                },
            ],
            actionability_semantics: ActionabilitySemanticsView {
                scope: "participant_actionability is derived independently for each participant and target message; it is never a room-global task state".to_string(),
                derivation: "the projection preserves raw acknowledgements and continuations, then selects the causally maximal handled acknowledgement facts and current continuation heads from the admitted room DAG; a reply is covered only when explicitly bound as the handled result or causally known by every maximal disposition head".to_string(),
                conflict: "incomparable handled and continuation maxima project conflict and actionable false; consumers must reconcile instead of guessing a winner".to_string(),
                resume: "a causally later continuing head after handled, released, or declined is an explicit resumption and projects continuing/actionable true; an earlier continuing fact remains visible but does not keep later handled work actionable".to_string(),
                meaning: "state is the participant's literal disposition while actionable independently answers whether current attention evidence exists: a continuing intention or causally uncovered reply makes it true; actionable_reasons and bounded uncovered_reply_event_ids explain why. It is not assignment, obligation, presence, correctness, global completion, or proof that anyone is working".to_string(),
                ordering: "timestamps, receipt order, event ID order, SSE current_sequence, and local fact ordinals never choose a semantic winner; they are not substitutes for DAG ancestry".to_string(),
            },
            resident_observation_semantics: ResidentObservationSemanticsView {
                delivery: "at least once until a fully served page set is committed; crash or restart before commit causes safe rereading, so actions must remain idempotent".to_string(),
                consumer_id: "caller-chosen stable local namespace, not a principal, device, credential, actor, or protocol identity; independent consumers on one home have independent progress".to_string(),
                counters: "fact_high_water and last_fact_sequence are durable home-local first-admission ordinals; they are not SSE current_sequence, wall-clock order, event IDs, room read cursors, acknowledgements, or replicated protocol facts".to_string(),
                stream: "SSE current_sequence is only a process-local wake and reconciliation fence; after every process restart or reconnect, fetch resident.observation.page using the stable consumer_id and never use current_sequence as a durable cursor".to_string(),
                page: "the first page omits fact_high_water and after_fact_sequence; continue with both exact returned values while has_more; after process restart begin again; full ordinary roots and replies are returned, and truncated message fields retain their existing open-before-action semantics; an empty final page may be left uncommitted when the consumer already has no items to examine".to_string(),
                commit: "commit only after every page through fact_high_water has been examined; the final page token binds the consumer, high water, and exact served room set; commit is consumer-local bookkeeping and emits no global snapshot.changed".to_string(),
                privacy: "pages enumerate only currently accessible rooms and project private facts only after ordinary decryption and semantic admission; excluded rooms and ciphertext metadata are absent".to_string(),
            },
            replay_policy: "none; on every connect or reconnect, use service.ready.current_sequence and fetch coordination_snapshot_url until its current_sequence is at least that value; current_sequence covers admitted/invalidation transitions, resets with the process, and is never comparable to durable resident fact_high_water; projected_at_ms timestamps time-derived projection and heartbeat is not semantic change evidence".to_string(),
        }
    }
}

async fn get_discovery(State(state): State<Arc<AppState>>) -> Json<DiscoveryView> {
    Json(state.discovery.clone())
}

async fn contract() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        shell_contract_typescript(),
    )
}

async fn snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Ok(Ok(_permit)) =
        time::timeout(Duration::from_secs(1), state.request_slots.acquire()).await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let Ok(_command_guard) = time::timeout(Duration::from_secs(1), state.command_gate.lock()).await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    match time::timeout(
        Duration::from_secs(30),
        state
            .shell
            .execute_serialized_command("shell.refresh", Value::Null),
    )
    .await
    {
        Err(_) => StatusCode::GATEWAY_TIMEOUT.into_response(),
        Ok(Ok(snapshot)) => (StatusCode::OK, Json(snapshot)).into_response(),
        Ok(Err(error)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response(),
    }
}

async fn coordination_snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Ok(Ok(_permit)) =
        time::timeout(Duration::from_secs(1), state.request_slots.acquire()).await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let Ok(_command_guard) = time::timeout(Duration::from_secs(1), state.command_gate.lock()).await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    for _ in 0..3 {
        let before = snapshot_sequence();
        match time::timeout(
            Duration::from_secs(30),
            state.shell.observational_snapshot(),
        )
        .await
        {
            Err(_) => return StatusCode::GATEWAY_TIMEOUT.into_response(),
            Ok(Err(error)) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response()
            }
            Ok(Ok(snapshot)) => {
                let after = snapshot_sequence();
                if before == after {
                    return (
                        StatusCode::OK,
                        Json(coordination_snapshot_value(snapshot, after)),
                    )
                        .into_response();
                }
            }
        }
    }
    StatusCode::CONFLICT.into_response()
}

async fn command(
    State(state): State<Arc<AppState>>,
    Path(command_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let Ok(Ok(_permit)) =
        time::timeout(Duration::from_secs(1), state.request_slots.acquire()).await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let Ok(command_guard) = time::timeout(Duration::from_secs(1), state.command_gate.lock()).await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    if command_id == "resident.origin.open" {
        let Some(device_id) = state.shell.current_device_id().await else {
            drop(command_guard);
            return origin_error_response(
                StatusCode::BAD_REQUEST,
                &command_id,
                "origin_unavailable",
                "Voxelle cannot certify an origin session before this home has a device identity.",
                "Initialize or restore the home, then retry the identical resident.origin.open request.",
            );
        };
        let response = match serde_json::from_value::<OpenResidentOriginRequest>(payload) {
            Ok(request) => match open_origin_session(&state.origin_registry_path, &device_id, request) {
                Ok(session) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "ok": true,
                        "command_id": command_id,
                        "snapshot": session,
                        "activity_items": [],
                        "error": null,
                        "recovery": null
                    })),
                )
                    .into_response(),
                Err(OriginOpenError::Input) => origin_error_response(
                    StatusCode::BAD_REQUEST,
                    &command_id,
                    "origin_input_invalid",
                    "The origin session request is invalid.",
                    "Use an 8–128 character client_instance_id, a 1–80 character label, and a caller-generated 32-byte unpadded base64url secret.",
                ),
                Err(OriginOpenError::Authentication) => origin_authentication_failed(&command_id),
                Err(OriginOpenError::Internal) => origin_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &command_id,
                    "origin_registry_failed",
                    "Voxelle could not access the local origin registry.",
                    "Retry once; if it repeats, retain the service logs for diagnosis.",
                ),
            },
            Err(_) => origin_error_response(
                StatusCode::BAD_REQUEST,
                &command_id,
                "origin_input_invalid",
                "The origin session request is invalid.",
                "Send the exact OpenResidentOriginRequest contract.",
            ),
        };
        drop(command_guard);
        return response;
    }
    let origin = if ORIGIN_REQUIRED_COMMANDS.contains(&command_id.as_str()) {
        let Some(device_id) = state.shell.current_device_id().await else {
            drop(command_guard);
            return origin_error_response(
                StatusCode::BAD_REQUEST,
                &command_id,
                "origin_unavailable",
                "Voxelle cannot authenticate an origin session before this home has a device identity.",
                "Initialize or restore the home, then open an origin session.",
            );
        };
        let (capability, label) = match authenticate_origin(
            &state.origin_registry_path,
            &device_id,
            &headers,
        ) {
            Ok(value) => value,
            Err(OriginAuthError::Missing) => {
                drop(command_guard);
                return origin_error_response(
                    StatusCode::UNAUTHORIZED,
                    &command_id,
                    "origin_required",
                    "This command requires an authenticated origin session.",
                    "Call resident.origin.open, then send Voxelle-Origin-Id and Voxelle-Origin-Secret headers.",
                );
            }
            Err(OriginAuthError::Failed) => {
                drop(command_guard);
                return origin_authentication_failed(&command_id);
            }
            Err(OriginAuthError::Internal) => {
                drop(command_guard);
                return origin_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &command_id,
                    "origin_registry_failed",
                    "Voxelle could not access the local origin registry.",
                    "Retry once; if it repeats, retain the service logs for diagnosis.",
                );
            }
        };
        let request_id = format!("inhabitant-{}", new_bearer_token());
        match state
            .shell
            .issue_inhabitant_origin_context(&capability, label, request_id)
            .await
        {
            Ok(origin) => Some(origin),
            Err(_) => {
                drop(command_guard);
                return origin_error_response(
                    StatusCode::BAD_REQUEST,
                    &command_id,
                    "origin_unavailable",
                    "Voxelle could not certify this origin session for the current home.",
                    "Initialize or restore the home, then retry with the same origin credentials.",
                );
            }
        }
    } else {
        None
    };
    let Ok(result) = time::timeout(
        Duration::from_secs(30),
        run_command(&state.shell, &command_id, payload, origin),
    )
    .await
    else {
        return StatusCode::GATEWAY_TIMEOUT.into_response();
    };
    drop(command_guard);
    let status = if result.ok {
        if !command_id.starts_with("resident.observation.") {
            notify_snapshot_change(&state.snapshot_changes);
        }
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result)).into_response()
}

async fn run_command(
    shell: &ShellState,
    command_id: &str,
    payload: Value,
    origin: Option<OriginContext>,
) -> ActionResult {
    let activity_cursor = shell.activity_cursor().await;
    if let Some(result) = shell
        .execute_resident_command(command_id, payload.clone())
        .await
    {
        let activity_items = shell.activity_items_after(activity_cursor).await;
        return match result {
            Ok(value) => ActionResult {
                ok: true,
                command_id: command_id.to_string(),
                snapshot: Some(value),
                activity_items,
                error: None,
                recovery: None,
            },
            Err(error) => ActionResult {
                ok: false,
                command_id: command_id.to_string(),
                snapshot: None,
                activity_items,
                recovery: Some(error.recovery),
                error: Some(error),
            },
        };
    }
    let result = match origin {
        Some(origin) => {
            shell
                .execute_serialized_command_with_origin(command_id, payload, origin)
                .await
        }
        None => shell.execute_serialized_command(command_id, payload).await,
    };
    let activity_items = match &result {
        Ok(snapshot) => snapshot
            .service_activity
            .iter()
            .filter(|item| item.id > activity_cursor)
            .cloned()
            .collect(),
        Err(_) => shell.activity_items_after(activity_cursor).await,
    };
    match result {
        Ok(snapshot) => ActionResult {
            ok: true,
            command_id: command_id.to_string(),
            snapshot: Some(coordination_snapshot_value(snapshot, snapshot_sequence())),
            activity_items,
            error: None,
            recovery: None,
        },
        Err(error) => ActionResult {
            ok: false,
            command_id: command_id.to_string(),
            snapshot: None,
            activity_items,
            recovery: Some(error.recovery),
            error: Some(error),
        },
    }
}

fn coordination_snapshot_value(snapshot: ShellSnapshotView, current_sequence: u64) -> Value {
    let mut value = serde_json::to_value(snapshot).expect("shell snapshot serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("product_component");
        object.remove("ui_ontology");
        object.insert(
            "projection".to_string(),
            Value::String("coordination".to_string()),
        );
        object.insert(
            "current_sequence".to_string(),
            Value::from(current_sequence),
        );
        object.insert(
            "projected_at_ms".to_string(),
            Value::from(u64::try_from(unix_ms()).unwrap_or(u64::MAX)),
        );
    }
    value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OriginOpenError {
    Input,
    Authentication,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OriginAuthError {
    Missing,
    Failed,
    Internal,
}

fn open_origin_session(
    registry_path: &FsPath,
    device_id: &str,
    request: OpenResidentOriginRequest,
) -> std::result::Result<ResidentOriginSessionView, OriginOpenError> {
    if request.client_instance_id.len() < 8
        || request.client_instance_id.len() > 128
        || request.client_instance_id.chars().any(char::is_whitespace)
        || request.label.trim() != request.label
        || request.label.is_empty()
        || request.label.chars().count() > 80
    {
        return Err(OriginOpenError::Input);
    }
    let capability = decode_origin_secret(&request.secret).ok_or(OriginOpenError::Input)?;
    let secret_hash = origin_secret_hash(&capability);
    let origin_id = origin_id(&capability);
    let mut registry =
        load_origin_registry(registry_path).map_err(|_| OriginOpenError::Internal)?;
    if let Some(existing) = registry
        .sessions
        .iter()
        .find(|session| session.client_instance_id == request.client_instance_id)
    {
        if existing.device_id != device_id
            || !constant_time_eq(existing.secret_hash.as_bytes(), secret_hash.as_bytes())
        {
            return Err(OriginOpenError::Authentication);
        }
        if existing.label != request.label || existing.origin_id != origin_id {
            return Err(OriginOpenError::Input);
        }
        return Ok(existing.view());
    }
    let record = OriginSessionRecord {
        origin_id,
        client_instance_id: request.client_instance_id,
        secret_hash,
        label: request.label,
        device_id: device_id.to_string(),
        created_ms: u64::try_from(unix_ms()).unwrap_or(u64::MAX),
    };
    let view = record.view();
    registry.v = 1;
    registry.sessions.push(record);
    registry
        .sessions
        .sort_by(|left, right| left.client_instance_id.cmp(&right.client_instance_id));
    write_origin_registry(registry_path, &registry).map_err(|_| OriginOpenError::Internal)?;
    Ok(view)
}

impl OriginSessionRecord {
    fn view(&self) -> ResidentOriginSessionView {
        ResidentOriginSessionView {
            origin_id: self.origin_id.clone(),
            client_instance_id: self.client_instance_id.clone(),
            label: self.label.clone(),
            device_id: self.device_id.clone(),
            created_ms: self.created_ms,
        }
    }
}

fn authenticate_origin(
    registry_path: &FsPath,
    device_id: &str,
    headers: &HeaderMap,
) -> std::result::Result<([u8; 32], String), OriginAuthError> {
    let Some(origin_id) = headers
        .get(ORIGIN_ID_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return Err(OriginAuthError::Missing);
    };
    let Some(secret) = headers
        .get(ORIGIN_SECRET_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return Err(OriginAuthError::Missing);
    };
    let capability = decode_origin_secret(secret).ok_or(OriginAuthError::Failed)?;
    let secret_hash = origin_secret_hash(&capability);
    let registry = load_origin_registry(registry_path).map_err(|_| OriginAuthError::Internal)?;
    let record = registry
        .sessions
        .iter()
        .find(|session| session.origin_id == origin_id && session.device_id == device_id)
        .ok_or(OriginAuthError::Failed)?;
    if !constant_time_eq(record.secret_hash.as_bytes(), secret_hash.as_bytes()) {
        return Err(OriginAuthError::Failed);
    }
    Ok((capability, record.label.clone()))
}

fn decode_origin_secret(secret: &str) -> Option<[u8; 32]> {
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(secret)
        .ok()?;
    decoded.try_into().ok()
}

fn origin_secret_hash(capability: &[u8; 32]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"voxelle-inhabitant-origin-secret-v1\0");
    digest.update(capability);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest.finalize())
}

fn origin_id(capability: &[u8; 32]) -> String {
    format!(
        "os:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(capability))
    )
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn load_origin_registry(path: &FsPath) -> Result<OriginRegistry> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("decode local origin registry"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OriginRegistry {
            v: 1,
            sessions: Vec::new(),
        }),
        Err(error) => Err(error).context("read local origin registry"),
    }
}

fn write_origin_registry(path: &FsPath, registry: &OriginRegistry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create origin registry parent {}", parent.display()))?;
    }
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        anyhow::bail!("refusing symlink origin registry {}", path.display());
    }
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8; 9]>())
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write as _;
    let mut file = options.open(&temporary).context("create origin registry")?;
    file.write_all(&serde_json::to_vec_pretty(registry)?)
        .context("write origin registry")?;
    file.sync_all().context("sync origin registry")?;
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).context("remove prior origin registry")?;
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).context("replace origin registry");
    }
    Ok(())
}

fn origin_error_response(
    status: StatusCode,
    command_id: &str,
    code: &'static str,
    message: &'static str,
    recovery_message: &'static str,
) -> axum::response::Response {
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "command_id": command_id,
            "snapshot": null,
            "activity_items": [],
            "error": {
                "message": message,
                "recovery": "needs_input",
                "recovery_message": recovery_message,
                "detail": code
            },
            "recovery": "needs_input",
            "code": code
        })),
    )
        .into_response()
}

fn origin_authentication_failed(command_id: &str) -> axum::response::Response {
    origin_error_response(
        StatusCode::FORBIDDEN,
        command_id,
        "origin_authentication_failed",
        "The origin session credentials are invalid.",
        "Use the original caller-held secret or open a distinct origin session with a new client_instance_id.",
    )
}

async fn events(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Ok(Ok(permit)) = time::timeout(
        Duration::from_secs(1),
        state.event_slots.clone().acquire_owned(),
    )
    .await
    else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let discovery = state.discovery.clone();
    let changes = state.snapshot_changes.subscribe();
    let stream = stream::unfold(
        EventState::Ready(Box::new(discovery), changes, permit),
        |state| async move {
            match state {
                EventState::Ready(discovery, changes, permit) => {
                    let ready = serde_json::json!({
                        "surface_version": discovery.surface_version,
                        "home_root": discovery.home_root,
                        "base_url": discovery.base_url,
                        "current_sequence": snapshot_sequence(),
                        "reconnect_action": "fetch coordination_snapshot_url before acting",
                        "coordination_snapshot_url": discovery.coordination_snapshot_url,
                    });
                    Some((
                        Ok::<Event, Infallible>(
                            Event::default()
                                .event("service.ready")
                                .data(ready.to_string()),
                        ),
                        EventState::Listening(discovery, changes, permit),
                    ))
                }
                EventState::Listening(discovery, mut changes, permit) => {
                    let event = tokio::select! {
                        received = changes.recv() => match received {
                            Ok(sequence) => snapshot_changed_event(sequence, &discovery.coordination_snapshot_url),
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                snapshot_changed_event(snapshot_sequence(), &discovery.coordination_snapshot_url)
                            }
                            Err(broadcast::error::RecvError::Closed) => return None,
                        },
                        _ = time::sleep(Duration::from_secs(30)) => {
                            let heartbeat = serde_json::json!({
                                "at_unix_ms": unix_ms(),
                                "pid": std::process::id(),
                            });
                            Event::default().event("heartbeat").data(heartbeat.to_string())
                        }
                    };
                    Some((
                        Ok::<Event, Infallible>(event),
                        EventState::Listening(discovery, changes, permit),
                    ))
                }
            }
        },
    );
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

enum EventState {
    Ready(
        Box<DiscoveryView>,
        broadcast::Receiver<u64>,
        tokio::sync::OwnedSemaphorePermit,
    ),
    Listening(
        Box<DiscoveryView>,
        broadcast::Receiver<u64>,
        tokio::sync::OwnedSemaphorePermit,
    ),
}

static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn snapshot_change_channel() -> (broadcast::Sender<u64>, Arc<dyn Fn() + Send + Sync>) {
    let (changes, _) = broadcast::channel(64);
    let notifier = changes.clone();
    let snapshot_invalidated = Arc::new(move || notify_snapshot_change(&notifier));
    (changes, snapshot_invalidated)
}

fn notify_snapshot_change(changes: &broadcast::Sender<u64>) {
    let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = changes.send(sequence);
}

fn snapshot_sequence() -> u64 {
    SNAPSHOT_SEQUENCE.load(Ordering::Relaxed)
}

fn snapshot_changed_event(sequence: u64, snapshot_url: &str) -> Event {
    let changed = serde_json::json!({
        "sequence": sequence,
        "at_unix_ms": unix_ms(),
        "snapshot_url": snapshot_url,
    });
    Event::default()
        .event("snapshot.changed")
        .id(sequence.to_string())
        .data(changed.to_string())
}

async fn authenticate(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> axum::response::Response {
    if request.headers().contains_key(header::ORIGIN)
        || !authorized(request.headers(), &state.bearer_token)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

fn authorized(headers: &HeaderMap, bearer_token: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        == Some(bearer_token)
}

fn new_bearer_token() -> String {
    let mut token = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut token);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token)
}

fn validate_host(host: IpAddr) -> Result<()> {
    if !host.is_loopback() {
        anyhow::bail!("the inhabitant sidecar supports loopback binds only");
    }
    Ok(())
}

fn write_discovery_file(
    path: Option<&FsPath>,
    home: &FsPath,
    discovery: &DiscoveryView,
) -> Result<()> {
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".voxelle-inhabitantd.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create discovery parent {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(discovery)?;
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("refusing symlink discovery file {}", path.display());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write as _;
    let mut file = options
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("protect {}", path.display()))?;
    }
    file.write_all(format!("{json}\n").as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn app_url(addr: SocketAddr) -> String {
    format!("http://{addr}/inhabitant/v0/discovery")
}

fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_secret(byte: u8) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([byte; 32])
    }

    fn origin_headers(origin_id: &str, secret: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(ORIGIN_ID_HEADER, origin_id.parse().expect("origin id"));
        headers.insert(ORIGIN_SECRET_HEADER, secret.parse().expect("origin secret"));
        headers
    }

    #[test]
    fn authorization_requires_exact_bearer_token() {
        let mut headers = HeaderMap::new();
        assert!(!authorized(&headers, "secret"));
        headers.insert(
            header::AUTHORIZATION,
            "Bearer wrong".parse().expect("header"),
        );
        assert!(!authorized(&headers, "secret"));
        headers.insert(
            header::AUTHORIZATION,
            "Bearer secret".parse().expect("header"),
        );
        assert!(authorized(&headers, "secret"));
    }

    #[test]
    fn sidecar_refuses_non_loopback_bind() {
        assert!(validate_host("127.0.0.1".parse().expect("loopback")).is_ok());
        assert!(validate_host("::1".parse().expect("loopback")).is_ok());
        assert!(validate_host("0.0.0.0".parse().expect("wildcard")).is_err());
    }

    #[test]
    fn bearer_token_has_256_bits_of_random_material() {
        let token = new_bearer_token();
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(token)
            .expect("token");
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn origin_open_is_persistent_idempotent_and_never_stores_plaintext_secret() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(ORIGIN_REGISTRY_FILE);
        let secret = test_secret(7);
        let request = OpenResidentOriginRequest {
            client_instance_id: "resident-instance-a".to_string(),
            secret: secret.clone(),
            label: "Resident A".to_string(),
        };
        let opened = open_origin_session(&path, "device:test", request.clone()).expect("open");
        let retried = open_origin_session(&path, "device:test", request).expect("retry");
        assert_eq!(opened, retried);
        assert_eq!(opened.origin_id, origin_id(&[7; 32]));

        let persisted = std::fs::read_to_string(&path).expect("registry");
        assert!(!persisted.contains(&secret));
        assert!(!persisted.contains("BwcHBwcH"));
        assert!(persisted.contains("secret_hash"));

        let headers = origin_headers(&opened.origin_id, &secret);
        let (capability, label) =
            authenticate_origin(&path, "device:test", &headers).expect("restart auth");
        assert_eq!(capability, [7; 32]);
        assert_eq!(label, "Resident A");
    }

    #[test]
    fn origin_authentication_is_generic_for_wrong_or_unknown_credentials() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(ORIGIN_REGISTRY_FILE);
        let secret = test_secret(8);
        let opened = open_origin_session(
            &path,
            "device:test",
            OpenResidentOriginRequest {
                client_instance_id: "resident-instance-b".to_string(),
                secret: secret.clone(),
                label: "Build resident".to_string(),
            },
        )
        .expect("open");
        assert_eq!(
            authenticate_origin(&path, "device:test", &HeaderMap::new()),
            Err(OriginAuthError::Missing)
        );
        assert_eq!(
            authenticate_origin(
                &path,
                "device:test",
                &origin_headers(&opened.origin_id, &test_secret(9))
            ),
            Err(OriginAuthError::Failed)
        );
        assert_eq!(
            authenticate_origin(&path, "device:test", &origin_headers("os:unknown", &secret)),
            Err(OriginAuthError::Failed)
        );
        assert_eq!(
            authenticate_origin(
                &path,
                "device:rotated",
                &origin_headers(&opened.origin_id, &secret)
            ),
            Err(OriginAuthError::Failed)
        );
        assert_eq!(
            open_origin_session(
                &path,
                "device:test",
                OpenResidentOriginRequest {
                    client_instance_id: "resident-instance-b".to_string(),
                    secret: test_secret(9),
                    label: "Build resident".to_string(),
                }
            ),
            Err(OriginOpenError::Authentication)
        );
        assert!(ORIGIN_REQUIRED_COMMANDS.contains(&"message.send"));
    }

    #[tokio::test]
    async fn authenticated_origin_is_signed_and_projected_on_message() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let registry_path = home.join(ORIGIN_REGISTRY_FILE);
        let shell = ShellState::new(&home);
        let initialized = run_command(
            &shell,
            "home.init",
            serde_json::json!({"default_room": null}),
            None,
        )
        .await;
        assert!(initialized.ok);
        let secret = test_secret(10);
        let opened = open_origin_session(
            &registry_path,
            &shell.current_device_id().await.expect("device"),
            OpenResidentOriginRequest {
                client_instance_id: "resident-projection".to_string(),
                secret: secret.clone(),
                label: "Projection resident".to_string(),
            },
        )
        .expect("open");
        let (capability, label) = authenticate_origin(
            &registry_path,
            &shell.current_device_id().await.expect("device"),
            &origin_headers(&opened.origin_id, &secret),
        )
        .expect("authenticate");
        let origin = shell
            .issue_inhabitant_origin_context(
                &capability,
                label,
                "projection-request-001".to_string(),
            )
            .await
            .expect("certify");
        let sent = run_command(
            &shell,
            "message.send",
            serde_json::json!({
                "text": "attributed",
                "room": null,
                "mentions": [],
                "thread_root_event_id": null,
                "client_request_id": "projection-message-001"
            }),
            Some(origin),
        )
        .await;
        assert!(sent.ok);
        let message = &sent.snapshot.expect("snapshot")["home"]["room"]["messages"][0];
        assert_eq!(message["origin"]["session_id"], opened.origin_id);
        assert_eq!(message["origin"]["surface_protocol"], "inhabitant");
        assert_eq!(message["origin"]["display_label"], "Projection resident");
        assert_eq!(message["origin"]["request_id"], "projection-request-001");
    }

    #[tokio::test]
    async fn bearer_only_semantic_mutation_is_rejected_before_dispatch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let bearer = "test-bearer";
        let discovery = DiscoveryView::new(home.clone(), "http://127.0.0.1:1".to_string(), bearer);
        let (snapshot_changes, snapshot_invalidated) = snapshot_change_channel();
        let shell = Arc::new(ShellState::new_with_notifier(&home, snapshot_invalidated));
        assert!(
            run_command(
                &shell,
                "home.init",
                serde_json::json!({"default_room": null}),
                None,
            )
            .await
            .ok
        );
        let state = Arc::new(AppState {
            shell,
            discovery,
            bearer_token: Arc::from(bearer),
            request_slots: Arc::new(Semaphore::new(8)),
            command_gate: Arc::new(Mutex::new(())),
            event_slots: Arc::new(Semaphore::new(8)),
            snapshot_changes,
            origin_registry_path: home.join(ORIGIN_REGISTRY_FILE),
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer test-bearer".parse().expect("bearer"),
        );
        let response = command(
            State(state),
            Path("message.send".to_string()),
            headers,
            Json(serde_json::json!({
                "text": "must not dispatch",
                "room": null,
                "mentions": [],
                "thread_root_event_id": null,
                "client_request_id": "missing-origin-001"
            })),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn discovery_names_origin_transport_and_required_commands_exactly() {
        let view = DiscoveryView::new(
            PathBuf::from("/tmp/example"),
            "http://127.0.0.1:1".to_string(),
            "bearer",
        );
        assert_eq!(
            view.origin_authentication.required_commands,
            ORIGIN_REQUIRED_COMMANDS
        );
        assert!(view
            .capabilities
            .commands
            .contains(&"resident.origin.open".to_string()));
        assert!(view
            .command_transport
            .origin_id_header
            .starts_with("Voxelle-Origin-Id"));
        assert!(view
            .command_transport
            .origin_secret_header
            .starts_with("Voxelle-Origin-Secret"));
    }

    #[tokio::test]
    async fn action_result_reuses_the_shell_recovery_classification() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));
        let result = run_command(&shell, "not_a_command", serde_json::json!({}), None).await;
        assert_eq!(result.recovery, Some(ShellRecovery::InternalError));
        assert_eq!(
            result.error.expect("structured error").recovery,
            ShellRecovery::InternalError
        );

        let initialized = run_command(
            &shell,
            "home.init",
            serde_json::json!({ "default_room": null }),
            None,
        )
        .await;
        assert!(initialized.ok);
        assert!(initialized
            .activity_items
            .iter()
            .any(|item| item.summary.starts_with("initialized home for ")));
        assert!(initialized
            .activity_items
            .iter()
            .any(|item| item.summary.starts_with("service started at ")));
        let initialized_last_id = initialized
            .activity_items
            .last()
            .expect("initialization activity")
            .id;

        let stopped = run_command(&shell, "runtime.goOffline", serde_json::json!({}), None).await;
        assert_eq!(stopped.activity_items.len(), 1);
        assert_eq!(stopped.activity_items[0].summary, "service stopped");
        assert!(stopped.activity_items[0].id > initialized_last_id);

        let input = run_command(
            &shell,
            "message.search",
            serde_json::json!({ "query": " ", "room": null, "limit": 10 }),
            None,
        )
        .await;
        assert_eq!(input.recovery, Some(ShellRecovery::NeedsInput));
        assert_eq!(
            input.error.expect("input error").recovery,
            ShellRecovery::NeedsInput
        );
    }

    #[tokio::test]
    async fn snapshot_notifier_wakes_event_subscribers_with_monotonic_sequences() {
        let (changes, notify) = snapshot_change_channel();
        let mut first = changes.subscribe();
        let mut second = changes.subscribe();

        notify();
        let first_sequence = first.recv().await.expect("first subscriber");
        assert_eq!(
            second.recv().await.expect("second subscriber"),
            first_sequence
        );

        notify();
        assert_eq!(first.recv().await.expect("next change"), first_sequence + 1);
    }

    #[cfg(unix)]
    #[test]
    fn discovery_file_is_private_and_refuses_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("discovery.json");
        let view = DiscoveryView::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:1".into(),
            "secret",
        );
        write_discovery_file(Some(&path), dir.path(), &view).expect("write");
        assert_eq!(
            std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let victim = dir.path().join("victim");
        std::fs::write(&victim, "preserve").expect("victim");
        let link = dir.path().join("link");
        symlink(&victim, &link).expect("link");
        assert!(write_discovery_file(Some(&link), dir.path(), &view).is_err());
        assert_eq!(std::fs::read_to_string(victim).expect("victim"), "preserve");
    }
}
