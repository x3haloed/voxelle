use anyhow::{Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use clap::Parser;
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tokio::signal;
use tracing::info;

#[derive(Debug, Parser)]
#[command(
    name = "voxelle-signal",
    about = "Untrusted, optional WebSocket signaling relay for Voxelle WebRTC offer/answer exchange."
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 9002)]
    port: u16,
    #[arg(long, default_value_t = 3600)]
    ttl_seconds: u64,
}

#[derive(Clone)]
struct AppState {
    ttl: Duration,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

#[derive(Debug, Clone)]
struct Session {
    created_at: SystemTime,
    offer: Option<String>,
    answer: Option<String>,
    clients: Vec<tokio::sync::mpsc::UnboundedSender<Message>>,
}

#[derive(Debug, Serialize)]
struct Info {
    name: &'static str,
    v: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "t")]
enum ClientMsg {
    #[serde(rename = "join")]
    Join { v: u32, sid: String },
    #[serde(rename = "set_offer")]
    SetOffer { v: u32, sid: String, offer: String },
    #[serde(rename = "set_answer")]
    SetAnswer { v: u32, sid: String, answer: String },
    #[serde(rename = "get_state")]
    GetState { v: u32, sid: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "t")]
enum ServerMsg {
    #[serde(rename = "hello")]
    Hello { v: u32 },
    #[serde(rename = "state")]
    State {
        v: u32,
        sid: String,
        has_offer: bool,
        has_answer: bool,
        offer: Option<String>,
        answer: Option<String>,
    },
    #[serde(rename = "error")]
    Error { v: u32, error: String },
}

fn json_msg<T: Serialize>(v: T) -> Message {
    Message::Text(serde_json::to_string(&v).unwrap_or_else(|_| "{\"t\":\"error\",\"v\":1,\"error\":\"encode\"}".into()))
}

fn purge_expired(state: &AppState) {
    let mut sessions = state.sessions.lock().expect("lock");
    let ttl = state.ttl;
    let now = SystemTime::now();
    sessions.retain(|_, s| now.duration_since(s.created_at).unwrap_or_default() <= ttl);
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    purge_expired(&state);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let mut joined_sid: Option<String> = None;

    let (mut socket_tx, mut socket_rx) = socket.split();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let _ = socket_tx.send(msg).await;
        }
    });

    let _ = tx.send(json_msg(ServerMsg::Hello { v: 1 }));

    while let Some(Ok(msg)) = socket_rx.next().await {
        let Message::Text(text) = msg else { continue };
        let parsed: Result<ClientMsg> = serde_json::from_str(&text).context("parse");
        let Ok(cmd) = parsed else {
            let _ = tx.send(json_msg(ServerMsg::Error { v: 1, error: "invalid json".into() }));
            continue;
        };

        match cmd {
            ClientMsg::Join { v: 1, sid } => {
                joined_sid = Some(sid.clone());
                let mut sessions = state.sessions.lock().expect("lock");
                let entry = sessions.entry(sid.clone()).or_insert_with(|| Session {
                    created_at: SystemTime::now(),
                    offer: None,
                    answer: None,
                    clients: vec![],
                });
                entry.clients.push(tx.clone());
                let offer = entry.offer.clone();
                let answer = entry.answer.clone();
                let _ = tx.send(json_msg(ServerMsg::State {
                    v: 1,
                    sid,
                    has_offer: offer.is_some(),
                    has_answer: answer.is_some(),
                    offer,
                    answer,
                }));
            }
            ClientMsg::SetOffer { v: 1, sid, offer } => {
                let mut sessions = state.sessions.lock().expect("lock");
                let entry = sessions.entry(sid.clone()).or_insert_with(|| Session {
                    created_at: SystemTime::now(),
                    offer: None,
                    answer: None,
                    clients: vec![],
                });
                entry.offer = Some(offer);
                let broadcast = json_msg(ServerMsg::State {
                    v: 1,
                    sid: sid.clone(),
                    has_offer: true,
                    has_answer: entry.answer.is_some(),
                    offer: entry.offer.clone(),
                    answer: entry.answer.clone(),
                });
                entry.clients.retain(|c| c.send(broadcast.clone()).is_ok());
            }
            ClientMsg::SetAnswer { v: 1, sid, answer } => {
                let mut sessions = state.sessions.lock().expect("lock");
                let entry = sessions.entry(sid.clone()).or_insert_with(|| Session {
                    created_at: SystemTime::now(),
                    offer: None,
                    answer: None,
                    clients: vec![],
                });
                entry.answer = Some(answer);
                let broadcast = json_msg(ServerMsg::State {
                    v: 1,
                    sid: sid.clone(),
                    has_offer: entry.offer.is_some(),
                    has_answer: true,
                    offer: entry.offer.clone(),
                    answer: entry.answer.clone(),
                });
                entry.clients.retain(|c| c.send(broadcast.clone()).is_ok());
            }
            ClientMsg::GetState { v: 1, sid } => {
                let sessions = state.sessions.lock().expect("lock");
                let Some(entry) = sessions.get(&sid) else {
                    let _ = tx.send(json_msg(ServerMsg::Error { v: 1, error: "unknown sid".into() }));
                    continue;
                };
                let _ = tx.send(json_msg(ServerMsg::State {
                    v: 1,
                    sid,
                    has_offer: entry.offer.is_some(),
                    has_answer: entry.answer.is_some(),
                    offer: entry.offer.clone(),
                    answer: entry.answer.clone(),
                }));
            }
            _ => {
                let _ = tx.send(json_msg(ServerMsg::Error { v: 1, error: "unsupported version".into() }));
            }
        }
    }

    if let Some(sid) = joined_sid {
        let mut sessions = state.sessions.lock().expect("lock");
        if let Some(entry) = sessions.get_mut(&sid) {
            entry.clients.retain(|c| !c.is_closed());
        }
    }
}

async fn info_handler() -> impl IntoResponse {
    Json(Info { name: "voxelle-signal", v: 1 })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse().context("parse addr")?;

    let state = AppState {
        ttl: Duration::from_secs(cli.ttl_seconds),
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/info", get(info_handler))
        .route("/ws", get(ws_handler))
        .with_state(state);

    info!("Serving signaling relay at ws://{}/ws (ttl={}s)", addr, cli.ttl_seconds);
    info!("This relay is untrusted: it forwards SDP/answer blobs only.");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}
