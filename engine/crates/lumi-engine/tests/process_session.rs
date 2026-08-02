use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lumi_engine::StartupReady;
use lumi_protocol::{MessageDecoder, MessageType, PROTOCOL_VERSION};
use serde_json::Value;

const TEST_SESSION_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn real_engine_process_serves_authenticated_snapshot_on_loopback() {
    let mut child = match Command::new(env!("CARGO_BIN_EXE_lumi-engine"))
        .env("LUMI_SESSION_TOKEN", TEST_SESSION_TOKEN)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => panic!("failed to start engine: {error}"),
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        panic!("engine stdout was not captured");
    };
    let mut stdout = BufReader::new(stdout);
    let mut ready_line = String::new();
    if let Err(error) = stdout.read_line(&mut ready_line) {
        let _ = child.kill();
        panic!("failed to read engine startup record: {error}");
    }
    let ready: StartupReady = match serde_json::from_str(&ready_line) {
        Ok(ready) => ready,
        Err(error) => {
            let _ = child.kill();
            panic!("invalid startup record: {error}");
        }
    };

    assert_eq!(ready.host, "127.0.0.1");
    assert_eq!(ready.protocol_version, PROTOCOL_VERSION);

    let mut connection = match TcpStream::connect((ready.host.as_str(), ready.port)) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = child.kill();
            panic!("failed to connect to engine: {error}");
        }
    };
    if let Err(error) = connection.set_read_timeout(Some(Duration::from_secs(2))) {
        let _ = child.kill();
        panic!("failed to set read timeout: {error}");
    }
    if let Err(error) = writeln!(connection, "{{\"sessionToken\":\"{TEST_SESSION_TOKEN}\"}}") {
        let _ = child.kill();
        panic!("failed to authenticate: {error}");
    }

    let mut snapshot_line = String::new();
    if let Err(error) = BufReader::new(&connection).read_line(&mut snapshot_line) {
        let _ = child.kill();
        panic!("failed to read snapshot: {error}");
    }
    let snapshot = match MessageDecoder::decode(snapshot_line.as_bytes()) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let _ = child.kill();
            panic!("invalid snapshot: {error}");
        }
    };

    assert_eq!(snapshot.message_type, MessageType::Snapshot);
    assert_eq!(snapshot.sequence, 1);
    assert_eq!(snapshot.payload.get("stateRevision"), Some(&Value::from(7)));
    assert_eq!(
        snapshot
            .payload
            .get("runtimeCore")
            .and_then(Value::as_object)
            .and_then(|runtime| runtime.get("model")),
        Some(&Value::String("singleWriterReducer".to_owned()))
    );
    assert_eq!(
        snapshot
            .payload
            .get("runtimeCore")
            .and_then(Value::as_object)
            .and_then(|runtime| runtime.get("health")),
        Some(&Value::String("ready".to_owned()))
    );
    assert_eq!(
        snapshot
            .payload
            .get("runtimeCore")
            .and_then(Value::as_object)
            .and_then(|runtime| runtime.get("processedEvents")),
        Some(&Value::from(7))
    );
    assert_eq!(
        snapshot
            .payload
            .get("deckSource")
            .and_then(Value::as_object)
            .and_then(|source| source.get("providerKind")),
        Some(&Value::String("simulator".to_owned()))
    );
    assert_eq!(snapshot.payload.get("leaderDeckId"), Some(&Value::from(1)));
    let Some(decks) = snapshot.payload.get("decks").and_then(Value::as_array) else {
        let _ = child.kill();
        panic!("snapshot must contain decks");
    };
    assert_eq!(decks.len(), 2);
    assert_eq!(
        decks[0].get("track").and_then(|track| track.get("title")),
        Some(&Value::String("Aurora Signal".to_owned()))
    );
    assert_eq!(
        decks[1].get("track").and_then(|track| track.get("title")),
        Some(&Value::String("Neon Horizon".to_owned()))
    );
    drop(connection);

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(status.success());
                break;
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                panic!("engine did not exit after its client disconnected");
            }
            Err(error) => {
                let _ = child.kill();
                panic!("failed to inspect engine process: {error}");
            }
        }
    }
}
