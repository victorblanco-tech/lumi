# Dry-run lighting output

`lumi-lighting-output` owns the provider-neutral output port. It accepts only a
prevalidated `OutputExecutionRequest` containing the exact plan revision,
track-load instance, phrase cue, semantic action, and scheduled monotonic time.
It contains no MIDI, SoundSwitch, simulator, transport, or UI types.

`lumi-output-dry-run` is the Epic 1 adapter. It performs no external I/O and
records bounded normalized `simulated` results. Results re-enter the serialized
runtime as effect events and are exposed in snapshots for the native timeline.

The operational gate is checked twice: the reducer only schedules phrase cues
in `LIVE`, and the output worker revalidates operation state, leader, active
plan ID/revision, cue, and track-load identity immediately before the provider
call. A stale request is recorded as `skipped` with
`staleExecutionContext`; the provider is not called.

The canonical 64x demo transcript lives in
`fixtures/demo-session-v1/output-effects.json`. It proves that leader
activation plus four phrase boundaries execute the precomputed plan in stable
order. `ARMED` and `PAUSED` retain tracking but produce no provider records;
`OFF` clears the active plan without emitting a blackout action.
