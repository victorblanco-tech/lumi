#!/usr/bin/env python3
"""Read the bounded local recorder; never connect to or control Lumi.

All comparisons are in the recorder's monotonic clock. Raw CDJ status beats
are one-based. Reception is NOT physical output or SoundSwitch phase feedback.
"""
import argparse
import json
from pathlib import Path


def percentile(values, fraction):
    values = sorted(values)
    return round(values[min(len(values) - 1, int((len(values) - 1) * fraction))], 3) if values else None


def analyze(document):
    events = document["events"]
    report = {"version": document.get("version"), "processId": document.get("processId"),
              "records": len(events), "droppedRecords": max((e.get("droppedRecords", 0) for e in events), default=0),
              "scope": "CDJ reception through MIDI provider call; not SoundSwitch or physical light phase"}
    report["queueMilliseconds"] = {
        key: {"p50": percentile(values, .5), "p95": percentile(values, .95), "max": max(values, default=None)}
        for key in ("bridgeQueueAgeMicros", "ingressQueueAgeMicros")
        for values in ([e[key] / 1000 for e in events if key in e],)
    }
    sends = []
    for schedule in (e for e in events if e["stage"] == "schedule"):
        dispatched = next((e for e in events if e["stage"] == "midiDispatch"
                           and e["generation"] == schedule["generation"] and e["action"] == "Autoloop"), None)
        row = {k: schedule[k] for k in ("generation", "deck", "phrase", "bank", "autoloop", "observedBeat", "offsetMillis")}
        row["leadMilliseconds"] = schedule["leadMicros"] / 1000
        row["retimeRequests"] = sum(1 for e in events if e["stage"] == "scheduleRetimed" and e["generation"] == schedule["generation"])
        if dispatched:
            sent_at = dispatched["engineMicros"] - dispatched["completedAgeMicros"] - dispatched["providerMicros"]
            row.update(lateMilliseconds=dispatched["lateMicros"] / 1000,
                       providerMilliseconds=dispatched["providerMicros"] / 1000, succeeded=dispatched["succeeded"])
            # Find the corresponding phrase boundary, even if plan preparation
            # has already rolled out of the recorder's bounded history.
            boundary = next((e for e in events if e["stage"] == "reduced"
                             and e.get("observation", {}).get("kind") == "phrase"
                             and e["observation"].get("deck") == schedule["deck"]
                             and e["observation"].get("load") == schedule["load"]
                             and e["observation"].get("phrase") == schedule["phrase"]
                             and e["engineMicros"] >= schedule["engineMicros"]), None)
            if boundary:
                row["sendMinusReducedPhraseMilliseconds"] = round((sent_at - boundary["engineMicros"]) / 1000, 3)
            statuses = [e for e in events if e["stage"] == "ingress"
                        and e.get("input", {}).get("kind") == "status"
                        and e["input"].get("deck") == schedule["deck"]
                        and e["engineMicros"] <= sent_at]
            if statuses:
                row["latestReceivedStatusBeatZeroBased"] = statuses[-1]["input"]["beat"] - 1
                row["latestStatusAgeMilliseconds"] = round((sent_at - statuses[-1]["engineMicros"]) / 1000, 3)
        else:
            row["dispatch"] = "not present (pending, cancelled, or outside retained history)"
        sends.append(row)
    report["autoloopSends"] = sends
    report["operations"] = [e for e in events if e["stage"] == "operation"]
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("recording", type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(json.loads(args.recording.read_text())), indent=2))
