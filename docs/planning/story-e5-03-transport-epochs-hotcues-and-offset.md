# Story E5-03: Explicit playback epochs, hotcues and output offset

- Status: **Refined; waiting for E5-02**
- Priority: **P0 Critical**
- Target: `0.4.0-dev-52`
- GitHub tracking: [#119](https://github.com/victorblanco-tech/lumi/issues/119)

## Outcome

Start during playback, pause/resume, master handoff, hotcue, seek and beatjump
select the correct landing cue exactly once and in beat, without affecting the
independent Link timeline beyond a confirmed transport boundary.

## Scope and acceptance

- create a new playback epoch only for a confirmed user-visible transport
  boundary;
- cancel only not-yet-emitted cue work from the previous epoch;
- reselect the current cue once on Start/resume when required;
- land hotcue/seek/beatjump on the confirmed phrase and eligible beat;
- add negative/positive offset as a replaceable future deadline;
- allow BPM changes to move an unsent deadline, then freeze it after emission;
- run a deterministic 100-action matrix with zero wrong or duplicate cues.
