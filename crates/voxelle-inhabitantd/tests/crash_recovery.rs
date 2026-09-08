//! Exercise process death, rather than a graceful in-process store reopen.
use base64::Engine as _;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Daemon {
    child: Child,
    address: String,
    authorization: String,
}

impl Daemon {
    fn start(home: &Path, discovery: &Path) -> Self {
        // Never accept a discovery record from the previous process.
        if discovery.exists() {
            fs::remove_file(discovery).unwrap();
        }
        let child = Command::new(env!("CARGO_BIN_EXE_voxelle-inhabitantd"))
            .arg("--home")
            .arg(home)
            .arg("--discovery-file")
            .arg(discovery)
            .env("VOXELLE_VAULT_BACKEND", "test-file")
            .stdout(Stdio::null())
            .spawn()
            .expect("start isolated daemon");
        let mut daemon = Self {
            child,
            address: String::new(),
            authorization: String::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(
                daemon.child.try_wait().unwrap().is_none(),
                "daemon exited during startup"
            );
            if let Ok(bytes) = fs::read(discovery) {
                if let Ok(record) = serde_json::from_slice::<Value>(&bytes) {
                    assert_eq!(record["pid"], daemon.child.id());
                    daemon.address = record["base_url"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("http://")
                        .unwrap()
                        .to_owned();
                    daemon.authorization = record["authorization"].as_str().unwrap().to_owned();
                    return daemon;
                }
            }
            assert!(
                Instant::now() < deadline,
                "discovery did not become available"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn send(
        &self,
        command: Option<&str>,
        payload: &Value,
        origin: Option<(&str, &str)>,
    ) -> TcpStream {
        let mut stream = TcpStream::connect(&self.address).expect("connect daemon");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let (method, path, body) = match command {
            Some(command) => (
                "POST",
                format!("/inhabitant/v0/commands/{command}"),
                payload.to_string(),
            ),
            None => ("GET", "/inhabitant/v0/snapshot".to_owned(), String::new()),
        };
        let origin_headers = origin
            .map(|(id, secret)| {
                format!("Voxelle-Origin-Id: {id}\r\nVoxelle-Origin-Secret: {secret}\r\n")
            })
            .unwrap_or_default();
        write!(stream,
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: {}\r\n{origin_headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.address, self.authorization, body.len()
        ).unwrap();
        stream
    }

    fn request(
        &self,
        command: Option<&str>,
        payload: &Value,
        origin: Option<(&str, &str)>,
    ) -> Value {
        let mut stream = self.send(command, payload, origin);
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read complete HTTP response");
        let (headers, body) = response.split_once("\r\n\r\n").expect("HTTP response");
        assert!(
            headers.starts_with("HTTP/1.1 200 "),
            "request failed: {response}"
        );
        let body: Value = serde_json::from_str(body).expect("JSON response");
        if command.is_some() {
            assert_eq!(body["ok"], true, "command failed: {body}");
        }
        body
    }

    fn crash(&mut self) {
        // Child::kill is SIGKILL on Unix and TerminateProcess on Windows.
        self.child.kill().expect("abruptly terminate daemon");
        assert!(!self.child.wait().unwrap().success());
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn message(snapshot: &Value, token: &str) -> Value {
    let messages = snapshot["home"]["room"]["messages"]
        .as_array()
        .expect("room messages");
    let matching: Vec<_> = messages
        .iter()
        .filter(|message| message["client_request_id"] == token)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "request token must identify exactly one retained message"
    );
    matching[0].clone()
}

#[test]
fn abrupt_restart_retains_admitted_message_and_reconciles_unread_send_response() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("home");
    let discovery = directory.path().join("discovery.json");
    let mut daemon = Daemon::start(&home, &discovery);
    daemon.request(Some("home.init"), &json!({"default_room": null}), None);
    let secret = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0x37; 32]);
    let origin_request = json!({
        "client_instance_id": "crash-recovery-client",
        "secret": secret,
        "label": "Crash recovery fixture"
    });
    let opened = daemon.request(Some("resident.origin.open"), &origin_request, None);
    let origin = opened["snapshot"]["origin_id"].as_str().unwrap().to_owned();
    let token = "crash-unread-response-001";
    let payload = json!({"text": "Survive abrupt process death exactly once", "room": null, "client_request_id": token});

    // Submit normally but never consume this request's response. A separate
    // observer establishes admission; a lost response does not imply failure.
    let unread_response = daemon.send(Some("message.send"), &payload, Some((&origin, &secret)));
    let deadline = Instant::now() + Duration::from_secs(20);
    let admitted = loop {
        let snapshot = daemon.request(None, &Value::Null, None);
        if snapshot["home"]["room"]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["client_request_id"] == token)
        {
            break message(&snapshot, token);
        }
        assert!(Instant::now() < deadline, "send never became observable");
        thread::sleep(Duration::from_millis(20));
    };
    daemon.crash();
    drop(unread_response);
    let previous_authorization = daemon.authorization.clone();
    drop(daemon);

    let mut reopened = Daemon::start(&home, &discovery);
    assert_ne!(
        reopened.authorization, previous_authorization,
        "process bearer must rotate"
    );
    let snapshot = reopened.request(None, &Value::Null, None);
    assert_eq!(
        message(&snapshot, token),
        admitted,
        "accepted fact and certified origin must survive process death"
    );
    let resumed = reopened.request(Some("resident.origin.open"), &origin_request, None);
    assert_eq!(
        resumed["snapshot"], opened["snapshot"],
        "same caller recovers its durable origin"
    );
    for _ in 0..2 {
        let retry = reopened.request(Some("message.send"), &payload, Some((&origin, &secret)));
        assert_eq!(
            message(&retry["snapshot"], token),
            admitted,
            "retry must reuse the original event"
        );
    }
    reopened.crash();
    drop(reopened);
    let final_process = Daemon::start(&home, &discovery);
    let final_snapshot = final_process.request(None, &Value::Null, None);
    assert_eq!(
        message(&final_snapshot, token),
        admitted,
        "retry must not append duplicate facts"
    );
}
