# Local quality gates

Lumi is developed local-first. The gates below separate ordinary deterministic
regression tests from tests that need network access, exclusive CoreMIDI names,
the LAN simulator or physical show hardware.

## Entry point

```bash
./scripts/verify-local.sh <gate>
```

| Gate | Purpose | Hardware/network |
|---|---|---|
| `functional` | Domain, planner, library, engine and native presentation behavior | No hardware; offline after dependencies are cached |
| `technical` | Formatting, lint, queue/fault recovery, Java adapters and release performance budgets | No hardware; offline after dependencies are cached |
| `full` | Existing complete repository, Xcode bundle and visual-evidence gate | No hardware; every installed Lumi app must be closed |
| `security` | RustSec and OSV audit of production Rust and Maven dependencies | Internet required; `cargo-audit`, `curl` and `jq` required |
| `lab` | Real network simulator plus optional real Carabiner/SoundSwitch peer | LAN simulator, synced disposable Dev DB and explicit environment |
| `soak` | Four isolated/combined configurable Live lanes plus retained evidence | LAN simulator, managed Carabiner helper, synced disposable Dev DB |

No gate silently turns a missing prerequisite into a pass. Ignored tests stay
ignored in ordinary gates and are selected explicitly by `lab`.

GitHub CI routes work by changed area. A push to `dev` gets documentation
validation, portable Rust verification, and/or a fast native Swift package gate
as applicable. A pull request to `main` keeps the complete affected-platform
verification, including the arm64 app bundle and visual evidence for application
changes. Release tags run the complete suite. Unknown paths deliberately fall
back to both full platform gates.

`Foundation gate` is the stable branch-protection result. It succeeds only when
classification and every affected child gate pass; a skipped irrelevant child
job can therefore never weaken protection. The Foundation workflow does not run
again after a merge to `main`, avoiding a duplicate full build.
A merge-commit synchronization back to `dev` with an identical repository tree
runs classification and the stable gate only.

`LumiEngineClient` is part of the fast native gate because it owns the bounded
desktop-to-service control plane. Dependency vulnerability and release-license
auditing run in a separate weekly/manual Linux workflow, never on an ordinary
`dev` push. Dependabot remains the early-update signal; the audit is the
independent evidence before a release.

Public CI is an independent confirmation; the hardware-aware local and lab
evidence remains mandatory for show-critical changes.

## Everyday workflow

Before or during a story:

```bash
./scripts/verify-local.sh functional
```

Before handing over a runtime/performance change:

```bash
./scripts/verify-local.sh technical
```

Before creating an RC candidate:

```bash
./scripts/verify-local.sh full
./scripts/verify-local.sh security
```

`full` refuses to start while an installed Lumi app or service is running. This
protects the deterministic tests from colliding with the fixed `Lumi Virtual
MIDI` and `Lumi Clock` endpoints. Stop Dev, RC and Prod; the script never kills
a show process automatically.

## Security prerequisites

Install the Rust audit tool once:

```bash
cargo install cargo-audit --locked
```

The security gate queries the public RustSec and OSV databases. This is a
development-only operation; Lumi does not need internet during normal use or a
show. A vulnerability may only be temporarily accepted through a reviewed,
versioned exception with rationale, affected boundary, compensating control,
owner and expiry. The initial gate intentionally has no blanket ignore list.

## LAN simulator gate

Use a disposable copy of the Dev database. Never point the gate at the only
copy of production data.

```bash
export LUMI_SIM_URL='http://simulator-host:17840'
export LUMI_SIM_TOKEN='temporary simulator token'
export LUMI_PROLINK_NETWORK_DATABASE='/absolute/path/to/disposable-lumi.sqlite3'
./scripts/verify-local.sh lab
```

The simulator must be broadcasting a USB-backed loaded track that resolves in
the supplied database. The gate validates idle-client timing and the exact
stopped/cued → play → Pause → Start AutoLoop sequence.

To include the real managed Link helper:

```bash
export LUMI_CARABINER_TEST_EXECUTABLE="$PWD/build/carabiner-runtime/Carabiner"
export LUMI_EXPECT_LINK_PEER=1 # only while SoundSwitch is open with Link enabled
./scripts/verify-local.sh lab
```

## Configurable Live integration soak

The soak gate uses the same simulator and database variables plus the managed
Carabiner executable. Development evidence requires at least 30 seconds per
lane; RC evidence requires at least one hour per lane.

```bash
export LUMI_LIVE_SOAK_SECONDS=60
export LUMI_CARABINER_TEST_EXECUTABLE="$PWD/build/carabiner-runtime/Carabiner"
./scripts/verify-local.sh soak

# RC gate
export LUMI_LIVE_SOAK_SECONDS=3600
export LUMI_REQUIRE_RC_DURATION=1
./scripts/verify-local.sh soak
```

The four sequential modes are Pro DJ Link-only, Link-only, realtime MIDI-only
and combined. The combined mode adds pitch, transport and UI-polling stress and
writes a credential-free JSON artifact below `build/Evidence` by default.

## Performance evidence

Every accepted show-path measurement records at least:

- fixture/version and commit;
- source mode and hardware/simulator identity;
- sample count and duration;
- p50, p95, p99 and maximum input-to-CoreMIDI error;
- missed, duplicate and stale-generation outputs;
- fallback-beat, queue high-water, coalesced and dropped counts;
- concurrent workload (idle, UI stress, USB sync, helper restart);
- failure/recovery events.

The release-critical normal pre-armed phrase target is p95 <= 20 ms at the
CoreMIDI boundary. A smooth waveform is useful but is not timing evidence.

## One-hour and physical gates

The bounded `lab` command is a development acceptance gate. The `soak` command
is the configurable duration runner; one hour per lane is mandatory for RC
evidence. Final physical
evidence uses CDJ-1500X, DJM-V5, SoundSwitch, Control One and visible DMX output
and is retained without copyrighted audio or user library data.
