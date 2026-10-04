# ADR 0045 Automatic network media resolution

Status: Direction accepted on 2026-10-04. Production implementation depends on
the physical NFS proof of concept in E10-09.

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

If the hardware gate passes, build a supervised, low-priority media resolver.
Its inputs are discovered device addresses and the source Player/slot reported
by the current bridge. NFS reads run outside the show pump and UI thread, with a
hard total deadline, bounded bytes, bounded concurrency and retry backoff. The
show never waits for a file or database read.

The resolution chain is source Player and slot, current media generation,
validated marker, existing trusted Lumi source, source-scoped track alias, then
the compatible library analysis and Lumi phrase timeline. A Player loading from
another Player uses the source Player's slot, not its own USB slot.

Invalidate results on device loss, media removal or replacement. Results carry
the requested device/media generation and track-load identity, so a delayed read
cannot attach the previous USB or previous track to the current Player. Design
mount evidence and periodic bounded revalidation before accepting a persistent
cache; do not assume that every physical swap produces an observed empty frame.

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

## Evidence required

Unit and process tests cover identity validation, collisions, stale results,
deadlines, failure isolation and source-scoped lookup. Native UI tests cover
macOS and Remote presentation without layout jumps. Physical tests must prove
marker access, two independent USBs, loading over Link, swaps and reconnects.
Measure timing with and without resolver activity; do not claim that simulation
proves CDJ filesystem support or physical lighting timing.

## References

- [Crate Digger FileFetcher](https://deepsymmetry.org/cratedigger/apidocs/org/deepsymmetry/cratedigger/FileFetcher.html)
- [DJ Link database export access](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Beat Link MediaDetails](https://deepsymmetry.org/beatlink/apidocs/org/deepsymmetry/beatlink/MediaDetails.html)
- [ADR 0043 USB identity and verified sync](0043-usb-identity-and-verified-sync-boundaries.md)
- [E10-09 Network media resolution](../../planning/story-e10-09-network-media-resolution.md)
