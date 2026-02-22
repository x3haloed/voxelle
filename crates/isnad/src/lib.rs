use anyhow::{anyhow, Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const STATUSES: [&str; 6] = ["backlog", "next", "doing", "blocked", "done", "rejected"];
pub const PRIORITIES: [&str; 4] = ["low", "medium", "high", "urgent"];

pub fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn new_id(prefix: &str, suffix_hex_len: usize) -> String {
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut hex = Uuid::new_v4().simple().to_string();
    hex.truncate(suffix_hex_len);
    format!("{prefix}_{ts}_{hex}")
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub isnad_dir: PathBuf,
    pub ledger: PathBuf,
    pub control: PathBuf,
    pub state_dir: PathBuf,
    pub board_json: PathBuf,
    pub board_md: PathBuf,
    pub cursors: PathBuf,
}

pub fn paths_for(root: impl AsRef<Path>) -> Paths {
    let root = root.as_ref().to_path_buf();
    let isnad_dir = root.join(".isnad");
    let state_dir = isnad_dir.join("state");
    Paths {
        root,
        isnad_dir: isnad_dir.clone(),
        ledger: isnad_dir.join("ledger.jsonl"),
        control: isnad_dir.join("control.jsonl"),
        state_dir: state_dir.clone(),
        board_json: state_dir.join("board.json"),
        board_md: state_dir.join("board.md"),
        cursors: state_dir.join("cursors.json"),
    }
}

fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create dir {}", path.display()))
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<()> {
    ensure_dir(
        path.parent()
            .ok_or_else(|| anyhow!("no parent for {}", path.display()))?,
    )?;
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn append_jsonl(path: &Path, value: &Value) -> Result<()> {
    ensure_dir(
        path.parent()
            .ok_or_else(|| anyhow!("no parent for {}", path.display()))?,
    )?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

pub fn scaffold(root: impl AsRef<Path>, force: bool) -> Result<Paths> {
    let p = paths_for(root);
    ensure_dir(&p.state_dir)?;

    if !p.ledger.exists() {
        let init = serde_json::json!({
            "id": new_id("L", 12),
            "ts": utc_now(),
            "type": "init",
            "claim": "Initialized isnad workspace.",
            "action": "Created .isnad directory and initial state files.",
            "artifact": { "path": ".isnad" },
            "evidence": { "cwd": p.root.display().to_string() },
            "next_decision": "continue",
            "meta": { "scaffold_version": 1, "actor": "agent" }
        });
        append_jsonl(&p.ledger, &init)?;
    }

    if !p.control.exists() {
        fs::write(&p.control, "").with_context(|| format!("write {}", p.control.display()))?;
    }

    if force || !p.board_json.exists() {
        let mut columns = Map::new();
        for status in STATUSES {
            columns.insert(status.to_string(), Value::Array(vec![]));
        }
        let empty_board = Value::Object(
            [
                ("generated_at".to_string(), Value::String(utc_now())),
                ("columns".to_string(), Value::Object(columns)),
                ("cards".to_string(), Value::Object(Map::new())),
                ("unread_directives".to_string(), Value::Object(Map::new())),
            ]
            .into_iter()
            .collect(),
        );
        write_json_pretty(&p.board_json, &empty_board)?;
    }

    if force || !p.board_md.exists() {
        fs::write(
            &p.board_md,
            "# Board (derived)\n\nRun `cargo run -p voxelle-board -- fold` to regenerate.\n",
        )
        .with_context(|| format!("write {}", p.board_md.display()))?;
    }

    if force || !p.cursors.exists() {
        let cursors = serde_json::json!({
            "generated_at": utc_now(),
            "control_ack_cursor": null,
            "last_seen_control_seq": 0,
            "last_ack_control_seq": 0,
            "folded_control_bytes": 0,
            "folded_ledger_bytes": 0
        });
        write_json_pretty(&p.cursors, &cursors)?;
    }

    Ok(p)
}

fn read_jsonl_with_seq(path: &Path) -> Result<Vec<Map<String, Value>>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut seq: i64 = 0;
    let mut out = vec![];
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        seq += 1;
        obj.insert("_seq".to_string(), Value::Number(seq.into()));
        out.push(obj);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct Card {
    task_id: String,
    title: String,
    status: String,
    priority: String,
    updated_at: String,
    updated_seq: i64,
    latest_snapshot_id: Option<String>,
    provisional: bool,
}

fn set_updated(card: &mut Card, ts: &str, seq: i64) {
    if seq >= card.updated_seq {
        card.updated_seq = seq;
        if !ts.is_empty() {
            card.updated_at = ts.to_string();
        }
    }
}

fn is_status(s: &str) -> bool {
    STATUSES.iter().any(|x| *x == s)
}

fn is_priority(p: &str) -> bool {
    PRIORITIES.iter().any(|x| *x == p)
}

fn priority_rank(p: &str) -> i64 {
    match p {
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "urgent" => 4,
        _ => 0,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardOut {
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub updated_at: String,
    pub updated_seq: i64,
    pub latest_snapshot_id: Option<String>,
    pub unread_directive_count: usize,
    pub provisional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub generated_at: String,
    pub columns: HashMap<String, Vec<CardOut>>,
    pub cards: HashMap<String, CardOut>,
    pub unread_directives: HashMap<String, Vec<String>>,
    pub last_ack_directive_id: Option<String>,
    pub last_ack_directive_ts: Option<String>,
    pub last_ack_control_seq: i64,
}

pub fn fold(root: impl AsRef<Path>) -> Result<Board> {
    let p = paths_for(root);

    let ledger = read_jsonl_with_seq(&p.ledger)?;
    let control = read_jsonl_with_seq(&p.control)?;

    let mut cards: HashMap<String, Card> = HashMap::new();
    let mut unread_directives: HashMap<String, Vec<String>> = HashMap::new();
    let mut acked_directives: HashSet<String> = HashSet::new();
    let mut last_ack_directive_id: Option<String> = None;
    let mut last_ack_directive_ts: Option<String> = None;
    let mut last_ack_control_seq: i64 = 0;

    for rec in &ledger {
        let rec_type = rec.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let ts = rec.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        let seq = rec.get("_seq").and_then(|v| v.as_i64()).unwrap_or(0);
        let task_id = rec.get("task_id").and_then(|v| v.as_str());

        if rec_type == "task_opened" {
            let Some(task_id) = task_id.filter(|t| !t.is_empty()) else {
                continue;
            };
            let mut title = rec.get("claim").and_then(|v| v.as_str()).unwrap_or("Untitled task");
            if let Some(Value::Object(meta)) = rec.get("meta") {
                if let Some(t) = meta.get("title").and_then(|v| v.as_str()) {
                    if !t.is_empty() {
                        title = t;
                    }
                }
            }
            let mut card = Card {
                task_id: task_id.to_string(),
                title: title.to_string(),
                status: "backlog".to_string(),
                priority: "medium".to_string(),
                updated_at: "".to_string(),
                updated_seq: 0,
                latest_snapshot_id: None,
                provisional: false,
            };
            set_updated(&mut card, ts, seq);
            cards.insert(task_id.to_string(), card);
        }

        if rec_type == "task_updated" {
            let Some(task_id) = task_id.filter(|t| !t.is_empty()) else {
                continue;
            };
            let Some(card) = cards.get_mut(task_id) else {
                continue;
            };
            if let Some(Value::Object(meta)) = rec.get("meta") {
                if let Some(t) = meta.get("title").and_then(|v| v.as_str()) {
                    if !t.is_empty() {
                        card.title = t.to_string();
                    }
                }
            }
            set_updated(card, ts, seq);
        }

        if rec_type == "snapshot" {
            let Some(task_id) = task_id.filter(|t| !t.is_empty()) else {
                continue;
            };
            let Some(card) = cards.get_mut(task_id) else {
                continue;
            };
            card.latest_snapshot_id = rec.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
            set_updated(card, ts, seq);
        }

        if rec_type == "ack_directive" {
            if let Some(Value::Object(meta)) = rec.get("meta") {
                if let Some(did) = meta.get("directive_id").and_then(|v| v.as_str()) {
                    if !did.is_empty() {
                        acked_directives.insert(did.to_string());
                        last_ack_directive_id = Some(did.to_string());
                        if !ts.is_empty() {
                            last_ack_directive_ts = Some(ts.to_string());
                        }
                    }
                }
            }
        }
    }

    for d in &control {
        let d_id = d.get("id").and_then(|v| v.as_str());
        let d_type = d.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let ts = d.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        let seq = d.get("_seq").and_then(|v| v.as_i64()).unwrap_or(0);
        let task_id = d.get("task_id").and_then(|v| v.as_str());
        let payload = d.get("payload").and_then(|v| v.as_object());

        if d_type == "open_task" {
            let Some(task_id) = task_id.filter(|t| !t.is_empty()) else {
                continue;
            };
            let title = payload.and_then(|p| p.get("title")).and_then(|v| v.as_str());
            let status = payload.and_then(|p| p.get("status")).and_then(|v| v.as_str());
            let priority = payload.and_then(|p| p.get("priority")).and_then(|v| v.as_str());

            let card = cards.entry(task_id.to_string()).or_insert_with(|| Card {
                task_id: task_id.to_string(),
                title: title.unwrap_or("Untitled task").to_string(),
                status: "backlog".to_string(),
                priority: "medium".to_string(),
                updated_at: "".to_string(),
                updated_seq: 0,
                latest_snapshot_id: None,
                provisional: true,
            });

            if let Some(t) = title {
                if !t.is_empty() && (card.title == "(unopened task)" || card.title == "Untitled task") {
                    card.title = t.to_string();
                }
            }

            if let Some(s) = status {
                if is_status(s) {
                    card.status = s.to_string();
                }
            }
            if let Some(pv) = priority {
                if is_priority(pv) {
                    card.priority = pv.to_string();
                }
            }
            set_updated(card, ts, seq);
        }

        if d_type != "open_task" {
            if let Some(task_id) = task_id.filter(|t| !t.is_empty()) {
                cards.entry(task_id.to_string()).or_insert_with(|| Card {
                    task_id: task_id.to_string(),
                    title: "(unopened task)".to_string(),
                    status: "backlog".to_string(),
                    priority: "medium".to_string(),
                    updated_at: "".to_string(),
                    updated_seq: 0,
                    latest_snapshot_id: None,
                    provisional: true,
                });
            }
        }

        if let Some(task_id) = task_id.filter(|t| !t.is_empty()) {
            if let Some(card) = cards.get_mut(task_id) {
                if d_type == "set_status" {
                    if let Some(s) = payload.and_then(|p| p.get("status")).and_then(|v| v.as_str()) {
                        if is_status(s) {
                            card.status = s.to_string();
                            set_updated(card, ts, seq);
                        }
                    }
                }

                if d_type == "set_priority" {
                    if let Some(pr) = payload
                        .and_then(|p| p.get("priority"))
                        .and_then(|v| v.as_str())
                    {
                        if is_priority(pr) {
                            card.priority = pr.to_string();
                            set_updated(card, ts, seq);
                        }
                    }
                }

                if d_type == "pause" {
                    card.status = "blocked".to_string();
                    set_updated(card, ts, seq);
                }

                if let Some(d_id) = d_id.filter(|id| !id.is_empty()) {
                    if !acked_directives.contains(d_id) {
                        unread_directives
                            .entry(task_id.to_string())
                            .or_default()
                            .push(d_id.to_string());
                    } else {
                        last_ack_control_seq = last_ack_control_seq.max(seq);
                    }
                }
            }
        }
    }

    let mut columns: HashMap<String, Vec<CardOut>> =
        STATUSES.iter().map(|s| (s.to_string(), vec![])).collect();
    let mut cards_out: HashMap<String, CardOut> = HashMap::new();

    for (task_id, card) in cards {
        let unread = unread_directives.get(&task_id).map(|v| v.len()).unwrap_or(0);
        let out = CardOut {
            task_id: card.task_id,
            title: card.title,
            status: card.status.clone(),
            priority: card.priority.clone(),
            updated_at: card.updated_at,
            updated_seq: card.updated_seq,
            latest_snapshot_id: card.latest_snapshot_id,
            unread_directive_count: unread,
            provisional: card.provisional,
        };
        cards_out.insert(task_id.clone(), out.clone());
        if let Some(col) = columns.get_mut(&out.status) {
            col.push(out);
        }
    }

    for status in STATUSES {
        if let Some(col) = columns.get_mut(status) {
            col.sort_by(|a, b| {
                let ra = priority_rank(&a.priority);
                let rb = priority_rank(&b.priority);
                (rb, b.updated_seq).cmp(&(ra, a.updated_seq))
            });
        }
    }

    Ok(Board {
        generated_at: utc_now(),
        columns,
        cards: cards_out,
        unread_directives,
        last_ack_directive_id,
        last_ack_directive_ts,
        last_ack_control_seq,
    })
}

pub fn render_markdown(board: &Board) -> String {
    let mut out = String::new();
    out.push_str("# Board (derived)\n\n");
    out.push_str(&format!("Generated: {}\n\n", board.generated_at));

    for status in STATUSES {
        let mut chars = status.chars();
        let heading = match chars.next() {
            Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
            None => status.to_string(),
        };
        out.push_str(&format!("## {heading}\n"));
        if let Some(col) = board.columns.get(status) {
            for card in col {
                let provisional = if card.provisional { " (provisional)" } else { "" };
                let suffix = if card.unread_directive_count > 0 {
                    format!(" (unread:{})", card.unread_directive_count)
                } else {
                    "".to_string()
                };
                out.push_str(&format!(
                    "- [{}] {}{}  ({}){}\n",
                    card.task_id, card.title, provisional, card.priority, suffix
                ));
            }
        }
        out.push('\n');
    }

    out
}

pub fn write_state(root: impl AsRef<Path>, board: &Board) -> Result<(PathBuf, PathBuf)> {
    let p = paths_for(root);
    ensure_dir(&p.state_dir)?;

    let json = serde_json::to_value(board)?;
    write_json_pretty(&p.board_json, &json)?;
    fs::write(&p.board_md, render_markdown(board)).with_context(|| format!("write {}", p.board_md.display()))?;
    Ok((p.board_json, p.board_md))
}

pub fn read_jsonl_values(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut out = vec![];
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if val.is_object() {
            out.push(val);
        }
    }
    Ok(out)
}
