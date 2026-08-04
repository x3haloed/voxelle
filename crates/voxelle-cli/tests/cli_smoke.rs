use assert_cmd::Command;
use predicates::prelude::*;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use tempfile::tempdir;
use voxelle_core::PeerIdentity;
use voxelle_net::{PeerEndpoint, QuicCertificate};

fn voxelle() -> Command {
    let mut command = Command::cargo_bin("voxelle").expect("voxelle binary");
    command.env("VOXELLE_VAULT_BACKEND", "test-file");
    command
}

#[test]
fn cli_home_workflow_drives_app_actions() {
    let dir = tempdir().expect("tempdir");
    let home = dir.path().join("alice");
    let recovery_kit = dir.path().join("alice.voxrecover");
    let recovered_home = dir.path().join("alice-recovered");

    voxelle()
        .args(["init", "--home"])
        .arg(&home)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"default_room\": \"room:general\"",
        ))
        .stdout(predicate::str::contains("\"authority_peer_id\": \"p:"));

    voxelle()
        .args(["send", "--home"])
        .arg(&home)
        .args(["--text", "hello home"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("e:"));

    voxelle()
        .args(["read", "--home"])
        .arg(&home)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"text\": \"hello home\""));

    voxelle()
        .args(["recovery", "export", "--home"])
        .arg(&home)
        .args(["--out"])
        .arg(&recovery_kit)
        .assert()
        .success();

    voxelle()
        .args(["recovery", "restore", "--home"])
        .arg(&recovered_home)
        .args(["--kit"])
        .arg(&recovery_kit)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"peers_reached\": 0"))
        .stdout(predicate::str::contains("\"peer_id\": \"p:"));
}

#[test]
fn cli_creates_identity_room_message_and_syncs_local_store() {
    let dir = tempdir().expect("tempdir");
    let identity = dir.path().join("alice.identity.json");
    let store_a = dir.path().join("a.sqlite3");
    let store_b = dir.path().join("b.sqlite3");

    let output = voxelle()
        .args(["identity", "create", "--out"])
        .arg(&identity)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("p:"))
        .get_output()
        .stdout
        .clone();
    let authority = String::from_utf8(output).unwrap().trim().to_string();

    voxelle()
        .args(["room", "create", "--identity"])
        .arg(&identity)
        .args(["--store"])
        .arg(&store_a)
        .assert()
        .success()
        .stdout(predicate::str::contains("room=room:general"));

    voxelle()
        .args(["event", "send", "--identity"])
        .arg(&identity)
        .args(["--store"])
        .arg(&store_a)
        .args(["--text", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("e:"));

    voxelle()
        .args(["sync", "local", "--from"])
        .arg(&store_a)
        .args(["--to"])
        .arg(&store_b)
        .args(["--authority-peer-id", &authority])
        .assert()
        .success()
        .stdout(predicate::str::contains("accepted=2"));

    voxelle()
        .args(["room", "count", "--store"])
        .arg(&store_b)
        .assert()
        .success()
        .stdout("1\n");

    voxelle()
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

    voxelle()
        .args(["identity", "create", "--out"])
        .arg(&identity)
        .assert()
        .success();

    let remote_identity = PeerIdentity::generate().expect("remote identity");
    let remote_cert = QuicCertificate::generate().expect("remote cert");
    let endpoint = PeerEndpoint {
        v: 1,
        addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9),
        peer_id: remote_identity.peer_id,
        device_id: remote_identity.device.id,
        quic_cert_der_b64: remote_cert.cert_der_b64,
        quic_cert_fingerprint: remote_cert.fingerprint,
    };
    std::fs::write(
        &endpoint_path,
        serde_json::to_string_pretty(&endpoint).unwrap() + "\n",
    )
    .expect("write endpoint");

    voxelle()
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
