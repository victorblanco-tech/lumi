use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lumi_engine::StartupReady;
use lumi_protocol::{MessageDecoder, MessageEnvelope, MessageType, PROTOCOL_VERSION};
use serde_json::{Value, json};

const TEST_SESSION_TOKEN: &str = "network-acceptance-0123456789abcdef0123456789abcdef";

/// Opt-in LAN acceptance check for the USB-backed Pro DJ Link simulator.
///
/// The test deliberately leaves the authenticated engine connection idle for
/// three seconds. Direct deck frames and timing anchors must still advance on
/// the engine's own cadence, proving that SwiftUI snapshot polling is not part
/// of the realtime path.
#[test]
#[ignore = "requires the Lumi Pro DJ Link network simulator"]
fn direct_timing_continues_while_the_client_is_idle() {
    if std::env::var("LUMI_RUN_PROLINK_NETWORK_TEST").as_deref() != Ok("1") {
        return;
    }

    let database = temporary_database_path();
    let mut child = start_engine(&database);
    let mut connection = connect_and_authenticate(&mut child);
    let initial = read_response(&mut connection);
    assert_eq!(initial.message_type, MessageType::Snapshot);

    let connected = exchange(
        &mut connection,
        &command(
            "select-connected-decks",
            1,
            json!({
                "kind": "selectDeckSourceMode",
                "mode": "connectedDecks",
                "expectedStateRevision": required_u64(&initial.payload, "stateRevision"),
            }),
        ),
    );
    assert_eq!(connected.message_type, MessageType::Snapshot);

    let enabled = exchange(
        &mut connection,
        &command(
            "enable-link",
            2,
            json!({ "kind": "setAbletonLinkEnabled", "enabled": true }),
        ),
    );
    assert_eq!(enabled.message_type, MessageType::Snapshot);

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut sequence = 3_u64;
    let baseline = loop {
        let snapshot = exchange(
            &mut connection,
            &command(
                "wait-for-first-anchor",
                sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        sequence = sequence.saturating_add(1);
        let link = required_object(&snapshot.payload, "abletonLinkIntegration");
        if required_u64(link, "receivedAnchorCount") > 0 {
            break snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "the simulator was discovered but no precise beat anchor arrived"
        );
        thread::sleep(Duration::from_millis(250));
    };
    let baseline_link = required_object(&baseline.payload, "abletonLinkIntegration");
    let baseline_input = required_object(&baseline.payload, "deckInputIntegration");
    let baseline_pumps = required_u64(baseline_link, "enginePumpCount");
    let baseline_received = required_u64(baseline_link, "receivedAnchorCount");
    let baseline_applied = required_u64(baseline_link, "appliedAnchorCount");
    let baseline_frames = required_u64(baseline_input, "receivedMessageCount");

    thread::sleep(Duration::from_secs(3));
    let observed = exchange(
        &mut connection,
        &command(
            "snapshot-after-idle",
            sequence,
            json!({ "kind": "getSnapshot" }),
        ),
    );

    let deck_source = required_object(&observed.payload, "deckSource");
    assert_eq!(
        deck_source.get("providerKind").and_then(Value::as_str),
        Some("directProDjLink")
    );
    let input = required_object(&observed.payload, "deckInputIntegration");
    assert!(required_u64(input, "receivedMessageCount") > baseline_frames);
    assert!(
        input
            .get("discoveredPlayers")
            .and_then(Value::as_array)
            .is_some_and(|players| !players.is_empty())
    );
    let link = required_object(&observed.payload, "abletonLinkIntegration");
    assert!(required_u64(link, "enginePumpCount").saturating_sub(baseline_pumps) >= 100);
    assert!(required_u64(link, "receivedAnchorCount") > baseline_received);
    assert!(required_u64(link, "appliedAnchorCount") > baseline_applied);
    assert!(
        link.get("bpmMilli")
            .and_then(Value::as_u64)
            .is_some_and(|bpm| bpm > 0)
    );
    assert_eq!(required_u64(link, "failureCount"), 0);

    drop(connection);
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("engine should exit after disconnect: {error}"));
    assert!(status.success());
    remove_database(&database);
}

fn start_engine(database: &Path) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lumi-engine"));
    command
        .env("LUMI_SESSION_TOKEN", TEST_SESSION_TOKEN)
        .env("LUMI_LIBRARY_DATABASE_PATH", database)
        .env("LUMI_DECK_INPUT_DISABLED", "1")
        .env("LUMI_AUTO_PUBLISH_MIDI", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    command
        .spawn()
        .unwrap_or_else(|error| panic!("engine should start: {error}"))
}

fn connect_and_authenticate(child: &mut Child) -> TcpStream {
    let stdout = child
        .stdout
        .take()
        .unwrap_or_else(|| panic!("engine stdout should be captured"));
    let mut ready_line = String::new();
    BufReader::new(stdout)
        .read_line(&mut ready_line)
        .unwrap_or_else(|error| panic!("startup record should be readable: {error}"));
    let ready: StartupReady = serde_json::from_str(&ready_line)
        .unwrap_or_else(|error| panic!("startup record should decode: {error}"));
    let mut connection = TcpStream::connect((ready.host.as_str(), ready.port))
        .unwrap_or_else(|error| panic!("engine should accept loopback connection: {error}"));
    connection
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap_or_else(|error| panic!("read timeout should set: {error}"));
    writeln!(connection, "{{\"sessionToken\":\"{TEST_SESSION_TOKEN}\"}}")
        .unwrap_or_else(|error| panic!("authentication should write: {error}"));
    connection
}

fn command(message_id: &str, sequence: u64, payload: Value) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "messageType": "command",
        "messageId": message_id,
        "sequence": sequence,
        "correlationId": message_id,
        "sentAt": "2026-08-12T12:00:00Z",
        "payload": payload,
    })
}

fn exchange(connection: &mut TcpStream, command: &Value) -> MessageEnvelope {
    writeln!(connection, "{command}")
        .unwrap_or_else(|error| panic!("command should write: {error}"));
    read_response(connection)
}

fn read_response(connection: &mut TcpStream) -> MessageEnvelope {
    let mut line = String::new();
    BufReader::new(&*connection)
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("response should be readable: {error}"));
    MessageDecoder::decode(line.as_bytes())
        .unwrap_or_else(|error| panic!("response should decode: {error}"))
}

fn required_object<'a>(
    value: &'a serde_json::Map<String, Value>,
    field: &str,
) -> &'a serde_json::Map<String, Value> {
    value
        .get(field)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{field} should be an object"))
}

fn required_u64(value: &serde_json::Map<String, Value>, field: &str) -> u64 {
    value
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("{field} should be an unsigned integer"))
}

fn temporary_database_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "lumi-prolink-network-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("acceptance")
    ))
}

fn remove_database(database: &Path) {
    for path in [
        database.to_path_buf(),
        PathBuf::from(format!("{}-wal", database.display())),
        PathBuf::from(format!("{}-shm", database.display())),
    ] {
        let _ = fs::remove_file(path);
    }
}
