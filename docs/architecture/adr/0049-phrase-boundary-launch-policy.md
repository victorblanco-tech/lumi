# ADR 0049 — optional phrase-boundary launch

Date: 2026-10-09
Status: accepted design; implementation and acceptance pending (E11-05).

## Decision

Keep Immediate as the default. Add an explicit On phrase start policy for the
initial launch of an armed show. This is an output gate owned by the engine,
not a playback command, a UI timer or a new Ableton Link synchronization mode.
Both clients display the same authoritative pending target and remaining beats.

Arm prepares the available plan without transmitting a new AutoLoop. With the
optional policy selected, Start while paused waits for playback. Playback at a
phrase boundary admits that phrase immediately. Playback inside a phrase waits
for the next usable boundary. The operator can cue a few beats before it to
provide enough time for the existing output offset and bank preparation.

Resolve the target from the current verified track's exact grid and executable
plan. Do not infer a boundary from rounded UI position. If there is no next
usable boundary, show that explicitly and offer a deliberate immediate launch;
never wait indefinitely without explanation or silently change the policy.

## State and cancellation

Track Off, Armed, Waiting for playback, Waiting for boundary, Launched and No
upcoming boundary separately. Bind pending targets to deck, track-load, plan
revision and transport epoch. Pause/Off cancels unsent output. Before launch,
seek, track load, master handover or plan replacement invalidates the target and
requires resolution against the new authoritative state. A pitch change updates
an unsent deadline using the existing scheduler, not a second competing timer.

After the first accepted launch, normal phrase transitions and hotcues use the
existing sparse AutoLoop executor. They do not re-enter the initial run-in gate.
Deduplication must cover the early scheduled send and the eventual boundary
observation so one boundary cannot produce two button presses. A late deadline
is recorded as late; it must not be reported as an on-time launch.

## Isolation and persistence

Persist the selected policy per Lumi channel outside the timing pump. Reject
policy changes during active output, or require an explicit return to Arm;
never retrospectively apply them to the playing AutoLoop. Extend the versioned
Remote projection and command capability contract before adding an iPhone
control. Older clients must not mistake a pending launch for an emitted cue.

Static Looks follow the same initial gate. Ableton Link remains tempo-only and
independent. Do not reset the global Link timeline, repeatedly retrigger an
AutoLoop, or add continuous phase correction. No reliable SoundSwitch playback
phase feedback is available in the current integration, so no automatic
"correct only if out of sync" feature can honestly be enabled yet.

## Acceptance

Use deterministic engine tests plus the real simulator packet path: paused
Arm/Start/Play, Start while playing, exact-boundary and mid-phrase cue, last
phrase, positive/negative offsets, bank preparation, pitch changes, rapid
Pause/Start, seek, master handover, unload and late plan arrival. Assert exactly
one intended dispatch, no stale dispatch and bounded cancellation. Exercise
Mac and Remote UI with the same pending state. Measure intended deadline to
MIDI emission separately from SoundSwitch acceptance and physical lighting.
Passing software tests is not proof of physical first-beat alignment.
