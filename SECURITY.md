# Security policy

## Reporting a vulnerability

Please do not open a public issue for a suspected security vulnerability.
Contact the maintainer privately through the security reporting option on the
GitHub repository. Include the affected Lumi version, impact and a minimal way
to reproduce the problem. Do not send music libraries, rekordbox databases,
SoundSwitch projects, credentials or unrelated personal data.

## Supported versions

Security fixes target the latest production release. Development and release
candidate builds are test channels and may require upgrading to a newer build.

Lumi runs locally, but it processes removable media, local files, MIDI and LAN
traffic. Only use releases and checksums published by the official Lumi GitHub
repository, and use the Pro DJ Link simulator only on a trusted development LAN.

## Dependency maintenance and release boundaries

Apply and test dependency fixes on `dev`. Dependabot version updates already
target `dev`; GitHub security updates may instead target the default branch.
For those alerts, incorporate the fix into `dev` and ship it through the normal
`dev` or `hotfix/*` release path. Do not bypass the main-branch guard to merge a
bot branch, and do not dismiss an alert merely because a fix exists on `dev`.
The published application remains affected until a reviewed release includes
the fix. Release publication and production installation require explicit approval.

The Security workflow audits `dev` and `main` separately, with independent Rust,
Maven and license jobs. One failed check must not hide the other results. Run
`./scripts/verify-security.sh` locally for all gates, or pass `rust`, `maven` or
`licenses` for one gate. Scheduled workflow changes only become active after
they reach the default branch; before then dispatch the workflow on `dev`.

SQLCipher's embedded SQLite version is checked by a runtime regression test in
`lumi-library-sqlite`. A clean RustSec report alone is insufficient: the
SQLCipher feature can bundle different SQLite sources from plain libsqlite3-sys.
