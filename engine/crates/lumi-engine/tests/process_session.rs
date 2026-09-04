use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use lumi_engine::StartupReady;
use lumi_protocol::{MessageDecoder, MessageEnvelope, MessageType, PROTOCOL_VERSION};
use serde_json::{Value, json};

const TEST_SESSION_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn real_engine_process_serves_state_and_fails_safe_between_ui_clients() {
    let mut child = match Command::new(env!("CARGO_BIN_EXE_lumi-engine"))
        .env("LUMI_SESSION_TOKEN", TEST_SESSION_TOKEN)
        .env(
            "LUMI_DECK_INPUT_DESTINATION_NAME",
            format!("Lumi Deck Input Process Test {}", std::process::id()),
        )
        // This test exercises the authenticated process protocol. Keeping it
        // independent from the host CoreMIDI daemon makes the process check
        // deterministic on build machines and developer Macs alike.
        .env("LUMI_DECK_INPUT_DISABLED", "1")
        .env("LUMI_AUTO_PUBLISH_MIDI", "0")
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
            let mut stderr_text = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut stderr_text);
            }
            let exit_status = child.wait().ok();
            panic!(
                "invalid startup record: {error}; exit status: {exit_status:?}; stderr: {stderr_text}"
            );
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
    assert_eq!(snapshot.payload.get("stateRevision"), Some(&Value::from(2)));
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
        Some(&Value::from(2))
    );
    assert_eq!(
        snapshot
            .payload
            .get("deckSource")
            .and_then(Value::as_object)
            .and_then(|source| source.get("providerKind")),
        Some(&Value::String("localPlayback".to_owned()))
    );
    assert_eq!(
        snapshot
            .payload
            .get("deckSource")
            .and_then(Value::as_object)
            .and_then(|source| source.get("mode")),
        Some(&Value::String("localPlayback".to_owned()))
    );
    assert_eq!(snapshot.payload.get("simulation"), None);
    assert_eq!(snapshot.payload.get("leaderDeckId"), Some(&Value::Null));
    let Some(decks) = snapshot.payload.get("decks").and_then(Value::as_array) else {
        let _ = child.kill();
        panic!("snapshot must contain decks");
    };
    assert!(decks.is_empty());
    assert_eq!(snapshot.payload.get("nextPlan"), Some(&Value::Null));

    let arm_command = command(
        "arm-1",
        1,
        json!({
            "kind": "setOperationState",
            "operationState": "armed",
            "expectedStateRevision": 2,
        }),
    );
    let armed = exchange(&mut connection, &arm_command);
    assert_eq!(
        armed.payload.get("operationState"),
        Some(&Value::String("armed".to_owned()))
    );
    assert_eq!(armed.payload.get("stateRevision"), Some(&Value::from(3)));

    let duplicate = exchange(&mut connection, &arm_command);
    assert_eq!(
        duplicate.payload.get("stateRevision"),
        armed.payload.get("stateRevision")
    );

    let stale_state = exchange(
        &mut connection,
        &command(
            "stale-operation",
            2,
            json!({
                "kind": "setOperationState",
                "operationState": "armed",
                "expectedStateRevision": 0,
            }),
        ),
    );
    assert_eq!(stale_state.message_type, MessageType::Error);
    assert_eq!(
        stale_state.payload.get("code"),
        Some(&Value::String("stateRevisionMismatch".to_owned()))
    );
    assert_eq!(
        stale_state.payload.get("actualStateRevision"),
        armed.payload.get("stateRevision")
    );

    if let Err(error) = writeln!(connection, "{{\"protocolVersion\":") {
        let _ = child.kill();
        panic!("malformed command must be writable: {error}");
    }
    let mut malformed_line = String::new();
    if let Err(error) = BufReader::new(&connection).read_line(&mut malformed_line) {
        let _ = child.kill();
        panic!("malformed command response must be readable: {error}");
    }
    let malformed = match MessageDecoder::decode(malformed_line.as_bytes()) {
        Ok(response) => response,
        Err(error) => {
            let _ = child.kill();
            panic!("malformed command response must be a valid envelope: {error}");
        }
    };
    assert_eq!(malformed.message_type, MessageType::Error);
    assert_eq!(
        malformed.payload.get("code"),
        Some(&Value::String("invalidEnvelope".to_owned()))
    );

    let after_faults = exchange(
        &mut connection,
        &command("snapshot-after-faults", 3, json!({ "kind": "getSnapshot" })),
    );
    assert_eq!(
        after_faults.payload.get("operationState"),
        Some(&Value::String("armed".to_owned()))
    );
    assert_eq!(
        after_faults
            .payload
            .get("outputProvider")
            .and_then(Value::as_object)
            .and_then(|output| output.get("recordCount")),
        Some(&Value::from(0))
    );
    // Exercise the unexpected-I/O path as well as the ordinary EOF path. A
    // client can disappear with a partial frame when macOS replaces a UI
    // process; the persistent engine must fail-safe and accept the next UI.
    if let Err(error) = write!(connection, "{{") {
        let _ = child.kill();
        panic!("partial command must be writable: {error}");
    }
    drop(connection);

    thread::sleep(Duration::from_millis(40));
    assert!(
        child.try_wait().is_ok_and(|status| status.is_none()),
        "the engine service must outlive a UI client"
    );
    let mut reconnected = TcpStream::connect((ready.host.as_str(), ready.port))
        .unwrap_or_else(|error| panic!("failed to reconnect to engine: {error}"));
    writeln!(reconnected, "{{\"sessionToken\":\"{TEST_SESSION_TOKEN}\"}}")
        .unwrap_or_else(|error| panic!("failed to reauthenticate: {error}"));
    let mut reconnect_snapshot = String::new();
    BufReader::new(&reconnected)
        .read_line(&mut reconnect_snapshot)
        .unwrap_or_else(|error| panic!("failed to read reconnect snapshot: {error}"));
    let reconnect_snapshot = MessageDecoder::decode(reconnect_snapshot.as_bytes())
        .unwrap_or_else(|error| panic!("invalid reconnect snapshot: {error}"));
    assert_eq!(
        reconnect_snapshot.payload.get("operationState"),
        Some(&Value::String("off".to_owned()))
    );
    assert_eq!(
        reconnect_snapshot.payload.get("stateRevision"),
        Some(&Value::from(4))
    );
    drop(reconnected);
    let _ = child.kill();
    let _ = child.wait();
}

fn command(message_id: &str, sequence: u64, payload: Value) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "messageType": "command",
        "messageId": message_id,
        "sequence": sequence,
        "correlationId": message_id,
        "sentAt": "2026-08-03T12:00:00Z",
        "payload": payload,
    })
}

fn exchange(connection: &mut TcpStream, command: &Value) -> MessageEnvelope {
    if let Err(error) = writeln!(connection, "{command}") {
        panic!("command must be writable: {error}");
    }
    let mut line = String::new();
    if let Err(error) = BufReader::new(&*connection).read_line(&mut line) {
        panic!("response must be readable: {error}");
    }
    match MessageDecoder::decode(line.as_bytes()) {
        Ok(response) => response,
        Err(error) => panic!("response must be a valid envelope: {error}"),
    }
}
