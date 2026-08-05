use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lumi_engine::StartupReady;
use lumi_protocol::{MessageDecoder, MessageEnvelope, MessageType, PROTOCOL_VERSION};
use serde_json::{Value, json};

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
    assert_eq!(
        snapshot.payload.get("stateRevision"),
        Some(&Value::from(10))
    );
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
        Some(&Value::from(10))
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
    let Some(next_plan) = snapshot.payload.get("nextPlan").and_then(Value::as_object) else {
        let _ = child.kill();
        panic!("snapshot must contain the next-track plan");
    };
    assert_eq!(next_plan.get("deckId"), Some(&Value::from(2)));
    assert_eq!(
        next_plan.get("status"),
        Some(&Value::String("ready".to_owned()))
    );
    assert_eq!(next_plan.get("revision"), Some(&Value::from(1)));
    assert_eq!(
        next_plan
            .get("themeDecision")
            .and_then(|decision| decision.get("themeId")),
        Some(&Value::from(2))
    );
    assert_eq!(
        next_plan
            .get("themeDecision")
            .and_then(|decision| decision.get("reason")),
        Some(&Value::String("colorPrefer".to_owned()))
    );
    assert_eq!(
        next_plan
            .get("cues")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(4)
    );

    let Some(plan_id) = next_plan.get("planId").and_then(Value::as_str) else {
        let _ = child.kill();
        panic!("planId must be a decimal string");
    };
    let theme_command = command(
        "theme-1",
        1,
        json!({
            "kind": "selectTheme",
            "planId": plan_id,
            "trackLoadId": 2001,
            "themeId": 1,
            "expectedPlanRevision": 1,
        }),
    );
    let themed = exchange(&mut connection, &theme_command);
    assert_eq!(plan_revision(&themed), 2);
    assert_eq!(
        themed
            .payload
            .get("nextPlan")
            .and_then(|plan| plan.get("themeDecision"))
            .and_then(|decision| decision.get("reason")),
        Some(&Value::String("planInstanceUserChoice".to_owned()))
    );
    assert!(plan_cues(&themed).iter().all(|cue| {
        cue.get("action").and_then(|action| action.get("themeId")) == Some(&Value::from(1))
    }));

    let duplicate = exchange(&mut connection, &theme_command);
    assert_eq!(plan_revision(&duplicate), 2);
    assert_eq!(
        duplicate.payload.get("stateRevision"),
        themed.payload.get("stateRevision")
    );

    let locked = exchange(
        &mut connection,
        &command(
            "lock-1",
            2,
            json!({
                "kind": "setCueLock",
                "planId": plan_id,
                "trackLoadId": 2001,
                "phraseIndex": 1,
                "locked": true,
                "expectedPlanRevision": 2,
            }),
        ),
    );
    assert_eq!(plan_revision(&locked), 3);
    assert_eq!(
        plan_cues(&locked)[1].get("locked"),
        Some(&Value::Bool(true))
    );

    let regenerated = exchange(
        &mut connection,
        &command(
            "regenerate-1",
            3,
            json!({
                "kind": "regeneratePlan",
                "planId": plan_id,
                "trackLoadId": 2001,
                "expectedPlanRevision": 3,
            }),
        ),
    );
    assert_eq!(plan_revision(&regenerated), 4);
    let regenerated_cues = plan_cues(&regenerated);
    assert_eq!(regenerated_cues[1].get("locked"), Some(&Value::Bool(true)));
    assert_eq!(
        regenerated_cues[1]
            .get("action")
            .and_then(|action| action.get("themeId")),
        Some(&Value::from(1))
    );
    assert_eq!(
        regenerated_cues[0]
            .get("action")
            .and_then(|action| action.get("themeId")),
        Some(&Value::from(1))
    );

    let conflict = exchange(
        &mut connection,
        &command(
            "stale-theme",
            4,
            json!({
                "kind": "selectTheme",
                "planId": plan_id,
                "trackLoadId": 2001,
                "themeId": 2,
                "expectedPlanRevision": 1,
            }),
        ),
    );
    assert_eq!(conflict.message_type, MessageType::Error);
    assert_eq!(
        conflict.payload.get("kind"),
        Some(&Value::String("revisionConflict".to_owned()))
    );
    assert_eq!(
        conflict.payload.get("actualPlanRevision"),
        Some(&Value::from(4))
    );

    let stale_state = exchange(
        &mut connection,
        &command(
            "stale-operation",
            5,
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
        regenerated.payload.get("stateRevision")
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
        &command("snapshot-after-faults", 6, json!({ "kind": "getSnapshot" })),
    );
    assert_eq!(
        after_faults.payload.get("operationState"),
        Some(&Value::String("off".to_owned()))
    );
    assert_eq!(
        after_faults
            .payload
            .get("outputProvider")
            .and_then(Value::as_object)
            .and_then(|output| output.get("recordCount")),
        Some(&Value::from(0))
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

fn plan_revision(snapshot: &MessageEnvelope) -> u64 {
    let Some(revision) = snapshot
        .payload
        .get("nextPlan")
        .and_then(Value::as_object)
        .and_then(|plan| plan.get("revision"))
        .and_then(Value::as_u64)
    else {
        panic!("snapshot must contain a plan revision");
    };
    revision
}

fn plan_cues(snapshot: &MessageEnvelope) -> &[Value] {
    let Some(cues) = snapshot
        .payload
        .get("nextPlan")
        .and_then(Value::as_object)
        .and_then(|plan| plan.get("cues"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        panic!("snapshot must contain plan cues");
    };
    cues
}
