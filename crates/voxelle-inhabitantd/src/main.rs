use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, Request, StatusCode},
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
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpListener, signal, sync::Semaphore, time};
use tracing::info;
use voxelle_app::{
    resolve_home_root, shell_command_ids, ShellError, ShellSnapshotView, ShellState,
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
    event_slots: Arc<Semaphore>,
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
    authorization: String,
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

    let state = Arc::new(AppState {
        shell: Arc::new(ShellState::new(home)),
        discovery: discovery_view,
        bearer_token: Arc::from(bearer_token),
        request_slots: Arc::new(Semaphore::new(8)),
        event_slots: Arc::new(Semaphore::new(8)),
    });
    let app = Router::new()
        .route("/inhabitant/v0/discovery", get(get_discovery))
        .route("/inhabitant/v0/snapshot", get(snapshot))
        .route("/inhabitant/v0/commands/:command_id", post(command))
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
            events_url: format!("{base_url}/inhabitant/v0/events"),
            commands_url: format!("{base_url}/inhabitant/v0/commands/{{command_id}}"),
            authorization: format!("Bearer {bearer_token}"),
            base_url,
            pid: std::process::id(),
            started_at_unix_ms: unix_ms(),
            capabilities: CapabilitiesView {
                commands: shell_command_ids(),
                events: vec!["service.ready".to_string(), "heartbeat".to_string()],
            },
        }
    }
}

async fn get_discovery(State(state): State<Arc<AppState>>) -> Json<DiscoveryView> {
    Json(state.discovery.clone())
}

async fn snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Ok(Ok(_permit)) =
        time::timeout(Duration::from_secs(1), state.request_slots.acquire()).await
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
    let Ok(result) = time::timeout(
        Duration::from_secs(30),
        run_command(&state.shell, &command_id, payload),
    )
    .await
    else {
        return StatusCode::GATEWAY_TIMEOUT.into_response();
    };
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
    let stream = stream::unfold(
        EventState::Ready(Box::new(discovery), permit),
        |state| async move {
            match state {
                EventState::Ready(discovery, permit) => {
                    let ready = serde_json::json!({
                        "surface_version": discovery.surface_version,
                        "home_root": discovery.home_root,
                        "base_url": discovery.base_url,
                    });
                    Some((
                        Ok::<Event, Infallible>(
                            Event::default()
                                .event("service.ready")
                                .data(ready.to_string()),
                        ),
                        EventState::Heartbeat(permit),
                    ))
                }
                EventState::Heartbeat(permit) => {
                    time::sleep(Duration::from_secs(30)).await;
                    let heartbeat = serde_json::json!({
                        "at_unix_ms": unix_ms(),
                        "pid": std::process::id(),
                    });
                    Some((
                        Ok::<Event, Infallible>(
                            Event::default()
                                .event("heartbeat")
                                .data(heartbeat.to_string()),
                        ),
                        EventState::Heartbeat(permit),
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
    Ready(Box<DiscoveryView>, tokio::sync::OwnedSemaphorePermit),
    Heartbeat(tokio::sync::OwnedSemaphorePermit),
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
