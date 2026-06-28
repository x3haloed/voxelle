use assert_cmd::Command;
use predicates::prelude::*;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use tempfile::tempdir;
use voxelle_core::PeerIdentity;
use voxelle_net::{PeerEndpoint, QuicCertificate};

#[test]
fn cli_creates_identity_room_message_and_syncs_local_store() {
    let dir = tempdir().expect("tempdir");
    let identity = dir.path().join("alice.identity.json");
    let store_a = dir.path().join("a.sqlite3");
    let store_b = dir.path().join("b.sqlite3");

    let output = Command::cargo_bin("voxelle")
        .unwrap()
        .args(["identity", "create", "--out"])
        .arg(&identity)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("ed25519:"))
        .get_output()
        .stdout
        .clone();
    let authority = String::from_utf8(output).unwrap().trim().to_string();

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["room", "create", "--identity"])
        .arg(&identity)
        .args(["--store"])
        .arg(&store_a)
        .assert()
        .success()
        .stdout(predicate::str::contains("room=room:general"));

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["event", "send", "--identity"])
        .arg(&identity)
        .args(["--store"])
        .arg(&store_a)
        .args(["--text", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("e:"));

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["sync", "local", "--from"])
        .arg(&store_a)
        .args(["--to"])
        .arg(&store_b)
        .args(["--authority-peer-id", &authority])
        .assert()
        .success()
        .stdout(predicate::str::contains("accepted=2"));

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["room", "count", "--store"])
        .arg(&store_b)
        .assert()
        .success()
        .stdout("1\n");

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["room", "heads", "--store"])
        .arg(&store_b)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("e:"));
}

#[test]
fn cli_diagnose_connect_reports_unreachable_peer_endpoint() {
    let dir = tempdir().expect("tempdir");
    let identity = dir.path().join("client.identity.json");
    let cert = dir.path().join("client.quic-cert.json");
    let endpoint_path = dir.path().join("endpoint.json");

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["identity", "create", "--out"])
        .arg(&identity)
        .assert()
        .success();

    let remote_identity = PeerIdentity::generate().expect("remote identity");
    let remote_cert = QuicCertificate::generate().expect("remote cert");
    let endpoint = PeerEndpoint {
        v: 1,
        addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9),
        peer_id: remote_identity.peer.id,
        device_id: remote_identity.device.id,
        quic_cert_der_b64: remote_cert.cert_der_b64,
        quic_cert_fingerprint: remote_cert.fingerprint,
    };
    std::fs::write(
        &endpoint_path,
        serde_json::to_string_pretty(&endpoint).unwrap() + "\n",
    )
    .expect("write endpoint");

    Command::cargo_bin("voxelle")
        .unwrap()
        .args(["diagnose", "connect", "--identity"])
        .arg(&identity)
        .args(["--cert"])
        .arg(&cert)
        .args(["--endpoint"])
        .arg(&endpoint_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"reachable\": false"))
        .stderr(predicate::str::contains("peer was not reachable"));
}
