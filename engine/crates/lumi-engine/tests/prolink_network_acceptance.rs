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
    let snapshot = exchange(
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

#[test]
#[ignore = "set LUMI_RUN_PROLINK_ONLY_SOAK=1 and LUMI_PROLINK_SOAK_SECONDS"]
fn prolink_only_configurable_soak_has_bounded_ingress_without_output_side_effects() {
    if std::env::var("LUMI_RUN_PROLINK_ONLY_SOAK").as_deref() != Ok("1") {
        return;
    }
    let duration_seconds: u64 = std::env::var("LUMI_PROLINK_SOAK_SECONDS")
        .unwrap_or_else(|_| panic!("LUMI_PROLINK_SOAK_SECONDS is required"))
        .parse()
        .unwrap_or_else(|error| panic!("LUMI_PROLINK_SOAK_SECONDS must be an integer: {error}"));
    assert!(
        duration_seconds > 0,
        "Pro DJ Link soak duration must be positive"
    );
    assert_eq!(
        simulator_status()
            .get("trafficProfile")
            .and_then(Value::as_str),
        Some("cdj-1500x")
    );
    simulator_control("master", Some("on"));
    simulator_control("on-air", Some("on"));
    simulator_control("pitch", Some("0"));
    simulator_control("play", None);

    let database = temporary_database_path();
    let mut child = start_engine(&database);
    let mut connection = connect_and_authenticate(&mut child);
    let initial = read_response(&mut connection);
    let mut sequence = 1_u64;
    let mut snapshot = exchange(
        &mut connection,
        &command(
            "prolink-only-select-connected",
            sequence,
            json!({
                "kind": "selectDeckSourceMode",
                "mode": "connectedDecks",
                "expectedStateRevision": required_u64(&initial.payload, "stateRevision"),
            }),
        ),
    );
    sequence = sequence.saturating_add(1);
    assert_eq!(
        snapshot.message_type,
        MessageType::Snapshot,
        "connected-deck selection failed: {:?}",
        snapshot.payload
    );
    let baseline_outputs = output_record_count(&snapshot);
    let finish = Instant::now() + Duration::from_secs(duration_seconds);
    let mut maximum_p95 = 0_u64;
    let mut polls = 0_u64;
    while Instant::now() < finish {
        thread::sleep(Duration::from_millis(100));
        snapshot = exchange(
            &mut connection,
            &command(
                "prolink-only-poll",
                sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        sequence = sequence.saturating_add(1);
        polls = polls.saturating_add(1);
        let input = required_object(&snapshot.payload, "deckInputIntegration");
        assert_eq!(required_u64(input, "ingressCriticalSaturationCount"), 0);
        assert!(
            required_u64(input, "ingressQueueDepth") <= required_u64(input, "ingressQueueCapacity")
        );
        maximum_p95 = maximum_p95.max(required_u64(input, "ingressSourceAgeP95Micros"));
        let link = required_object(&snapshot.payload, "abletonLinkIntegration");
        assert_eq!(link.get("enabled").and_then(Value::as_bool), Some(false));
        assert_eq!(output_record_count(&snapshot), baseline_outputs);
    }
    let input = required_object(&snapshot.payload, "deckInputIntegration");
    assert!(required_u64(input, "receivedMessageCount") > 0);
    assert!(required_u64(input, "ingressSourceAgeSampleCount") > 0);
    assert!(maximum_p95 <= 250_000, "Pro DJ Link p95 exceeded budget");
    println!(
        "Pro DJ Link-only soak: duration={}s polls={} messages={} p95={}us max={}us highWater={}",
        duration_seconds,
        polls,
        required_u64(input, "receivedMessageCount"),
        required_u64(input, "ingressSourceAgeP95Micros"),
        required_u64(input, "ingressSourceAgeMaxMicros"),
        required_u64(input, "ingressQueueHighWater"),
    );

    simulator_control("pause", None);
    drop(connection);
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("engine should exit after disconnect: {error}"));
    assert!(status.success());
    remove_database(&database);
}

/// Combined source, Link, lighting and aggressive snapshot-polling soak.
///
/// The artifact contains only bounded counters and percentiles. It deliberately
/// excludes track metadata and the simulator token so it can be retained as a
/// release-evidence asset.
#[test]
#[ignore = "set LUMI_RUN_LIVE_INTEGRATION_SOAK=1 and LUMI_LIVE_SOAK_SECONDS"]
fn combined_lanes_remain_bounded_and_emit_release_evidence() {
    if std::env::var("LUMI_RUN_LIVE_INTEGRATION_SOAK").as_deref() != Ok("1") {
        return;
    }
    let duration_seconds: u64 = std::env::var("LUMI_LIVE_SOAK_SECONDS")
        .unwrap_or_else(|_| panic!("LUMI_LIVE_SOAK_SECONDS is required"))
        .parse()
        .unwrap_or_else(|error| panic!("LUMI_LIVE_SOAK_SECONDS must be an integer: {error}"));
    assert!(duration_seconds > 0, "soak duration must be positive");
    let simulator = simulator_status();
    assert_eq!(
        simulator.get("trafficProfile").and_then(Value::as_str),
        Some("cdj-1500x"),
        "release evidence requires the representative cdj-1500x profile"
    );

    simulator_control("pause", None);
    simulator_control("seek", Some("60000"));
    simulator_control("master", Some("on"));
    simulator_control("on-air", Some("on"));
    simulator_control("pitch", Some("0"));

    let database = temporary_database_path();
    seed_network_database(&database);
    let mut child = start_engine(&database);
    let mut connection = connect_and_authenticate(&mut child);
    let initial = read_response(&mut connection);
    let mut sequence = 1_u64;
    let _connected = exchange(
        &mut connection,
        &command(
            "soak-select-connected",
            sequence,
            json!({
                "kind": "selectDeckSourceMode",
                "mode": "connectedDecks",
                "expectedStateRevision": required_u64(&initial.payload, "stateRevision"),
            }),
        ),
    );
    sequence = sequence.saturating_add(1);
    let mut snapshot = exchange(
        &mut connection,
        &command(
            "soak-enable-link",
            sequence,
            json!({ "kind": "setAbletonLinkEnabled", "enabled": true }),
        ),
    );
    assert_eq!(snapshot.message_type, MessageType::Snapshot);
    sequence = sequence.saturating_add(1);

    // The harness disables auto-publication. An output record alone is not
    // proof of MIDI dispatch: the runtime can record an AutoLoop while the
    // virtual source is stopped. This acceptance test requires real dispatch.
    snapshot = exchange(
        &mut connection,
        &command(
            "soak-publish-midi",
            sequence,
            json!({ "kind": "publishMidiSource" }),
        ),
    );
    sequence = sequence.saturating_add(1);
    let midi = required_object(&snapshot.payload, "midiIntegration");
    assert_eq!(midi.get("state").and_then(Value::as_str), Some("ready"));
    let baseline_pulses = required_u64(midi, "sentPulseCount");

    let ready_deadline = Instant::now() + Duration::from_secs(15);
    while !snapshot
        .payload
        .get("livePlan")
        .is_some_and(Value::is_object)
        || required_object(&snapshot.payload, "deckInputIntegration")
            .get("sourceState")
            .and_then(Value::as_str)
            != Some("ready")
    {
        assert!(
            Instant::now() < ready_deadline,
            "the representative simulator did not produce an exact Live plan"
        );
        thread::sleep(Duration::from_millis(50));
        snapshot = exchange(
            &mut connection,
            &command(
                "soak-wait-ready",
                sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        sequence = sequence.saturating_add(1);
    }

    for (message_id, state) in [("soak-arm", "armed"), ("soak-start", "live")] {
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
    let baseline_outputs = output_record_count(&snapshot);
    simulator_control("play", None);

    let started = Instant::now();
    let finish = started + Duration::from_secs(duration_seconds);
    let mut next_pitch = started + Duration::from_secs(2);
    let mut next_seek = started + Duration::from_secs(7);
    let mut next_operation_cycle = started + Duration::from_secs(11);
    let pitches = [("4.2", 161_510_u64), ("-2", 151_900), ("0", 155_000)];
    let seeks = [280_000_u64, 1_000, 60_000];
    let mut pitch_index = 0_usize;
    let mut seek_index = 0_usize;
    let mut pitch_changes = 0_u64;
    let mut seek_landings = 0_u64;
    let mut operation_cycles = 0_u64;
    let mut snapshot_polls = 0_u64;
    let mut maximum_queue_depth = 0_u64;
    let mut maximum_queue_high_water = 0_u64;
    let mut maximum_source_age_micros = 0_u64;
    let mut maximum_source_age_p95_micros = 0_u64;
    let mut maximum_engine_lateness_micros = 0_u64;
    let mut maximum_realtime_midi_p95_micros = 0_u64;

    while Instant::now() < finish {
        let now = Instant::now();
        if now >= next_pitch {
            let (pitch, _) = pitches[pitch_index % pitches.len()];
            simulator_control("pitch", Some(pitch));
            pitch_index = pitch_index.saturating_add(1);
            pitch_changes = pitch_changes.saturating_add(1);
            next_pitch += Duration::from_secs(2);
        }
        if now >= next_seek {
            let target = seeks[seek_index % seeks.len()];
            simulator_control("seek", Some(&target.to_string()));
            seek_index = seek_index.saturating_add(1);
            seek_landings = seek_landings.saturating_add(1);
            next_seek += Duration::from_secs(7);
        }
        if now >= next_operation_cycle {
            for (suffix, state) in [("pause", "paused"), ("resume", "live")] {
                snapshot = exchange(
                    &mut connection,
                    &command(
                        &format!("soak-{suffix}-{operation_cycles}"),
                        sequence,
                        json!({
                            "kind": "setOperationState",
                            "operationState": state,
                            "expectedStateRevision": required_u64(
                                &snapshot.payload,
                                "stateRevision",
                            ),
                        }),
                    ),
                );
                sequence = sequence.saturating_add(1);
            }
            operation_cycles = operation_cycles.saturating_add(1);
            next_operation_cycle += Duration::from_secs(11);
        }

        snapshot = exchange(
            &mut connection,
            &command(
                "soak-aggressive-ui-poll",
                sequence,
                json!({ "kind": "getSnapshot" }),
            ),
        );
        sequence = sequence.saturating_add(1);
        snapshot_polls = snapshot_polls.saturating_add(1);

        let input = required_object(&snapshot.payload, "deckInputIntegration");
        assert_eq!(
            input.get("sourceState").and_then(Value::as_str),
            Some("ready")
        );
        assert_eq!(required_u64(input, "ingressCriticalSaturationCount"), 0);
        let capacity = required_u64(input, "ingressQueueCapacity");
        let depth = required_u64(input, "ingressQueueDepth");
        let high_water = required_u64(input, "ingressQueueHighWater");
        assert!(depth <= capacity, "Pro DJ Link queue exceeded capacity");
        assert!(
            high_water <= capacity,
            "Pro DJ Link high-water exceeded capacity"
        );
        maximum_queue_depth = maximum_queue_depth.max(depth);
        maximum_queue_high_water = maximum_queue_high_water.max(high_water);
        maximum_source_age_micros =
            maximum_source_age_micros.max(required_u64(input, "ingressSourceAgeMaxMicros"));
        maximum_source_age_p95_micros =
            maximum_source_age_p95_micros.max(required_u64(input, "ingressSourceAgeP95Micros"));

        let link = required_object(&snapshot.payload, "abletonLinkIntegration");
        assert_eq!(required_u64(link, "failureCount"), 0);
        assert_eq!(required_u64(link, "failClosedCount"), 0);
        maximum_engine_lateness_micros =
            maximum_engine_lateness_micros.max(required_u64(link, "enginePumpMaxLatenessMicros"));

        let midi = required_object(&snapshot.payload, "midiIntegration");
        assert_eq!(midi.get("state").and_then(Value::as_str), Some("ready"));
        let scheduler = required_nested_object(midi, "realtimeScheduler");
        assert_eq!(required_u64(scheduler, "failedCount"), 0);
        let lane = required_nested_object(scheduler, "lane");
        assert_eq!(required_u64(lane, "saturationCount"), 0);
        assert!(
            required_u64(lane, "queueDepth") <= required_u64(lane, "queueCapacity"),
            "realtime MIDI queue exceeded capacity"
        );
        maximum_realtime_midi_p95_micros =
            maximum_realtime_midi_p95_micros.max(required_u64(lane, "latencyP95Micros"));
        thread::sleep(Duration::from_millis(25));
    }

    // The cumulative histogram uses finite buckets and must stay inside the
    // release budget even while UI polling and control mutations share the
    // process. The maximum remains evidence rather than a brittle scheduler
    // assertion; p95 is the operating budget.
    assert!(
        maximum_source_age_p95_micros <= 250_000,
        "source-to-engine p95 exceeded 250 ms: {maximum_source_age_p95_micros} µs"
    );
    assert!(
        maximum_realtime_midi_p95_micros <= 20_000,
        "realtime MIDI p95 exceeded 20 ms: {maximum_realtime_midi_p95_micros} µs"
    );
    assert!(
        output_record_count(&snapshot) > baseline_outputs,
        "combined soak did not execute an AutoLoop"
    );

    let final_input = required_object(&snapshot.payload, "deckInputIntegration");
    let final_link = required_object(&snapshot.payload, "abletonLinkIntegration");
    let final_midi = required_object(&snapshot.payload, "midiIntegration");
    let final_scheduler = required_nested_object(final_midi, "realtimeScheduler");
    let final_lane = required_nested_object(final_scheduler, "lane");
    assert_midi_dispatch_evidence(final_midi, baseline_pulses);
    let evidence = json!({
        "schemaVersion": 2,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "simulatorProfile": "cdj-1500x",
        "durationSeconds": duration_seconds,
        "uiSnapshotPolls": snapshot_polls,
        "actions": {
            "pitchChanges": pitch_changes,
            "seekLandings": seek_landings,
            "lightingOperationCycles": operation_cycles,
        },
        "proDjLink": {
            "receivedMessages": required_u64(final_input, "receivedMessageCount"),
            "queueCapacity": required_u64(final_input, "ingressQueueCapacity"),
            "maximumObservedDepth": maximum_queue_depth,
            "queueHighWater": maximum_queue_high_water,
            "coalescedMessages": required_u64(final_input, "ingressCoalescedMessageCount"),
            "criticalSaturation": required_u64(final_input, "ingressCriticalSaturationCount"),
            "sourceAgeSamples": required_u64(final_input, "ingressSourceAgeSampleCount"),
            "sourceAgeP50Micros": required_u64(final_input, "ingressSourceAgeP50Micros"),
            "sourceAgeP95Micros": required_u64(final_input, "ingressSourceAgeP95Micros"),
            "sourceAgeP99Micros": required_u64(final_input, "ingressSourceAgeP99Micros"),
            "sourceAgeMaxMicros": maximum_source_age_micros,
            "positionDiscontinuities": required_u64(final_input, "positionDiscontinuityCount"),
        },
        "abletonLink": {
            "peers": required_u64(final_link, "peers"),
            "receivedAnchors": required_u64(final_link, "receivedAnchorCount"),
            "appliedAnchors": required_u64(final_link, "appliedAnchorCount"),
            "hardReanchors": required_u64(final_link, "hardReanchorCount"),
            "failClosed": required_u64(final_link, "failClosedCount"),
            "failures": required_u64(final_link, "failureCount"),
            "enginePumpStarvation": required_u64(final_link, "enginePumpStarvationCount"),
            "enginePumpMaxLatenessMicros": maximum_engine_lateness_micros,
        },
        "autoLoop": {
            "outputs": output_record_count(&snapshot),
            "sentPulses": required_u64(final_midi, "sentPulseCount") - baseline_pulses,
            "executionEpoch": required_u64(final_scheduler, "executionEpoch"),
            "requested": required_u64(final_scheduler, "requestedCount"),
            "completed": required_u64(final_scheduler, "completedCount"),
            "duplicatesSuppressed": required_u64(final_scheduler, "duplicateCount"),
            "cancelled": required_u64(final_scheduler, "cancelledCount"),
            "failed": required_u64(final_scheduler, "failedCount"),
            "laneQueueHighWater": required_u64(final_lane, "queueHighWater"),
            "laneSaturation": required_u64(final_lane, "saturationCount"),
            "laneScheduled": required_u64(final_lane, "scheduledCount"),
            "laneEmitted": required_u64(final_lane, "emittedCount"),
            "laneLatencySamples": required_u64(final_lane, "latencySampleCount"),
            "laneLatencyP50Micros": required_u64(final_lane, "latencyP50Micros"),
            "laneLatencyP95Micros": required_u64(final_lane, "latencyP95Micros"),
            "laneLatencyP99Micros": required_u64(final_lane, "latencyP99Micros"),
            "laneLatencyMaxMicros": required_u64(final_lane, "latencyMaxMicros"),
        },
    });
    if let Ok(path) = std::env::var("LUMI_LIVE_EVIDENCE_PATH") {
        fs::write(
            &path,
            serde_json::to_vec_pretty(&evidence)
                .unwrap_or_else(|error| panic!("evidence should encode: {error}")),
        )
        .unwrap_or_else(|error| panic!("evidence should write to {path}: {error}"));
    }
    println!(
        "Combined Live soak evidence: {}",
        serde_json::to_string(&evidence)
            .unwrap_or_else(|error| panic!("evidence should encode: {error}"))
    );

    simulator_control("pause", None);
    simulator_control("pitch", Some("0"));
    drop(connection);
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("engine should exit after disconnect: {error}"));
    assert!(status.success());
    remove_database(&database);
}

fn assert_midi_dispatch_evidence(midi: &serde_json::Map<String, Value>, baseline_pulses: u64) {
    assert_eq!(midi.get("state").and_then(Value::as_str), Some("ready"));
    assert!(
        required_u64(midi, "sentPulseCount") > baseline_pulses,
        "no MIDI pulses were actually sent during the combined soak"
    );
    let scheduler = required_nested_object(midi, "realtimeScheduler");
    assert!(required_u64(scheduler, "requestedCount") > 0);
    assert!(required_u64(scheduler, "completedCount") > 0);
    let lane = required_nested_object(scheduler, "lane");
    assert!(required_u64(lane, "scheduledCount") > 0);
    assert!(required_u64(lane, "emittedCount") > 0);
    assert!(
        required_u64(lane, "latencySampleCount") > 0,
        "zero latency without any samples is not timing acceptance"
    );
}

#[test]
fn midi_acceptance_rejects_empty_dispatch_and_latency_counters() {
    let valid = json!({
        "state": "ready", "sentPulseCount": 4,
        "realtimeScheduler": {
            "requestedCount": 1, "completedCount": 1,
            "lane": { "scheduledCount": 2, "emittedCount": 2, "latencySampleCount": 2 }
        }
    });
    let valid_object = valid
        .as_object()
        .unwrap_or_else(|| panic!("fixture is an object"));
    assert_midi_dispatch_evidence(valid_object, 0);
    for pointer in [
        "/sentPulseCount",
        "/realtimeScheduler/requestedCount",
        "/realtimeScheduler/completedCount",
        "/realtimeScheduler/lane/scheduledCount",
        "/realtimeScheduler/lane/emittedCount",
        "/realtimeScheduler/lane/latencySampleCount",
    ] {
        let mut empty = valid.clone();
        *empty
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("fixture contains {pointer}")) = json!(0);
        assert!(
            std::panic::catch_unwind(|| {
                assert_midi_dispatch_evidence(
                    empty
                        .as_object()
                        .unwrap_or_else(|| panic!("fixture remains an object")),
                    0,
                );
            })
            .is_err(),
            "empty {pointer} must not pass acceptance"
        );
    }
    assert!(
        std::panic::catch_unwind(|| {
            assert_midi_dispatch_evidence(valid_object, 4);
        })
        .is_err(),
        "old pulses must not count as output from this run"
    );
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

fn required_nested_object<'a>(
    value: &'a serde_json::Map<String, Value>,
    field: &str,
) -> &'a serde_json::Map<String, Value> {
    required_object(value, field)
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
    let output = process
        .output()
        .unwrap_or_else(|error| panic!("simulator {command} should launch: {error}"));
    assert!(
        output.status.success(),
        "simulator {command} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn simulator_status() -> Value {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/prolink-simulatorctl.sh");
    let output = Command::new(script)
        .arg("status")
        .output()
        .unwrap_or_else(|error| panic!("simulator status should launch: {error}"));
    assert!(output.status.success(), "simulator status should succeed");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("simulator status should be JSON: {error}"))
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
