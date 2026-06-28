use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use quinn_proto::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    certificate: QuicCertificate,
    identity: PeerIdentity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuicCertificate {
    pub cert_der_b64: String,
    pub private_key_pkcs8_der_b64: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPeer {
    pub peer_id: String,
    pub device_id: String,
    pub device_pub: String,
    pub quic_cert_fingerprint: String,
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
    quic_cert_fingerprint: String,
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
        Self::bind_ipv6_loopback_with_certificate(identity, QuicCertificate::generate()?)
    }

    pub fn bind_ipv6_loopback_with_certificate(
        identity: PeerIdentity,
        certificate: QuicCertificate,
    ) -> Result<Self> {
        let server_config = server_config(&certificate)?;
        let endpoint = quinn::Endpoint::server(
            server_config,
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        )
        .context("bind IPv6 QUIC endpoint")?;

        Ok(Self {
            endpoint,
            certificate,
            identity,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }

    pub fn certificate_der(&self) -> CertificateDer<'static> {
        self.certificate.certificate_der().expect("stored cert DER")
    }

    pub fn certificate(&self) -> &QuicCertificate {
        &self.certificate
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

        let server_hello = make_handshake(
            &self.identity,
            HandshakeRole::Server,
            &self.certificate.fingerprint,
        )?;
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

        let expected_remote_cert_fingerprint = cert_fingerprint(remote_cert_der.as_ref());
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
        let client_hello = make_handshake(
            &self.identity,
            HandshakeRole::Client,
            &self.certificate.fingerprint,
        )?;
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
        if remote.quic_cert_fingerprint != expected_remote_cert_fingerprint {
            bail!(
                "remote QUIC cert fingerprint mismatch: expected {}, got {}",
                expected_remote_cert_fingerprint,
                remote.quic_cert_fingerprint
            );
        }

        Ok(AuthenticatedConnection { connection, remote })
    }

    pub async fn wait_idle(&self) {
        self.endpoint.wait_idle().await;
    }
}

impl QuicCertificate {
    pub fn generate() -> Result<Self> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
        let cert_der: CertificateDer<'static> = cert.cert.into();
        Self::from_der(cert_der.as_ref().to_vec(), cert.key_pair.serialize_der())
    }

    pub fn from_der(cert_der: Vec<u8>, private_key_pkcs8_der: Vec<u8>) -> Result<Self> {
        if cert_der.is_empty() {
            bail!("QUIC certificate DER is empty");
        }
        if private_key_pkcs8_der.is_empty() {
            bail!("QUIC private key DER is empty");
        }

        Ok(Self {
            cert_der_b64: base64::engine::general_purpose::STANDARD.encode(&cert_der),
            private_key_pkcs8_der_b64: base64::engine::general_purpose::STANDARD
                .encode(&private_key_pkcs8_der),
            fingerprint: cert_fingerprint(&cert_der),
        })
    }

    pub fn certificate_der(&self) -> Result<CertificateDer<'static>> {
        Ok(CertificateDer::from(
            base64::engine::general_purpose::STANDARD
                .decode(&self.cert_der_b64)
                .context("decode QUIC certificate DER")?,
        ))
    }

    fn private_key_der(&self) -> Result<PrivatePkcs8KeyDer<'static>> {
        Ok(PrivatePkcs8KeyDer::from(
            base64::engine::general_purpose::STANDARD
                .decode(&self.private_key_pkcs8_der_b64)
                .context("decode QUIC private key DER")?,
        ))
    }
}

fn server_config(certificate: &QuicCertificate) -> Result<quinn::ServerConfig> {
    ensure_rustls_provider();

    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.certificate_der()?],
            certificate.private_key_der()?.into(),
        )
        .context("build rustls server config")?;
    server_crypto.alpn_protocols = vec![VOXELLE_ALPN.to_vec()];

    let mut server_config =
        quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_crypto)?));
    let transport = Arc::get_mut(&mut server_config.transport)
        .ok_or_else(|| anyhow!("server transport config is shared"))?;
    transport.max_concurrent_uni_streams(0_u8.into());

    Ok(server_config)
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

fn cert_fingerprint(cert_der: &[u8]) -> String {
    format!(
        "sha256:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(cert_der))
    )
}

fn make_handshake(
    identity: &PeerIdentity,
    role: HandshakeRole,
    quic_cert_fingerprint: &str,
) -> Result<HandshakeV1> {
    let mut hello = HandshakeV1 {
        v: 1,
        role,
        peer_id: identity.peer.id.clone(),
        device_id: identity.device.id.clone(),
        device_pub: identity.device.spki_b64.clone(),
        quic_cert_fingerprint: quic_cert_fingerprint.to_string(),
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
    if !hello.quic_cert_fingerprint.starts_with("sha256:") {
        bail!("unsupported QUIC cert fingerprint");
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
        quic_cert_fingerprint: hello.quic_cert_fingerprint.clone(),
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
        let server_fingerprint = server.certificate().fingerprint.clone();
        let client_fingerprint = client.certificate().fingerprint.clone();

        let accept = tokio::spawn(async move { server.accept_one().await });
        let connected = client
            .connect(server_addr, server_cert, &expected_server_device_id)
            .await?;
        assert_eq!(connected.remote.device_id, expected_server_device_id);
        assert_eq!(connected.remote.quic_cert_fingerprint, server_fingerprint);
        connected.connection.close(0u32.into(), b"done");

        let accepted = accept.await??;
        assert_eq!(accepted.remote.device_id, expected_client_device_id);
        assert_eq!(accepted.remote.quic_cert_fingerprint, client_fingerprint);
        accepted.connection.close(0u32.into(), b"done");
        client.wait_idle().await;

        Ok(())
    }

    #[tokio::test]
    async fn persistent_quic_certificate_can_be_reused() -> Result<()> {
        let certificate = QuicCertificate::generate()?;
        let restored = QuicCertificate::from_der(
            certificate.certificate_der()?.as_ref().to_vec(),
            base64::engine::general_purpose::STANDARD
                .decode(&certificate.private_key_pkcs8_der_b64)?,
        )?;
        assert_eq!(certificate.fingerprint, restored.fingerprint);

        let server_identity = PeerIdentity::generate()?;
        let expected_server_device_id = server_identity.device.id.clone();
        let client = QuicNode::bind_ipv6_loopback(PeerIdentity::generate()?)?;
        let server = QuicNode::bind_ipv6_loopback_with_certificate(server_identity, restored)?;
        let server_addr = server.local_addr()?;
        let server_cert = server.certificate_der();

        let accept = tokio::spawn(async move { server.accept_one().await });
        let connected = client
            .connect(server_addr, server_cert, &expected_server_device_id)
            .await?;
        assert_eq!(
            connected.remote.quic_cert_fingerprint,
            certificate.fingerprint
        );
        connected.connection.close(0u32.into(), b"done");

        let accepted = accept.await??;
        accepted.connection.close(0u32.into(), b"done");

        Ok(())
    }

    #[tokio::test]
    async fn client_rejects_signed_cert_fingerprint_mismatch() -> Result<()> {
        let server_identity = PeerIdentity::generate()?;
        let expected_server_device_id = server_identity.device.id.clone();
        let mut server_certificate = QuicCertificate::generate()?;
        let pinned_server_cert = server_certificate.certificate_der()?;
        server_certificate.fingerprint = "sha256:not-the-presented-cert".to_string();

        let server =
            QuicNode::bind_ipv6_loopback_with_certificate(server_identity, server_certificate)?;
        let client = QuicNode::bind_ipv6_loopback(PeerIdentity::generate()?)?;
        let server_addr = server.local_addr()?;

        let accept = tokio::spawn(async move { server.accept_one().await });
        let err = client
            .connect(server_addr, pinned_server_cert, &expected_server_device_id)
            .await
            .expect_err("client should reject signed fingerprint mismatch");
        assert!(err
            .to_string()
            .contains("remote QUIC cert fingerprint mismatch"));

        let accepted = accept.await??;
        accepted.connection.close(0u32.into(), b"done");

        Ok(())
    }

    #[test]
    fn signed_handshake_binds_quic_cert_fingerprint() -> Result<()> {
        let identity = PeerIdentity::generate()?;
        let hello = make_handshake(&identity, HandshakeRole::Server, "sha256:good")?;
        assert_eq!(
            validate_handshake(&hello, HandshakeRole::Server)?.quic_cert_fingerprint,
            "sha256:good"
        );

        let mut tampered = hello;
        tampered.quic_cert_fingerprint = "sha256:bad".to_string();
        let err = validate_handshake(&tampered, HandshakeRole::Server)
            .expect_err("tampered fingerprint should break the device signature");
        assert!(err.to_string().contains("verify handshake signature"));

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
