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

No gate silently turns a missing prerequisite into a pass. Ignored tests stay
ignored in ordinary gates and are selected explicitly by `lab`.

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

The bounded `lab` command is a development acceptance gate. Story E4-03B adds a
configurable soak runner; one hour is mandatory for RC evidence. Final physical
evidence uses CDJ-1500X, DJM-V5, SoundSwitch, Control One and visible DMX output
and is retained without copyrighted audio or user library data.
