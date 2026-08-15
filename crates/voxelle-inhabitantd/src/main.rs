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
    resolve_home_root, shell_command_ids, shell_contract_typescript, ServiceActivityItem,
    ShellError, ShellRecovery, ShellSnapshotView, ShellState,
};

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
    command_semantics: Vec<CommandSemanticsView>,
    replay_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandTransportView {
    method: String,
    content_type: String,
    authorization_header: String,
    request_body: String,
    response_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandSemanticsView {
    command_id: String,
    retry: String,
    observation: String,
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
                commands: shell_command_ids(),
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
                request_body: "Direct JSON value matching the command payload_type; use {} for empty payloads".to_string(),
                response_body: "ActionResult JSON with ok, command_id, compact snapshot, activity_items, error, and recovery".to_string(),
            },
            command_semantics: vec![
                CommandSemanticsView {
                    command_id: "message.send".to_string(),
                    retry: "reuse the same client_request_id only for the identical principal, device, room, and payload; a conflicting reuse is rejected".to_string(),
                    observation: "ok proves local admission; inspect the projected message by client_request_id, sync_evidence for peer-relative propagation, and signed acknowledgements for recipient observation or handling".to_string(),
                },
                CommandSemanticsView {
                    command_id: "message.acknowledge".to_string(),
                    retry: "semantic idempotent for the same state and result_event_id; handled is monotonic, and rebinding a locally known handled result is rejected".to_string(),
                    observation: "the signed acknowledgement is an admitted participant assertion, not proof that the work was correct; optional handled result_event_id must name the handler's visible admitted reply threaded to the target, while observed must omit it; concurrent device results are retained and projected as a conflict".to_string(),
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
            replay_policy: "none; on every connect or reconnect, use service.ready.current_sequence and fetch coordination_snapshot_url until its current_sequence is at least that value".to_string(),
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
            state
                .shell
                .execute_serialized_command("shell.refresh", Value::Null),
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
    let Ok(result) = time::timeout(
        Duration::from_secs(30),
        run_command(&state.shell, &command_id, payload),
    )
    .await
    else {
        return StatusCode::GATEWAY_TIMEOUT.into_response();
    };
    drop(command_guard);
    let status = if result.ok {
        notify_snapshot_change(&state.snapshot_changes);
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result)).into_response()
}

async fn run_command(shell: &ShellState, command_id: &str, payload: Value) -> ActionResult {
    let activity_cursor = shell.activity_cursor().await;
    let result = shell.execute_serialized_command(command_id, payload).await;
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
    }
    value
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

    #[tokio::test]
    async fn action_result_reuses_the_shell_recovery_classification() {
        let dir = tempfile::tempdir().expect("tempdir");
        let shell = ShellState::new(dir.path().join("home"));
        let result = run_command(&shell, "not_a_command", serde_json::json!({})).await;
        assert_eq!(result.recovery, Some(ShellRecovery::InternalError));
        assert_eq!(
            result.error.expect("structured error").recovery,
            ShellRecovery::InternalError
        );

        let initialized = run_command(
            &shell,
            "home.init",
            serde_json::json!({ "default_room": null }),
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

        let stopped = run_command(&shell, "runtime.goOffline", serde_json::json!({})).await;
        assert_eq!(stopped.activity_items.len(), 1);
        assert_eq!(stopped.activity_items[0].summary, "service stopped");
        assert!(stopped.activity_items[0].id > initialized_last_id);

        let input = run_command(
            &shell,
            "message.search",
            serde_json::json!({ "query": " ", "room": null, "limit": 10 }),
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
