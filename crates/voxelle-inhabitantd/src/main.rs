use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpListener, signal, time};
use tracing::info;
use voxelle_app::{
    resolve_home_root, ShellError, ShellSnapshotView, ShellState, SHELL_COMMAND_IDS,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryView {
    surface_version: String,
    home_root: PathBuf,
    base_url: String,
    pid: u32,
    started_at_unix_ms: u128,
    snapshot_url: String,
    events_url: String,
    commands_url: String,
    capabilities: CapabilitiesView,
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
    snapshot: Option<ShellSnapshotView>,
    error: Option<ShellError>,
    recovery: Option<RecoveryKind>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum RecoveryKind {
    NeedsHome,
    NeedsServiceOnline,
    NeedsPeerRecord,
    NeedsReachability,
    NeedsSync,
    NeedsHuman,
    InternalError,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let home = resolve_home_root(cli.home);
    let listener = TcpListener::bind(SocketAddr::new(cli.host, cli.port))
        .await
        .context("bind inhabitant sidecar")?;
    let addr = listener.local_addr().context("read listener address")?;
    let base_url = format!("http://{addr}");
    let discovery_view = DiscoveryView::new(home.clone(), base_url);
    write_discovery_file(cli.discovery_file.as_deref(), &home, &discovery_view)
        .context("write discovery file")?;

    let state = Arc::new(AppState {
        shell: Arc::new(ShellState::new(home)),
        discovery: discovery_view,
    });
    let app = Router::new()
        .route("/inhabitant/v0/discovery", get(get_discovery))
        .route("/inhabitant/v0/snapshot", get(snapshot))
        .route("/inhabitant/v0/commands/:command_id", post(command))
        .route("/inhabitant/v0/events", get(events))
        .with_state(state);

    info!("Serving {}", app_url(addr));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve inhabitant sidecar")?;
    Ok(())
}

impl DiscoveryView {
    fn new(home_root: PathBuf, base_url: String) -> Self {
        Self {
            surface_version: "inhabitant.v0".to_string(),
            home_root,
            snapshot_url: format!("{base_url}/inhabitant/v0/snapshot"),
            events_url: format!("{base_url}/inhabitant/v0/events"),
            commands_url: format!("{base_url}/inhabitant/v0/commands/{{command_id}}"),
            base_url,
            pid: std::process::id(),
            started_at_unix_ms: unix_ms(),
            capabilities: CapabilitiesView {
                commands: SHELL_COMMAND_IDS.iter().map(ToString::to_string).collect(),
                events: vec!["service.ready".to_string(), "heartbeat".to_string()],
            },
        }
    }
}

async fn get_discovery(State(state): State<Arc<AppState>>) -> Json<DiscoveryView> {
    Json(state.discovery.clone())
}

async fn snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state
        .shell
        .execute_serialized_command("shell.refresh", Value::Null)
        .await
    {
        Ok(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response(),
    }
}

async fn command(
    State(state): State<Arc<AppState>>,
    Path(command_id): Path<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let result = run_command(&state.shell, &command_id, payload).await;
    let status = if result.ok {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result)).into_response()
}

async fn run_command(shell: &ShellState, command_id: &str, payload: Value) -> ActionResult {
    let result = shell.execute_serialized_command(command_id, payload).await;
    match result {
        Ok(snapshot) => ActionResult {
            ok: true,
            command_id: command_id.to_string(),
            snapshot: Some(snapshot),
            error: None,
            recovery: None,
        },
        Err(error) => ActionResult {
            ok: false,
            command_id: command_id.to_string(),
            snapshot: None,
            recovery: Some(classify_recovery(&error.message)),
            error: Some(error),
        },
    }
}

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let discovery = state.discovery.clone();
    let stream = stream::unfold(EventState::Ready(discovery), |state| async move {
        match state {
            EventState::Ready(discovery) => {
                let ready = serde_json::json!({
                    "surface_version": discovery.surface_version,
                    "home_root": discovery.home_root,
                    "base_url": discovery.base_url,
                });
                Some((
                    Ok(Event::default()
                        .event("service.ready")
                        .data(ready.to_string())),
                    EventState::Heartbeat,
                ))
            }
            EventState::Heartbeat => {
                time::sleep(Duration::from_secs(30)).await;
                let heartbeat = serde_json::json!({
                    "at_unix_ms": unix_ms(),
                    "pid": std::process::id(),
                });
                Some((
                    Ok(Event::default()
                        .event("heartbeat")
                        .data(heartbeat.to_string())),
                    EventState::Heartbeat,
                ))
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

enum EventState {
    Ready(DiscoveryView),
    Heartbeat,
}

fn classify_recovery(message: &str) -> RecoveryKind {
    let lower = message.to_ascii_lowercase();
    if lower.contains("identity.json") || lower.contains("home") {
        RecoveryKind::NeedsHome
    } else if lower.contains("service") || lower.contains("offline") {
        RecoveryKind::NeedsServiceOnline
    } else if lower.contains("peer record") || lower.contains("unknown peer") {
        RecoveryKind::NeedsPeerRecord
    } else if lower.contains("diagnostic") || lower.contains("reach") || lower.contains("connect") {
        RecoveryKind::NeedsReachability
    } else if lower.contains("sync") {
        RecoveryKind::NeedsSync
    } else if lower.contains("permission") || lower.contains("manual") {
        RecoveryKind::NeedsHuman
    } else {
        RecoveryKind::InternalError
    }
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
    std::fs::write(&path, format!("{json}\n"))
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
