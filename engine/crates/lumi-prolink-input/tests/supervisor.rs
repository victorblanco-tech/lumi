use std::thread;
use std::time::{Duration, Instant};

use lumi_prolink_input::{BridgeEvent, BridgeLaunchConfiguration, BridgeProcessSupervisor};

const HELLO: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":1,"observedAtNanos":10,"type":"hello","payload":{"bridgeVersion":"0.4.0-dev-20","beatLinkVersion":"8.0.0","readOnly":true}}"#;
const READY: &str = r#"{"protocol":"lumi-prolink-bridge","protocolVersion":1,"sequence":2,"observedAtNanos":20,"type":"sourceStatus","payload":{"status":"ready","detail":"test bridge ready"}}"#;

#[test]
fn supervises_and_decodes_a_bridge_process() {
    let script = format!("printf '%s\\n' '{HELLO}' '{READY}'; sleep 2");
    let configuration =
        BridgeLaunchConfiguration::command("/bin/sh", vec!["-c".to_owned(), script]);
    let mut supervisor = BridgeProcessSupervisor::spawn(&configuration)
        .unwrap_or_else(|error| panic!("bridge should launch: {error}"));

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut messages = Vec::new();
    while messages.len() < 2 && Instant::now() < deadline {
        messages.extend(
            supervisor
                .drain_messages()
                .unwrap_or_else(|error| panic!("bridge output should decode: {error}")),
        );
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(messages.len(), 2);
    assert!(matches!(messages[0].event, BridgeEvent::Hello(_)));
    assert!(matches!(messages[1].event, BridgeEvent::SourceStatus(_)));
    let diagnostics = supervisor
        .diagnostics()
        .unwrap_or_else(|error| panic!("diagnostics should be available: {error}"));
    assert!(diagnostics.running);
    assert_eq!(diagnostics.last_sequence, Some(2));
}
