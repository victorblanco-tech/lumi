# ADR 0045 Automatic network media resolution

Status: Direction accepted on 2026-10-04. Production implementation depends on
the physical NFS proof of concept in E10-09. That fixed-file gate passed for
CHRM and GRAY on both Players, including a safe physical swap. The isolated
identity worker and local authorization registry are implemented in 0.6.4-dev-1.
Basic native identity acceptance passed: after local enrollment, GRAY on Player 1
and CHRM on Player 2 were simultaneously identified without restarting Lumi.
Source-scoped runtime hydration and mount invalidation are implemented in
0.6.4-dev-4. Local regression evidence is separate from the remaining physical
cross-Player and lighting acceptance.

## Context

The USB review reproduced successful imports that could not be resolved on live
Players. The current lookup discards the source Player and slot and searches all
USB aliases by Rekordbox ID. That ID is not a global track identity. The owner
prefers automatic source identification rather than assigning USBs to Players.

Deep Symmetry documents NFS access to files on player-mounted media and provides
Crate Digger's FileFetcher. This establishes a possible transport, not proof that
the CDJ-1500X exposes Lumi's own root-level marker on OneLibrary media. Beat Link
MediaDetails exposes a name, creation date, capacity and counts; these fields are
useful diagnostics but cannot uniquely distinguish independent equal-model USBs.

## Decision

First test read-only retrieval of the existing `.lumi-media.json` from a physical
Player. Do not enable MetadataFinder, SignatureFinder, new dbserver queries or a
second VirtualCdj merely to perform this test. Keep the accepted timing bridge,
Ableton Link tempo relay and AutoLoop executor unchanged during the POC.
Test actual CDJs before adding simulator media behavior; simulator changes must
follow captured hardware observations rather than define the expected protocol.

If the hardware gate passes, build a supervised, low-priority media resolver.
Its inputs are discovered device addresses and the source Player/slot reported
by the current bridge. NFS reads run outside the show pump and UI thread, with a
hard total deadline, bounded bytes, bounded concurrency and retry backoff. The
show never waits for a file or database read.

The resolution chain is source Player and slot, current media generation,
validated marker, existing trusted Lumi source, source-scoped track alias, then
the compatible library analysis and Lumi phrase timeline. A Player loading from
another Player uses the source Player's slot, not its own USB slot.

The owner revised the lifecycle on 2026-10-06: track source and current USB mount
are independent identities. Passive CDJ USB-status transitions drive mount
resolution; a healthy unchanged mount has no periodic fixed-file reads. First
discovery, insertion and recovery after a status interruption verify the mount.
Failed initial reads retry with bounded backoff, but successful reads stop.

Eject, removal and replacement cancel pending mount reads and advance the mount
epoch. They do not revoke an already verified load: modern Players may continue
cached playback after removal, including tracks loaded over LINK. Each verified
load retains its source, prepared analysis and phrase plan until that load is
actually replaced or unloaded. A new unresolved load cannot adopt a subsequent
mount's identity. Delayed worker replies carry their original epoch and cannot
authorize the newly inserted USB.

USB transitions use the critical bridge lane so an eject/insertion pair cannot
collapse into only its final state. A gap longer than three seconds between CDJ
status observations invalidates current mount continuity and requests fresh
verification when status resumes. This is not proof that every swap can be
observed: a short unobserved removal/reinsertion remains a hardware acceptance
case, especially if the Player also reuses the same loaded-track numeric ID.

## Trust and failure boundaries

The marker remains the versioned identity record from ADR 0043. It is identity
evidence, not authentication, proof of sync freshness or authority to register a
new trusted source. Only known sources may bind automatically. NFS is not an
authenticated channel; copied or conflicting markers must remain ambiguous.

Allow only the fixed identity-file path in the POC. Reject malformed, oversized
or unsupported markers. The public FileFetcher API does not advertise a maximum
download size or hard total deadline, so those limits need an independently
verified supervisor or bounded reader; a check after a complete fetch is not a
sufficient download bound. No arbitrary path supplied by a marker is followed.

Do not write through NFS, modify Rekordbox-owned files, claim a real Player's
number, change its transport, or repeatedly retrieve its export database. A
failed resolver does not restart Pro DJ Link, stop Ableton Link or disturb the
current AutoLoop. Missing readiness is a per-track/source state, not a network
failure. A last-known plan may continue only for the same verified track; a
colliding numeric ID is not sufficient evidence.

If the root marker is not readable, stop at that hardware gate and report the
result. A different marker location or a database-based identity strategy needs
its own explicit decision and compatibility evidence. Do not silently fall back
to media names, title matching or a global numeric-ID lookup.

## Related library changes

E10-08 also owns non-fatal handling of tracks without source phrases, library
refresh notifications for already loaded Players and durable audio fingerprints
independent of current USB aliases. Prepare those results off the show pump.
Changed prepared data is adopted under a documented safe policy, not by replacing
the active plan midway through a phrase or replaying an executed AutoLoop.

## Phase 2 implementation boundaries

One bounded worker owns NFS reads and a read-only SQLite connection. The engine
pump sends at most one job and consumes at most four replies per poll. Device
loss/reconnect cancels the old job; generation checks discard its delayed reply.
Before Dev-9, successful identities were revalidated every 15 seconds. The
event-driven refactor removes that timer; failed unresolved reads back off to
30 seconds. A failed reader changes only that Player's USB diagnostic detail.
For an already verified medium, unsuccessful rechecks retain the source ID,
name, native color and media epoch. A timeout is not evidence of replacement
and must not invalidate an existing track binding or AutoLoop deadline. Retries
continue in the background for unresolved mounts. Device loss, explicit
unknown/conflicting identity and a confirmed different marker revoke only
authorization for future loads from that mount. Existing verified cached loads
retain their own bindings while their transport remains authoritative.

New track loads may reuse the verified identity of an unchanged mount. They
capture its epoch before asynchronous preparation; an intervening removal or
replacement cannot authorize that load from a different mount. Cross-source
fallback remains outside this change.

Schema 19 records an existing local trusted source, marker UUID/source key and
physical fingerprint. Local scan/sync creates this binding; a network reply
cannot. Legacy canonical source IDs are retained instead of migrating track or
phrase ownership. Conflicting physical bindings and duplicate trusted markers
remain conflicts. The physical fingerprint includes the label, so a rename may
require explicit local reauthorization; automatic rename migration is not claimed.

The resolver calls the reader directly with an eight-second process supervisor
and bounded response, avoiding nested workers and orphan processes. The standalone
exact-SHA POC mode remains available separately. Both use the same fixed-file
bounded RPC transport. Marker identity is not cryptographic authentication.

Phase 2 added diagnostics only. It did not replace the old runtime track
lookup yet. Phase 3 must carry source Player/slot and track-load identity, detect
mount changes and reject stale bindings before source-scoped hydration. A cached
15-second identity is not by itself enough to match a newly loaded track safely.

Native acceptance used the existing markers without modifying either USB. Both
local authorizations remain distinct and non-conflicted; no Sync action was
needed. The real-Player check kept device discovery and position authority READY.
It does not yet prove safe source-scoped runtime matching across media swaps or
tracks loaded from another Player.

## Phase 3 preparation and adoption policy

A newly loaded USB track captures its source Player's current mount epoch and
uses its verified identity without another read when mount continuity is intact.
Unresolved identity is prepared independently. Mount changes cancel stale
replies. Matching requires `USB_SLOT`, a locally trusted source, the captured
epoch, the exact Rekordbox ID and the current track-load ID.
No title, color, numeric-ID-only or cross-source fallback is used automatically.
An observed medium change affects future source authorization, not verified
cached loads on this or another Player. Their immutable load binding also owns
the source name and native color displayed in the track header. Current mount
state is shown separately as identified, empty or ejecting. No mount event
restarts the bridge or tempo relay, seeks a track or replays an AutoLoop.

The separate `lumi-live-library` thread owns a bounded read-only SQLite connection.
Alias, track analysis, phrase timeline and mapping catalog are read within one
consistent snapshot. It never checks removable audio paths or modifies the
database. The show pump polls bounded queues without waiting for that reader.
Connection-local `data_version` detects commits from the separate USB worker;
unresolved loaded tracks are reconsidered once per second without a new load.

Metadata hydration preserves physical playstatus, pitch-adjusted BPM, position,
load ID and cue history. It is not a seek or a musical boundary. A missing alias,
missing phrase timeline or unavailable library remains a per-track preparation
state, not a Pro DJ Link failure. The next authoritative position activates any
newly prepared phrase plan through the existing output path.

Prepared data is retained for the current load if a later sync changes analysis,
phrases, metadata or catalog revision. The Mac shows a pending update on the
existing USB row; a real track reload adopts the new preparation. A refresh alone
does not replay a cue or change beat coordinates. Pending adoption must not be
interpreted as accepting the incoming edition or automatically rebasing authored
Lumi phrases.

An offline verified USB is represented to the editor and Local Playback with an
explicit non-file `lumi-unavailable` audio URI. This preserves the existing
nonempty-URI contract and access to stored waveform/phrases, without playing demo
audio or accepting the canonical track's old mutable path as a substitute.

Schema 20 retains full audio fingerprints per canonical track independently of
the current USB aliases. Migration backfills fingerprints still available in
schema 19; previously overwritten fingerprints cannot be reconstructed. Playback
uses only a current non-archived alias associated with that canonical identity.
This proves compatibility at the last verified sync, not a fresh full-file hash
on every playback. Legacy tracks without full fingerprints retain their previous
path selection until a verified sync supplies that evidence.

Remote static publication includes USB resolution state, verified name, native
color and media epoch. A media-only change therefore reaches the iPhone without
waiting for a different track or lighting plan. No internal USB ID is exposed.

## Evidence required

Mac Live and Lumi Remote show the mounted USB on its owning Player, even before
a track is loaded. Its display color is the native media color from passive
Beat Link `MediaDetails`, not a track color and not a matching key. The color
observation uses the replaceable display lane and performs no network requests.
Unknown/conflicted identities never display a cached trusted name or color.
Unavailable native colors are shown neutrally, without guessing from a label.

Remote receives only a bounded optional list of Player numbers, state, verified
source name and native color ID. Internal IDs, addresses and resolver details
remain on the Mac. This is presentation, not Phase 3 source-scoped hydration.

Unit and process tests cover identity validation, collisions, stale results,
deadlines, failure isolation and source-scoped lookup. Native UI tests cover
macOS and Remote presentation without layout jumps. Physical tests must prove
marker access, two independent USBs, loading over Link, swaps and reconnects.
Measure timing with and without resolver activity; do not claim that simulation
proves CDJ filesystem support or physical lighting timing.

## Mounted media and loaded track origin

Mac Live and Lumi Remote show two independent facts. The Player identity card
shows its physically mounted USB. The track header shows the source Player and
slot reported for the current track load, with the resolved USB name and native
media color. Player 2 may therefore show mounted GRAY while the track shows
CHRM via Player 1 / LINK. A load from that Player's own USB is labelled local.

The optional `trackSource` projection is scoped to the current load and contains
only source Player number, slot, resolution state, verified display name and
native color. It does not expose marker IDs, paths or addresses. An unresolved
source is displayed as unidentified; the destination's mounted USB never supplies
a substitute name. Local Playback does not show a Pro DJ Link media origin.

Presentation reads only the existing in-memory identity and resolver state.
The existing load and media publication keys deliver origin changes to Remote;
transport anchors retain this data without reconstructing it. No additional
polling, NFS requests, SQLite reads, tempo corrections or AutoLoop commands are
introduced. Source rows reserve their height while identity resolves.

## Cached LINK ownership on CDJ-1500X

Two owner-run status captures on 2026-10-06 establish an exception to treating
every native source-Player change as a new load. In the CDJ-1500X firmware 1.10,
1152-byte subtype-8 layout, ejecting Player 1's USB changes Player 2's legacy
source field from Player 1 to Player 2 while its loaded ID and extended block
remain unchanged. The observed byte at `0x125` changes from zero to four.
Neither the source field alone nor the numeric track ID alone identifies a
new load in this state.

The cached-origin guard retains an already observed LINK origin only for this exact model,
firmware, declared wire length and subtype, with a local USB/Rekordbox ownership report,
the same numeric ID, an identical bounded opaque block at `0x170..0x1af`,
matching extended ID and the observed `0x125 == 4` state. A native loading
observation resets origin before tempo filtering; unload, device rediscovery,
changed ID or block, another remote source and unsupported layouts do not
use this exception. Mount events remain independent and perform no additional
reads. Timing, tempo, position and MIDI facts are not modified.

Dev-11's full-length-only guard failed physical acceptance because Beat Link
8.0's `VirtualCdj.createStatusReceiver` allocates a 512-byte datagram buffer.
The 1152-byte native status is truncated before Lumi's update listener receives
it. Dev-12 accepts either the full 1152-byte form or that 512-byte prefix only
when the header declares 1152 bytes (`0x22..0x23` plus `0x24`) and all other
model, firmware and subtype gates agree. Every field used by the guard lies
within the received prefix. The pinned library and its receive buffer remain
unchanged; a loopback UDP regression exercises the actual truncation boundary.

The opaque block is continuity evidence for an existing load, not a documented
content hash, a globally unique identity or authorization for a new track.
It remained equal even after the owner unloaded and reloaded the same track
between captures, so it is not a unique load-instance token either. Its unknown
fields are not interpreted. An unobserved reload with completely
identical evidence remains a hardware acceptance limitation; this exception
does not prove general collision resistance or source recovery after restarting
Lumi with an already detached cached track. Physical eject and subsequent
fresh-load acceptance are required before show-readiness sign-off.

## References

- [Crate Digger FileFetcher](https://deepsymmetry.org/cratedigger/apidocs/org/deepsymmetry/cratedigger/FileFetcher.html)
- [DJ Link database export access](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Beat Link MediaDetails](https://deepsymmetry.org/beatlink/apidocs/org/deepsymmetry/beatlink/MediaDetails.html)
- [ADR 0043 USB identity and verified sync](0043-usb-identity-and-verified-sync-boundaries.md)
- [E10-09 Network media resolution](../../planning/story-e10-09-network-media-resolution.md)
