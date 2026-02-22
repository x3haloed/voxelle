use anyhow::{Context, Result};
use axum::{
    extract::{State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use isnad::{
    append_jsonl, fold, new_id, paths_for, read_jsonl_values, scaffold, utc_now, write_state, Board,
};
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

const HTML: &str = include_str!("ui.html");

#[derive(Debug, Parser)]
#[command(name = "voxelle-board", about = "Local Work Board UI for .isnad (derived; writes control only).")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long)]
        force: bool,
    },
    Fold {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long)]
        watch: bool,
        #[arg(long, default_value_t = 0.75)]
        interval: f64,
    },
    Serve {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8787)]
        port: u16,
        #[arg(long)]
        no_open: bool,
        #[arg(long, default_value = "human")]
        author: String,
        #[arg(long, default_value = "board-ui")]
        via: String,
        #[arg(long, default_value = "")]
        operator: String,
    },
    AppendDirective {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long, value_name = "TYPE")]
        r#type: String,
        #[arg(long, value_name = "TASK_ID")]
        task: Option<String>,
        #[arg(long, default_value = "{}")]
        payload: String,
        #[arg(long, default_value = "")]
        rationale: String,
        #[arg(long, default_value = "human")]
        author: String,
        #[arg(long, default_value = "{}")]
        meta: String,
    },
    AppendLedger {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long, value_name = "TYPE")]
        r#type: String,
        #[arg(long, default_value = "")]
        topic: String,
        #[arg(long, value_name = "TASK_ID", default_value = "")]
        task: String,
        #[arg(long, default_value = "")]
        claim: String,
        #[arg(long, default_value = "")]
        action: String,
        #[arg(long, default_value = "")]
        artifact: String,
        #[arg(long, default_value = "")]
        evidence: String,
        #[arg(long, default_value = "")]
        next: String,
        #[arg(long, default_value = "{}")]
        meta: String,
    },
    AckDirectives {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long, default_value_t = 0)]
        limit: usize,
        #[arg(long, default_value = "agent")]
        actor: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    author: String,
    via: String,
    operator: Option<String>,
}

fn normalize_root(root: &str) -> Result<PathBuf> {
    Ok(Path::new(root).canonicalize().with_context(|| format!("canonicalize {root}"))?)
}

async fn index() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8")),
        ],
        HTML,
    )
}

async fn api_board(State(state): State<Arc<AppState>>) -> Result<Json<Board>, (StatusCode, String)> {
    let board = fold(&state.root).map_err(internal_error)?;
    write_state(&state.root, &board).map_err(internal_error)?;
    Ok(Json(board))
}

#[derive(Debug, Deserialize)]
struct OpenTaskReq {
    payload: Option<Value>,
}

async fn api_open_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OpenTaskReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    scaffold(&state.root, false).map_err(internal_error)?;
    let p = paths_for(&state.root);

    let task_id = new_id("T", 8);
    let directive = serde_json::json!({
        "id": new_id("D", 12),
        "ts": utc_now(),
        "type": "open_task",
        "task_id": task_id,
        "author": state.author,
        "meta": {
            "via": state.via,
            "operator": state.operator,
        },
        "payload": req.payload.unwrap_or_else(|| serde_json::json!({}))
    });
    append_jsonl(&p.control, &directive).map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true, "directive_id": directive["id"], "task_id": directive["task_id"]})))
}

#[derive(Debug, Deserialize)]
struct DirectiveReq {
    #[serde(rename = "type")]
    d_type: String,
    task_id: Option<String>,
    payload: Option<Value>,
}

async fn api_directives(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DirectiveReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    scaffold(&state.root, false).map_err(internal_error)?;
    let p = paths_for(&state.root);

    if req.d_type.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing type".to_string()));
    }
    let needs_task = matches!(
        req.d_type.as_str(),
        "set_status" | "set_priority" | "pause" | "resume" | "note"
    );
    if needs_task && req.task_id.as_deref().unwrap_or("").is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing task_id".to_string()));
    }

    let directive = serde_json::json!({
        "id": new_id("D", 12),
        "ts": utc_now(),
        "type": req.d_type,
        "task_id": req.task_id,
        "author": state.author,
        "meta": {
            "via": state.via,
            "operator": state.operator,
        },
        "payload": req.payload.unwrap_or_else(|| serde_json::json!({}))
    });
    append_jsonl(&p.control, &directive).map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true, "directive_id": directive["id"], "task_id": directive["task_id"]})))
}

fn internal_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn validate_task_id(task_id: &str) -> Result<()> {
    if task_id.is_empty() || task_id.len() > 64 {
        anyhow::bail!("Invalid task id: must be 1-64 chars");
    }
    let mut chars = task_id.chars();
    let Some(first) = chars.next() else {
        anyhow::bail!("Invalid task id: empty");
    };
    if !first.is_ascii_alphanumeric() {
        anyhow::bail!("Invalid task id: must start with letter or digit");
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            anyhow::bail!("Invalid task id: only letters/digits/_/- allowed");
        }
    }
    Ok(())
}

fn parse_json_object(s: &str, what: &str) -> Result<Value> {
    let val: Value = serde_json::from_str(s).with_context(|| format!("parse {what} as JSON"))?;
    if !val.is_object() {
        anyhow::bail!("{what} must be a JSON object");
    }
    Ok(val)
}

fn read_acknowledged_directive_ids(ledger_path: &Path) -> Result<std::collections::HashSet<String>> {
    let mut acked = std::collections::HashSet::new();
    for rec in read_jsonl_values(ledger_path)? {
        let Some(obj) = rec.as_object() else {
            continue;
        };
        let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if typ != "ack_directive" {
            continue;
        }
        let Some(meta) = obj.get("meta").and_then(|v| v.as_object()) else {
            continue;
        };
        let Some(did) = meta.get("directive_id").and_then(|v| v.as_str()) else {
            continue;
        };
        if !did.is_empty() {
            acked.insert(did.to_string());
        }
    }
    Ok(acked)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Command::Init { root, force } => {
            let root = normalize_root(&root)?;
            scaffold(&root, force)?;
            let board = fold(&root)?;
            write_state(&root, &board)?;
            info!("Initialized .isnad at {}", root.display());
        }
        Command::Fold {
            root,
            watch,
            interval,
        } => {
            let root = normalize_root(&root)?;
            scaffold(&root, false)?;
            let board = fold(&root)?;
            let (json_path, md_path) = write_state(&root, &board)?;
            info!("Wrote {}", json_path.display());
            info!("Wrote {}", md_path.display());

            if watch {
                let p = paths_for(&root);
                let mut last = stat_key(&p.ledger, &p.control);
                info!(
                    "Watching for changes every {}s (Ctrl-C to stop).",
                    interval.max(0.1)
                );
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs_f64(interval.max(0.1))).await;
                    let cur = stat_key(&p.ledger, &p.control);
                    if cur == last {
                        continue;
                    }
                    last = cur;
                    let board = fold(&root)?;
                    let (json_path, md_path) = write_state(&root, &board)?;
                    info!("Wrote {}", json_path.display());
                    info!("Wrote {}", md_path.display());
                }
            }
        }
        Command::Serve {
            root,
            host,
            port,
            no_open,
            author,
            via,
            operator,
        } => {
            let root = normalize_root(&root)?;
            scaffold(&root, false)?;

            let state = Arc::new(AppState {
                root: root.clone(),
                author,
                via,
                operator: if operator.trim().is_empty() {
                    None
                } else {
                    Some(operator)
                },
            });

            let app = Router::new()
                .route("/", get(index))
                .route("/api/board", get(api_board).post(api_board))
                .route("/api/open_task", post(api_open_task))
                .route("/api/directives", post(api_directives))
                .with_state(state);

            let addr: SocketAddr = format!("{host}:{port}").parse().context("parse bind addr")?;
            let url = format!("http://{host}:{port}/");
            info!("Serving {url}");
            info!("Derived UI: writes control directives only; does not edit ledger.");

            if !no_open {
                if let Err(e) = open::that_detached(&url) {
                    error!("Failed to open browser: {e}");
                }
            }

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
        Command::AppendDirective {
            root,
            r#type,
            task,
            payload,
            rationale,
            author,
            meta,
        } => {
            let root = normalize_root(&root)?;
            scaffold(&root, false)?;
            let p = paths_for(&root);

            let task_scoped =
                matches!(r#type.as_str(), "set_status" | "set_priority" | "pause" | "resume" | "note");
            if task_scoped && task.as_deref().unwrap_or("").is_empty() {
                anyhow::bail!("--task is required for --type {type}", type = r#type);
            }
            if let Some(task_id) = task.as_deref() {
                if !task_id.is_empty() {
                    validate_task_id(task_id)?;
                }
            }

            let payload_val = parse_json_object(&payload, "payload")?;
            let meta_val = parse_json_object(&meta, "meta")?;

            let mut directive = serde_json::json!({
                "id": new_id("D", 12),
                "ts": utc_now(),
                "type": r#type,
                "author": author,
                "meta": meta_val,
                "payload": payload_val
            });
            if let Some(task_id) = task {
                if !task_id.trim().is_empty() {
                    directive["task_id"] = Value::String(task_id);
                }
            }
            if !rationale.trim().is_empty() {
                directive["rationale"] = Value::String(rationale);
            }

            append_jsonl(&p.control, &directive)?;
            info!("Appended directive {} to {}", directive["id"], p.control.display());
        }
        Command::AppendLedger {
            root,
            r#type,
            topic,
            task,
            claim,
            action,
            artifact,
            evidence,
            next,
            meta,
        } => {
            let root = normalize_root(&root)?;
            scaffold(&root, false)?;
            let p = paths_for(&root);

            let meta_val = parse_json_object(&meta, "meta")?;
            let mut record = serde_json::json!({
                "id": new_id("L", 12),
                "ts": utc_now(),
                "type": r#type,
                "meta": meta_val
            });
            if !topic.trim().is_empty() {
                record["topic"] = Value::String(topic);
            }
            if !task.trim().is_empty() {
                validate_task_id(&task)?;
                record["task_id"] = Value::String(task);
            }
            if !claim.trim().is_empty() {
                record["claim"] = Value::String(claim);
            }
            if !action.trim().is_empty() {
                record["action"] = Value::String(action);
            }
            if !artifact.trim().is_empty() {
                record["artifact"] = Value::String(artifact);
            }
            if !evidence.trim().is_empty() {
                record["evidence"] = Value::String(evidence);
            }
            if !next.trim().is_empty() {
                record["next_decision"] = Value::String(next);
            }

            append_jsonl(&p.ledger, &record)?;
            info!("Appended record {} to {}", record["id"], p.ledger.display());
        }
        Command::AckDirectives {
            root,
            limit,
            actor,
            dry_run,
        } => {
            let root = normalize_root(&root)?;
            scaffold(&root, false)?;
            let p = paths_for(&root);

            let acked = read_acknowledged_directive_ids(&p.ledger)?;
            let mut to_ack: Vec<Value> = vec![];

            for d in read_jsonl_values(&p.control)? {
                let Some(obj) = d.as_object() else {
                    continue;
                };
                let did = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if did.is_empty() || acked.contains(did) {
                    continue;
                }
                to_ack.push(d);
                if limit > 0 && to_ack.len() >= limit {
                    break;
                }
            }

            if to_ack.is_empty() {
                info!("No unread directives found.");
                return Ok(());
            }

            for d in to_ack {
                let obj = d.as_object().expect("object");
                let did = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let task_id = obj.get("task_id").cloned().unwrap_or(Value::Null);
                let receipt = serde_json::json!({
                    "id": new_id("L", 12),
                    "ts": utc_now(),
                    "type": "ack_directive",
                    "task_id": task_id,
                    "claim": format!("Acknowledged directive {did}."),
                    "action": "Recorded receipt of human intent; will follow up with actions/tests or cannot_comply.",
                    "evidence": { "control_id": did },
                    "next_decision": "continue",
                    "meta": { "directive_id": did, "ack_actor": actor }
                });
                if dry_run {
                    println!("{}", serde_json::to_string_pretty(&receipt)?);
                } else {
                    append_jsonl(&p.ledger, &receipt)?;
                    info!("acked {} -> {}", receipt["meta"]["directive_id"], receipt["id"]);
                }
            }
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}

fn stat_key(ledger: &Path, control: &Path) -> (u64, u128, u64, u128) {
    fn one(path: &Path) -> (u64, u128) {
        let Ok(meta) = std::fs::metadata(path) else {
            return (0, 0);
        };
        let size = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        (size, mtime)
    }
    let (ls, lm) = one(ledger);
    let (cs, cm) = one(control);
    (ls, lm, cs, cm)
}
