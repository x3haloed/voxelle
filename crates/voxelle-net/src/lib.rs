use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use quinn_proto::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::sync::Once;
use voxelle_core::{id_from_spki_der, verify_signature_from_spki_b64, PeerIdentity};

pub const VOXELLE_ALPN: &[u8] = b"voxelle-ipv6/0";
const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;
static RUSTLS_PROVIDER: Once = Once::new();

#[derive(Debug)]
pub struct QuicNode {
    endpoint: quinn::Endpoint,
    cert_der: CertificateDer<'static>,
    identity: PeerIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPeer {
    pub peer_id: String,
    pub device_id: String,
    pub device_pub: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedConnection {
    pub connection: quinn::Connection,
    pub remote: AuthenticatedPeer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HandshakeV1 {
    v: u8,
    role: HandshakeRole,
    peer_id: String,
    device_id: String,
    device_pub: String,
    sig: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum HandshakeRole {
    Client,
    Server,
}

impl QuicNode {
    pub fn bind_ipv6_loopback(identity: PeerIdentity) -> Result<Self> {
        let (server_config, cert_der) = server_config()?;
        let endpoint = quinn::Endpoint::server(
            server_config,
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        )
        .context("bind IPv6 QUIC endpoint")?;

        Ok(Self {
            endpoint,
            cert_der,
            identity,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }

    pub fn certificate_der(&self) -> CertificateDer<'static> {
        self.cert_der.clone()
    }

    pub async fn accept_one(&self) -> Result<AuthenticatedConnection> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| anyhow!("QUIC endpoint closed"))?;
        let connection = incoming.await.context("accept QUIC connection")?;
        let (mut send, recv) = connection
            .accept_bi()
            .await
            .context("accept handshake stream")?;

        let client_hello: HandshakeV1 = recv_handshake(recv).await?;
        let remote = validate_handshake(&client_hello, HandshakeRole::Client)?;

        let server_hello = make_handshake(&self.identity, HandshakeRole::Server)?;
        let bytes = serde_json::to_vec(&server_hello).context("serialize server handshake")?;
        send.write_all(&bytes)
            .await
            .context("write server handshake")?;
        send.finish().context("finish server handshake stream")?;

        Ok(AuthenticatedConnection { connection, remote })
    }

    pub async fn connect(
        &self,
        remote_addr: SocketAddr,
        remote_cert_der: CertificateDer<'static>,
        expected_remote_device_id: &str,
    ) -> Result<AuthenticatedConnection> {
        if !remote_addr.is_ipv6() {
            bail!("remote address must be IPv6");
        }

        let mut endpoint =
            quinn::Endpoint::client(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0))
                .context("bind IPv6 QUIC client endpoint")?;
        endpoint.set_default_client_config(client_config(remote_cert_der)?);

        let connection = endpoint
            .connect(remote_addr, "localhost")
            .context("start QUIC connect")?
            .await
            .context("complete QUIC connect")?;

        let (mut send, recv) = connection
            .open_bi()
            .await
            .context("open handshake stream")?;
        let client_hello = make_handshake(&self.identity, HandshakeRole::Client)?;
        let bytes = serde_json::to_vec(&client_hello).context("serialize client handshake")?;
        send.write_all(&bytes)
            .await
            .context("write client handshake")?;
        send.finish().context("finish client handshake stream")?;

        let server_hello: HandshakeV1 = recv_handshake(recv).await?;
        let remote = validate_handshake(&server_hello, HandshakeRole::Server)?;
        if remote.device_id != expected_remote_device_id {
            bail!(
                "remote device id mismatch: expected {}, got {}",
                expected_remote_device_id,
                remote.device_id
            );
        }

        Ok(AuthenticatedConnection { connection, remote })
    }

    pub async fn wait_idle(&self) {
        self.endpoint.wait_idle().await;
    }
}

fn server_config() -> Result<(quinn::ServerConfig, CertificateDer<'static>)> {
    ensure_rustls_provider();

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = CertificateDer::from(cert.cert);
    let private_key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], private_key.into())
        .context("build rustls server config")?;
    server_crypto.alpn_protocols = vec![VOXELLE_ALPN.to_vec()];

    let mut server_config =
        quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_crypto)?));
    let transport = Arc::get_mut(&mut server_config.transport)
        .ok_or_else(|| anyhow!("server transport config is shared"))?;
    transport.max_concurrent_uni_streams(0_u8.into());

    Ok((server_config, cert_der))
}

fn client_config(server_cert_der: CertificateDer<'static>) -> Result<quinn::ClientConfig> {
    ensure_rustls_provider();

    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(server_cert_der)
        .context("trust server certificate")?;

    let mut client_crypto = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_crypto.alpn_protocols = vec![VOXELLE_ALPN.to_vec()];

    Ok(quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(client_crypto)?,
    )))
}

fn ensure_rustls_provider() {
    RUSTLS_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn make_handshake(identity: &PeerIdentity, role: HandshakeRole) -> Result<HandshakeV1> {
    let mut hello = HandshakeV1 {
        v: 1,
        role,
        peer_id: identity.peer.id.clone(),
        device_id: identity.device.id.clone(),
        device_pub: identity.device.spki_b64.clone(),
        sig: String::new(),
    };
    hello.sig = identity.device.sign(&handshake_signing_bytes(&hello)?);
    Ok(hello)
}

fn validate_handshake(
    hello: &HandshakeV1,
    expected_role: HandshakeRole,
) -> Result<AuthenticatedPeer> {
    if hello.v != 1 {
        bail!("unsupported handshake version {}", hello.v);
    }
    if hello.role != expected_role {
        bail!("unexpected handshake role");
    }

    let device_spki = base64::engine::general_purpose::STANDARD
        .decode(&hello.device_pub)
        .context("decode handshake device public key")?;
    let device_id = id_from_spki_der(&device_spki).context("derive handshake device id")?;
    if device_id != hello.device_id {
        bail!("handshake device id does not match device public key");
    }

    verify_signature_from_spki_b64(
        &hello.device_pub,
        &handshake_signing_bytes(hello)?,
        &hello.sig,
    )
    .context("verify handshake signature")?;

    Ok(AuthenticatedPeer {
        peer_id: hello.peer_id.clone(),
        device_id: hello.device_id.clone(),
        device_pub: hello.device_pub.clone(),
    })
}

fn handshake_signing_bytes(hello: &HandshakeV1) -> Result<Vec<u8>> {
    let mut unsigned = hello.clone();
    unsigned.sig.clear();
    serde_json::to_vec(&unsigned).context("serialize handshake signing bytes")
}

async fn recv_handshake<T: for<'de> Deserialize<'de>>(mut recv: quinn::RecvStream) -> Result<T> {
    let bytes = recv
        .read_to_end(MAX_HANDSHAKE_BYTES)
        .await
        .context("read handshake")?;
    serde_json::from_slice(&bytes).context("parse handshake")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ipv6_loopback_handshake_authenticates_remote_device() -> Result<()> {
        let server_identity = PeerIdentity::generate()?;
        let client_identity = PeerIdentity::generate()?;
        let expected_server_device_id = server_identity.device.id.clone();
        let expected_client_device_id = client_identity.device.id.clone();

        let server = QuicNode::bind_ipv6_loopback(server_identity)?;
        let client = QuicNode::bind_ipv6_loopback(client_identity)?;
        let server_addr = server.local_addr()?;
        assert!(server_addr.is_ipv6());
        let server_cert = server.certificate_der();

        let accept = tokio::spawn(async move { server.accept_one().await });
        let connected = client
            .connect(server_addr, server_cert, &expected_server_device_id)
            .await?;
        assert_eq!(connected.remote.device_id, expected_server_device_id);
        connected.connection.close(0u32.into(), b"done");

        let accepted = accept.await??;
        assert_eq!(accepted.remote.device_id, expected_client_device_id);
        accepted.connection.close(0u32.into(), b"done");
        client.wait_idle().await;

        Ok(())
    }

    #[tokio::test]
    async fn ipv6_loopback_handshake_rejects_unexpected_remote_device() -> Result<()> {
        let server = QuicNode::bind_ipv6_loopback(PeerIdentity::generate()?)?;
        let client = QuicNode::bind_ipv6_loopback(PeerIdentity::generate()?)?;
        let wrong_identity = PeerIdentity::generate()?;
        let server_addr = server.local_addr()?;
        let server_cert = server.certificate_der();

        let accept = tokio::spawn(async move { server.accept_one().await });
        let err = client
            .connect(server_addr, server_cert, &wrong_identity.device.id)
            .await
            .expect_err("client should reject unexpected server device id");

        assert!(err.to_string().contains("remote device id mismatch"));
        let _ = accept.await??;
        client.wait_idle().await;

        Ok(())
    }
}
