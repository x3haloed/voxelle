use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use voxelle_core::{
    accept_event, create_delegation, create_event, EventV1, PeerIdentity, RoomContext,
    GOVERNANCE_ROOM_ID,
};
use voxelle_net::{PeerEndpoint, QuicCertificate};
use voxelle_store::Store;

pub const DEFAULT_ROOM_ID: &str = "room:general";

#[derive(Debug, Clone)]
pub struct VoxelleHome {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeConfig {
    pub v: u8,
    pub default_room: String,
    pub authority_peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityFile {
    pub v: u8,
    pub peer_secret_b64: String,
    pub device_secret_b64: String,
    pub peer_id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummary {
    pub home: PathBuf,
    pub peer_id: String,
    pub device_id: String,
    pub default_room: String,
    pub authority_peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageView {
    pub event_id: String,
    pub created_ms: i64,
    pub author_peer_id: String,
    pub text: String,
}

impl VoxelleHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn identity_path(&self) -> PathBuf {
        self.root.join("identity.json")
    }

    pub fn certificate_path(&self) -> PathBuf {
        self.root.join("quic-cert.json")
    }

    pub fn store_path(&self) -> PathBuf {
        self.root.join("store.sqlite3")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn init(&self, default_room: impl Into<String>) -> Result<ProfileSummary> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create {}", self.root.display()))?;
        let identity = self.load_or_create_identity()?;
        self.load_or_create_certificate()?;

        let default_room = default_room.into();
        let config = if self.config_path().exists() {
            self.load_config()?
        } else {
            let config = HomeConfig {
                v: 1,
                default_room,
                authority_peer_id: identity.peer.id.clone(),
            };
            write_json(&self.config_path(), &config)?;
            config
        };

        let store = self.open_store()?;
        self.ensure_member_join(&store, &identity, &config)?;

        Ok(ProfileSummary {
            home: self.root.clone(),
            peer_id: identity.peer.id,
            device_id: identity.device.id,
            default_room: config.default_room,
            authority_peer_id: config.authority_peer_id,
        })
    }

    pub fn send_message(&self, text: &str, room: Option<&str>) -> Result<EventV1> {
        let identity = self.load_identity()?;
        let config = self.load_config()?;
        let store = self.open_store()?;
        let room = room.unwrap_or(&config.default_room);
        let context = RoomContext::new(config.authority_peer_id);
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
            .map_err(|e| anyhow::anyhow!("message rejected: {e:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(event)
    }

    pub fn read_messages(&self, room: Option<&str>) -> Result<Vec<MessageView>> {
        let config = self.load_config()?;
        let store = self.open_store()?;
        let room = room.unwrap_or(&config.default_room);
        let mut messages = Vec::new();
        for event in store.room_events(room)? {
            if event.kind != "MSG_POST" {
                continue;
            }
            let text = event
                .body
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            messages.push(MessageView {
                event_id: event.event_id,
                created_ms: event.created_ms,
                author_peer_id: event.author_peer_id,
                text,
            });
        }
        Ok(messages)
    }

    pub fn export_endpoint(&self, advertised_addr: SocketAddr) -> Result<PeerEndpoint> {
        if !advertised_addr.is_ipv6() {
            anyhow::bail!("advertised address must be IPv6");
        }
        let identity = self.load_identity()?;
        let certificate = self.load_certificate()?;
        Ok(PeerEndpoint {
            v: 1,
            addr: advertised_addr,
            peer_id: identity.peer.id,
            device_id: identity.device.id,
            quic_cert_der_b64: certificate.cert_der_b64,
            quic_cert_fingerprint: certificate.fingerprint,
        })
    }

    pub fn load_identity(&self) -> Result<PeerIdentity> {
        let file: IdentityFile = read_json(&self.identity_path())?;
        if file.v != 1 {
            anyhow::bail!("unsupported identity version {}", file.v);
        }
        PeerIdentity::from_secret_keys_b64(&file.peer_secret_b64, &file.device_secret_b64)
    }

    pub fn load_certificate(&self) -> Result<QuicCertificate> {
        read_json(&self.certificate_path())
    }

    pub fn load_config(&self) -> Result<HomeConfig> {
        let config: HomeConfig = read_json(&self.config_path())?;
        if config.v != 1 {
            anyhow::bail!("unsupported home config version {}", config.v);
        }
        Ok(config)
    }

    pub fn open_store(&self) -> Result<Store> {
        Store::open(self.store_path())
    }

    fn load_or_create_identity(&self) -> Result<PeerIdentity> {
        if self.identity_path().exists() {
            return self.load_identity();
        }
        let identity = PeerIdentity::generate()?;
        let file = IdentityFile {
            v: 1,
            peer_secret_b64: identity.peer.secret_key_b64(),
            device_secret_b64: identity.device.secret_key_b64(),
            peer_id: identity.peer.id.clone(),
            device_id: identity.device.id.clone(),
        };
        write_json(&self.identity_path(), &file)?;
        Ok(identity)
    }

    fn load_or_create_certificate(&self) -> Result<QuicCertificate> {
        if self.certificate_path().exists() {
            return self.load_certificate();
        }
        let certificate = QuicCertificate::generate()?;
        write_json(&self.certificate_path(), &certificate)?;
        Ok(certificate)
    }

    fn ensure_member_join(
        &self,
        store: &Store,
        identity: &PeerIdentity,
        config: &HomeConfig,
    ) -> Result<()> {
        let existing_join = store
            .room_events(GOVERNANCE_ROOM_ID)?
            .into_iter()
            .any(|event| {
                event.kind == "MEMBER_JOIN"
                    && event.author_peer_id == identity.peer.id
                    && event.body.get("peer_id").and_then(|value| value.as_str())
                        == Some(identity.peer.id.as_str())
            });
        if existing_join {
            return Ok(());
        }

        let context = RoomContext::new(config.authority_peer_id.clone());
        let join = create_event(
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
        )?;
        let accepted = accept_event(&join, &[], &context, now_ms())
            .map_err(|e| anyhow::anyhow!("member join rejected: {e:?}"))?;
        store.insert_accepted_event(accepted, now_ms())?;
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv6Addr};
    use tempfile::tempdir;

    #[test]
    fn home_init_send_read_and_endpoint_export_are_app_actions() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));

        let profile = home.init(DEFAULT_ROOM_ID).expect("init");
        assert!(home.identity_path().exists());
        assert!(home.certificate_path().exists());
        assert!(home.store_path().exists());
        assert_eq!(profile.default_room, DEFAULT_ROOM_ID);
        assert_eq!(profile.peer_id, profile.authority_peer_id);

        let event = home
            .send_message("hello from app layer", None)
            .expect("send");
        assert_eq!(event.kind, "MSG_POST");

        let messages = home.read_messages(None).expect("read");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "hello from app layer");

        let endpoint = home
            .export_endpoint(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 4040))
            .expect("endpoint");
        endpoint.validate().expect("valid endpoint");
        assert_eq!(endpoint.peer_id, profile.peer_id);
        assert_eq!(endpoint.device_id, profile.device_id);
    }

    #[test]
    fn home_init_is_idempotent_and_preserves_identity() {
        let dir = tempdir().expect("tempdir");
        let home = VoxelleHome::new(dir.path().join("alice"));

        let first = home.init(DEFAULT_ROOM_ID).expect("first init");
        let second = home.init("room:ignored").expect("second init");

        assert_eq!(first.peer_id, second.peer_id);
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(second.default_room, DEFAULT_ROOM_ID);
        assert_eq!(
            home.open_store()
                .expect("store")
                .room_event_count(GOVERNANCE_ROOM_ID)
                .expect("count"),
            1
        );
    }
}
