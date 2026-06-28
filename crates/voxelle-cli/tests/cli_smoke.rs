use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

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
