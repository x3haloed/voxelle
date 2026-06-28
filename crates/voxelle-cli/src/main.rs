use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use voxelle_app::{VoxelleHome, DEFAULT_ROOM_ID};
use voxelle_core::{
    accept_event, create_delegation, create_event, PeerIdentity, RoomContext, GOVERNANCE_ROOM_ID,
};
use voxelle_net::{PeerEndpoint, QuicCertificate, QuicNode};
use voxelle_store::Store;
use voxelle_sync::{sync_rooms_once, SyncLimits};

#[derive(Debug, Parser)]
#[command(
    name = "voxelle",
    about = "Voxelle IPv6-native local-first runtime CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init {
        #[arg(long)]
        home: PathBuf,
        #[arg(long, default_value = DEFAULT_ROOM_ID)]
        room: String,
    },
    Send {
        #[arg(long)]
        home: PathBuf,
        #[arg(long)]
        text: String,
        #[arg(long)]
        room: Option<String>,
    },
    Read {
        #[arg(long)]
        home: PathBuf,
        #[arg(long)]
        room: Option<String>,
    },
    Endpoint {
        #[command(subcommand)]
        command: EndpointCommand,
    },
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
    Room {
        #[command(subcommand)]
        command: RoomCommand,
    },
    Event {
        #[command(subcommand)]
        command: EventCommand,
    },
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
    Diagnose {
        #[command(subcommand)]
        command: DiagnoseCommand,
    },
}

#[derive(Debug, Subcommand)]
enum EndpointCommand {
    Export {
        #[arg(long)]
        home: PathBuf,
        #[arg(long)]
        advertise: SocketAddr,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum IdentityCommand {
    Create {
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum RoomCommand {
    Create {
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        store: PathBuf,
        #[arg(long, default_value = "room:general")]
        room: String,
    },
    Heads {
        #[arg(long)]
        store: PathBuf,
        #[arg(long, default_value = "room:general")]
        room: String,
    },
    Count {
        #[arg(long)]
        store: PathBuf,
        #[arg(long, default_value = "room:general")]
        room: String,
    },
}

#[derive(Debug, Subcommand)]
enum EventCommand {
    Send {
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        store: PathBuf,
        #[arg(long, default_value = "room:general")]
        room: String,
        #[arg(long)]
        text: String,
    },
}

#[derive(Debug, Subcommand)]
enum SyncCommand {
    Local {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        to: PathBuf,
        #[arg(long)]
        authority_peer_id: String,
        #[arg(long, default_value = "room:general")]
        room: String,
        #[arg(long, default_value_t = 64)]
        max_events: usize,
    },
}

#[derive(Debug, Subcommand)]
enum DiagnoseCommand {
    Listen {
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        cert: PathBuf,
        #[arg(long, default_value = "[::1]:0")]
        bind: SocketAddr,
        #[arg(long)]
        advertise: Option<SocketAddr>,
    },
    Connect {
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        cert: PathBuf,
        #[arg(long)]
        endpoint: PathBuf,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentityFile {
    v: u8,
    peer_secret_b64: String,
    device_secret_b64: String,
    peer_id: String,
    device_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { home, room } => app_init(&home, &room),
        Command::Send { home, text, room } => app_send(&home, &text, room.as_deref()),
        Command::Read { home, room } => app_read(&home, room.as_deref()),
        Command::Endpoint { command } => match command {
            EndpointCommand::Export {
                home,
                advertise,
                out,
            } => app_endpoint_export(&home, advertise, out.as_deref()),
        },
        Command::Identity { command } => match command {
            IdentityCommand::Create { out } => identity_create(&out),
        },
        Command::Room { command } => match command {
            RoomCommand::Create {
                identity,
                store,
                room,
            } => room_create(&identity, &store, &room),
            RoomCommand::Heads { store, room } => room_heads(&store, &room),
            RoomCommand::Count { store, room } => room_count(&store, &room),
        },
        Command::Event { command } => match command {
            EventCommand::Send {
                identity,
                store,
                room,
                text,
            } => event_send(&identity, &store, &room, &text),
        },
        Command::Sync { command } => match command {
            SyncCommand::Local {
                from,
                to,
                authority_peer_id,
                room,
                max_events,
            } => sync_local(&from, &to, &authority_peer_id, &room, max_events),
        },
        Command::Diagnose { command } => match command {
            DiagnoseCommand::Listen {
                identity,
                cert,
                bind,
                advertise,
            } => diagnose_listen(&identity, &cert, bind, advertise).await,
            DiagnoseCommand::Connect {
                identity,
                cert,
                endpoint,
            } => diagnose_connect(&identity, &cert, &endpoint).await,
        },
    }
}

fn app_init(home: &Path, room: &str) -> Result<()> {
    let summary = VoxelleHome::new(home).init(room)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn app_send(home: &Path, text: &str, room: Option<&str>) -> Result<()> {
    let event = VoxelleHome::new(home).send_message(text, room)?;
    println!("{}", event.event_id);
    Ok(())
}

fn app_read(home: &Path, room: Option<&str>) -> Result<()> {
    let messages = VoxelleHome::new(home).read_messages(room)?;
    println!("{}", serde_json::to_string_pretty(&messages)?);
    Ok(())
}

fn app_endpoint_export(home: &Path, advertise: SocketAddr, out: Option<&Path>) -> Result<()> {
    let endpoint = VoxelleHome::new(home).export_endpoint(advertise)?;
    if let Some(out) = out {
        write_json(out, &endpoint)?;
    }
    println!("{}", serde_json::to_string_pretty(&endpoint)?);
    Ok(())
}

fn identity_create(out: &Path) -> Result<()> {
    let identity = PeerIdentity::generate()?;
    let file = IdentityFile {
        v: 1,
        peer_secret_b64: identity.peer.secret_key_b64(),
        device_secret_b64: identity.device.secret_key_b64(),
        peer_id: identity.peer.id.clone(),
        device_id: identity.device.id.clone(),
    };
    write_json(out, &file)?;
    println!("{}", file.peer_id);
    Ok(())
}

fn room_create(identity_path: &Path, store_path: &Path, room: &str) -> Result<()> {
    let identity = load_identity(identity_path)?;
    let store = Store::open(store_path)?;
    let context = RoomContext::new(identity.peer.id.clone());
    let join = member_join(&identity)?;
    let accepted = accept_event(&join, &[], &context, now_ms())
        .map_err(|e| anyhow::anyhow!("join rejected: {e:?}"))?;
    store.insert_accepted_event(accepted, now_ms())?;
    println!("room={room}");
    println!("authority={}", identity.peer.id);
    Ok(())
}

fn event_send(identity_path: &Path, store_path: &Path, room: &str, text: &str) -> Result<()> {
    let identity = load_identity(identity_path)?;
    let store = Store::open(store_path)?;
    let context = RoomContext::new(identity.peer.id.clone());
    let governance = store.room_events(GOVERNANCE_ROOM_ID)?;
    let event = create_event(
        &identity,
        create_delegation(
            &identity.peer,
            &identity.device,
            now_ms() - 60_000,
            now_ms() + 30 * 24 * 60 * 60_000,
            vec!["room:post".to_string()],
        )?,
        room,
        now_ms(),
        "MSG_POST",
        store.room_heads(room)?,
        serde_json::json!({ "text": text }),
    )?;
    let accepted = accept_event(&event, &governance, &context, now_ms())
        .map_err(|e| anyhow::anyhow!("event rejected: {e:?}"))?;
    store.insert_accepted_event(accepted, now_ms())?;
    println!("{}", event.event_id);
    Ok(())
}

fn room_heads(store_path: &Path, room: &str) -> Result<()> {
    let store = Store::open(store_path)?;
    for head in store.room_heads(room)? {
        println!("{head}");
    }
    Ok(())
}

fn room_count(store_path: &Path, room: &str) -> Result<()> {
    let store = Store::open(store_path)?;
    println!("{}", store.room_event_count(room)?);
    Ok(())
}

fn sync_local(
    from: &Path,
    to: &Path,
    authority_peer_id: &str,
    room: &str,
    max_events: usize,
) -> Result<()> {
    let source = Store::open(from)?;
    let dest = Store::open(to)?;
    let context = RoomContext::new(authority_peer_id);
    let stats = sync_rooms_once(
        &source,
        &dest,
        &[room],
        &context,
        now_ms(),
        SyncLimits {
            max_events_per_batch: max_events,
        },
    )?;
    println!(
        "offered={} accepted={} already_present={} rejected={} truncated={}",
        stats.offered, stats.accepted, stats.already_present, stats.rejected, stats.truncated
    );
    Ok(())
}

async fn diagnose_listen(
    identity_path: &Path,
    cert_path: &Path,
    bind: SocketAddr,
    advertise: Option<SocketAddr>,
) -> Result<()> {
    let identity = load_identity(identity_path)?;
    let certificate = load_or_create_certificate(cert_path)?;
    let node = QuicNode::bind_with_certificate(identity, certificate, bind)?;
    let advertised_addr = advertise.unwrap_or(node.local_addr()?);
    let endpoint = node.peer_endpoint(advertised_addr)?;
    let local = node.local_reachability_report(advertised_addr)?;

    println!("{}", serde_json::to_string_pretty(&endpoint)?);
    io::stdout().flush()?;
    eprintln!("{}", serde_json::to_string_pretty(&local)?);

    let report = node.serve_diagnostic_once().await?;
    eprintln!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

async fn diagnose_connect(
    identity_path: &Path,
    cert_path: &Path,
    endpoint_path: &Path,
) -> Result<()> {
    let identity = load_identity(identity_path)?;
    let certificate = load_or_create_certificate(cert_path)?;
    let endpoint: PeerEndpoint = read_json(endpoint_path)?;
    let node = QuicNode::bind_ipv6_loopback_with_certificate(identity, certificate)?;
    let report = node.diagnose_peer(&endpoint).await;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.reachable {
        anyhow::bail!(
            "peer was not reachable: {}",
            report.error.unwrap_or_else(|| "unknown error".to_string())
        );
    }
    Ok(())
}

fn member_join(identity: &PeerIdentity) -> Result<voxelle_core::EventV1> {
    create_event(
        identity,
        create_delegation(
            &identity.peer,
            &identity.device,
            now_ms() - 60_000,
            now_ms() + 30 * 24 * 60 * 60_000,
            vec!["room:join".to_string()],
        )?,
        GOVERNANCE_ROOM_ID,
        now_ms(),
        "MEMBER_JOIN",
        vec![],
        serde_json::json!({
            "peer_id": identity.peer.id,
            "peer_pub": identity.peer.spki_b64,
        }),
    )
}

fn load_identity(path: &Path) -> Result<PeerIdentity> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file: IdentityFile = serde_json::from_str(&raw).context("parse identity file")?;
    if file.v != 1 {
        anyhow::bail!("unsupported identity version");
    }
    PeerIdentity::from_secret_keys_b64(&file.peer_secret_b64, &file.device_secret_b64)
}

fn load_or_create_certificate(path: &Path) -> Result<QuicCertificate> {
    if path.exists() {
        return read_json(path);
    }
    let certificate = QuicCertificate::generate()?;
    write_json(path, &certificate)?;
    Ok(certificate)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as i64
}
