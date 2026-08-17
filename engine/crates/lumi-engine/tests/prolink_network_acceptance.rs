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
        if required_u64(link, "receivedAnchorCount") > 0
            && required_u64(link, "appliedAnchorCount") == required_u64(link, "receivedAnchorCount")
        {
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
    assert_eq!(
        required_u64(link, "receivedAnchorCount"),
        baseline_received,
        "unchanged master beats must not repeatedly correct Ableton Link"
    );
    assert_eq!(
        required_u64(link, "appliedAnchorCount"),
        baseline_applied,
        "unchanged master beats must not repeatedly re-anchor SoundSwitch"
    );
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

/// A CDJ pitch-slider movement is a latest-value BPM change, not a recurring
/// beat-phase correction. Each stable value must reach Link once and remain
/// stable while exact beat traffic continues.
#[test]
#[ignore = "requires the Lumi Pro DJ Link network simulator"]
fn master_pitch_changes_reach_link_once_without_old_value_regression() {
    if std::env::var("LUMI_RUN_PROLINK_NETWORK_TEST").as_deref() != Ok("1") {
        return;
    }
    simulator_control("master", Some("on"));
    simulator_control("play", None);
    simulator_control("pitch", Some("0"));

    let database = temporary_database_path();
    let mut child = start_engine(&database);
    let mut connection = connect_and_authenticate(&mut child);
    let initial = read_response(&mut connection);
    let mut sequence = 1_u64;
    let _connected = exchange(
        &mut connection,
        &command(
            "select-connected-decks-pitch",
            sequence,
            json!({
                "kind": "selectDeckSourceMode",
                "mode": "connectedDecks",
                "expectedStateRevision": required_u64(&initial.payload, "stateRevision"),
            }),
        ),
    );
    sequence = sequence.saturating_add(1);
    let _enabled = exchange(
        &mut connection,
        &command(
            "enable-link-pitch",
            sequence,
            json!({ "kind": "setAbletonLinkEnabled", "enabled": true }),
        ),
    );
    sequence = sequence.saturating_add(1);

    let mut applied = 0_u64;
    for (pitch, expected_bpm) in [("0", 155_000_u64), ("4.2", 161_510), ("-2", 151_900)] {
        simulator_control("pitch", Some(pitch));
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let snapshot = exchange(
                &mut connection,
                &command(
                    "wait-for-link-pitch",
                    sequence,
                    json!({ "kind": "getSnapshot" }),
                ),
            );
            sequence = sequence.saturating_add(1);
            let link = required_object(&snapshot.payload, "abletonLinkIntegration");
            let current_applied = required_u64(link, "appliedAnchorCount");
            if link.get("bpmMilli").and_then(Value::as_u64) == Some(expected_bpm)
                && current_applied > applied
            {
                applied = current_applied;
                break;
            }
            assert!(
                Instant::now() < deadline,
                "pitch {pitch} did not reach Link as {expected_bpm}"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    thread::sleep(Duration::from_secs(1));
    let stable = exchange(
        &mut connection,
        &command(
            "stable-link-pitch",
            sequence,
            json!({ "kind": "getSnapshot" }),
        ),
    );
    let link = required_object(&stable.payload, "abletonLinkIntegration");
    assert_eq!(link.get("bpmMilli").and_then(Value::as_u64), Some(151_900));
    assert_eq!(required_u64(link, "appliedAnchorCount"), applied);

    simulator_control("pause", None);
    simulator_control("pitch", Some("0"));
    drop(connection);
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("engine should exit after disconnect: {error}"));
    assert!(status.success());
    remove_database(&database);
}

/// Reproduces the physical failure sequence reported from Live Decks: Lumi is
/// already in Start while the Master is cued and stopped, then deck playback
/// begins. The current phrase must execute before any later phrase transition.
/// Operational Pause/Start must subsequently restore that same phrase once.
#[test]
#[ignore = "requires the Lumi Pro DJ Link network simulator and a synced Dev library"]
fn stopped_live_deck_start_and_operation_resume_restore_the_current_autoloop() {
    if std::env::var("LUMI_RUN_PROLINK_OUTPUT_TEST").as_deref() != Ok("1") {
        return;
    }

    simulator_control("pause", None);
    simulator_control("seek", Some("60000"));
    simulator_control("master", Some("on"));
    simulator_control("on-air", Some("on"));

    let database = temporary_database_path();
    seed_network_database(&database);
    let mut child = start_engine(&database);
    let mut connection = connect_and_authenticate(&mut child);
    let initial = read_response(&mut connection);
    let mut sequence = 1_u64;
    let mut snapshot = exchange(
        &mut connection,
        &command(
            "select-connected-decks-output",
            sequence,
            json!({
                "kind": "selectDeckSourceMode",
                "mode": "connectedDecks",
                "expectedStateRevision": required_u64(&initial.payload, "stateRevision"),
            }),
        ),
    );
    sequence = sequence.saturating_add(1);

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let has_stopped_master = snapshot
            .payload
            .get("leaderDeckId")
            .and_then(Value::as_u64)
            .is_some()
            && snapshot
                .payload
                .get("decks")
                .and_then(Value::as_array)
                .is_some_and(|decks| {
                    decks.iter().any(|deck| {
                        deck.get("playing").and_then(Value::as_bool) == Some(false)
                            && deck.get("planEligibility").and_then(Value::as_str)
                                == Some("readyExact")
                    })
                })
            && snapshot
                .payload
                .get("livePlan")
                .is_some_and(Value::is_object);
        if has_stopped_master {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "simulated Master did not become a stopped, exactly matched planned deck"
        );
        thread::sleep(Duration::from_millis(200));
        snapshot = exchange(
            &mut connection,
            &command(
                "wait-for-planned-master",
                sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        sequence = sequence.saturating_add(1);
    }

    for (message_id, state) in [("arm-output", "armed"), ("start-output", "live")] {
        snapshot = exchange(
            &mut connection,
            &command(
                message_id,
                sequence,
                json!({
                    "kind": "setOperationState",
                    "operationState": state,
                    "expectedStateRevision": required_u64(&snapshot.payload, "stateRevision"),
                }),
            ),
        );
        sequence = sequence.saturating_add(1);
        assert_eq!(
            snapshot
                .payload
                .get("operationState")
                .and_then(Value::as_str),
            Some(state)
        );
    }

    let baseline = output_record_count(&snapshot);
    simulator_control("play", None);
    snapshot = wait_for_output_count(
        &mut connection,
        &mut sequence,
        baseline.saturating_add(1),
        "deck playback start",
    );
    assert_eq!(output_record_count(&snapshot), baseline.saturating_add(1));

    for (message_id, state) in [("pause-output", "paused"), ("resume-output", "live")] {
        snapshot = exchange(
            &mut connection,
            &command(
                message_id,
                sequence,
                json!({
                    "kind": "setOperationState",
                    "operationState": state,
                    "expectedStateRevision": required_u64(&snapshot.payload, "stateRevision"),
                }),
            ),
        );
        sequence = sequence.saturating_add(1);
    }
    assert_eq!(output_record_count(&snapshot), baseline.saturating_add(2));

    let before_seek_output = output_record_count(&snapshot);
    simulator_control("seek", Some("280000"));
    let _ = wait_for_playback_position(
        &mut connection,
        &mut sequence,
        |position| position >= 270_000,
        "seek near track end after Pause/Start",
    );
    let near_end_snapshot = wait_for_output_count(
        &mut connection,
        &mut sequence,
        before_seek_output.saturating_add(1),
        "forward seek landing",
    );
    assert_eq!(
        output_record_count(&near_end_snapshot),
        before_seek_output.saturating_add(1),
        "one seek landing may select exactly one AutoLoop"
    );
    let near_end_revision = deck_transport_revision(&near_end_snapshot);
    simulator_control("seek", Some("1000"));
    let _ = wait_for_playback_position(
        &mut connection,
        &mut sequence,
        |position| position <= 10_000,
        "backward seek to track start after Pause/Start",
    );
    let _ = wait_for_output_count(
        &mut connection,
        &mut sequence,
        before_seek_output.saturating_add(2),
        "backward seek landing",
    );
    thread::sleep(Duration::from_millis(300));
    snapshot = exchange(
        &mut connection,
        &command(
            "verify-no-seek-duplicate",
            sequence,
            json!({ "kind": "getSnapshot" }),
        ),
    );
    assert_eq!(
        output_record_count(&snapshot),
        before_seek_output.saturating_add(2),
        "stale position bursts may not duplicate a landing AutoLoop"
    );
    assert!(
        snapshot
            .payload
            .get("decks")
            .and_then(Value::as_array)
            .and_then(|decks| decks.first())
            .and_then(|deck| deck.get("beat"))
            .and_then(Value::as_u64)
            .is_some_and(|beat| beat < 40),
        "the authoritative deck beat must also regress to the track start"
    );
    assert!(
        deck_transport_revision(&snapshot) > near_end_revision,
        "the backward seek must publish a new transport revision for visual consumers"
    );

    let landing_phrase = deck_phrase_index(&snapshot);
    let before_same_phrase_seek = output_record_count(&snapshot);
    simulator_control("seek", Some("5000"));
    let same_phrase_landing = wait_for_playback_position(
        &mut connection,
        &mut sequence,
        |position| (4_000..=10_000).contains(&position),
        "seek within the current phrase",
    );
    assert_eq!(
        deck_phrase_index(&same_phrase_landing),
        landing_phrase,
        "the acceptance fixture must keep this landing inside one phrase"
    );
    let same_phrase_output = wait_for_output_count(
        &mut connection,
        &mut sequence,
        before_same_phrase_seek.saturating_add(1),
        "same-phrase seek landing",
    );
    thread::sleep(Duration::from_millis(300));
    snapshot = exchange(
        &mut connection,
        &command(
            "verify-no-same-phrase-seek-duplicate",
            sequence,
            json!({ "kind": "getSnapshot" }),
        ),
    );
    assert_eq!(
        output_record_count(&snapshot),
        output_record_count(&same_phrase_output),
        "one confirmed seek inside a phrase must reassert that cue exactly once"
    );

    simulator_control("pause", None);
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
        .env("LUMI_EXIT_AFTER_CLIENT_DISCONNECT", "1")
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

fn output_record_count(snapshot: &MessageEnvelope) -> u64 {
    required_u64(
        required_object(&snapshot.payload, "outputProvider"),
        "recordCount",
    )
}

fn wait_for_output_count(
    connection: &mut TcpStream,
    sequence: &mut u64,
    expected: u64,
    reason: &str,
) -> MessageEnvelope {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = exchange(
            connection,
            &command(
                "wait-for-output",
                *sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        *sequence = sequence.saturating_add(1);
        if output_record_count(&snapshot) >= expected {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "current AutoLoop was not executed after {reason}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_playback_position(
    connection: &mut TcpStream,
    sequence: &mut u64,
    accepted: impl Fn(u64) -> bool,
    reason: &str,
) -> MessageEnvelope {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = exchange(
            connection,
            &command(
                "wait-for-position",
                *sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        *sequence = sequence.saturating_add(1);
        let position = snapshot
            .payload
            .get("decks")
            .and_then(Value::as_array)
            .and_then(|decks| decks.first())
            .and_then(|deck| deck.get("playbackPositionMillis"))
            .and_then(Value::as_u64);
        if position.is_some_and(&accepted) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "deck position did not follow {reason}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn deck_transport_revision(snapshot: &MessageEnvelope) -> u64 {
    snapshot
        .payload
        .get("decks")
        .and_then(Value::as_array)
        .and_then(|decks| decks.first())
        .and_then(|deck| deck.get("transportRevision"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("deck snapshot should expose transportRevision"))
}

fn deck_phrase_index(snapshot: &MessageEnvelope) -> u64 {
    snapshot
        .payload
        .get("decks")
        .and_then(Value::as_array)
        .and_then(|decks| decks.first())
        .and_then(|deck| deck.get("phraseIndex"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("deck snapshot should expose phraseIndex"))
}

fn seed_network_database(destination: &Path) {
    let source = std::env::var("LUMI_PROLINK_NETWORK_DATABASE").unwrap_or_else(|_| {
        panic!("LUMI_PROLINK_NETWORK_DATABASE must point to the synced Dev DB")
    });
    fs::copy(&source, destination)
        .unwrap_or_else(|error| panic!("synced Dev database should copy from {source}: {error}"));
}

fn simulator_control(command: &str, argument: Option<&str>) {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/prolink-simulatorctl.sh");
    let mut process = Command::new(script);
    process.arg(command);
    if let Some(argument) = argument {
        process.arg(argument);
    }
    let status = process
        .status()
        .unwrap_or_else(|error| panic!("simulator {command} should launch: {error}"));
    assert!(status.success(), "simulator {command} should succeed");
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
