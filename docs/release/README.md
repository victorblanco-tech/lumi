# Release and deployment

Start with the [production release checklist](release-checklist.md). It covers
the current GitHub-distributed, unsigned Apple Silicon release path as well as
the public-repository and GitHub Pages gates.

- [Release- en deploymentplan](release-and-deployment-plan.md)
- [Production release checklist](release-checklist.md)
- [Public Beta readiness](public-beta-readiness.md)
- [0.1.0 app demo and known limitations](0.1.0-demo-and-limitations.md)
- [Epic 1 – 0.1.0 release evidence](0.1.0-epic-1-evidence.md)
- [0.2.0 development demo and known limitations](0.2.0-demo-and-limitations.md)
- [Epic 2A – 0.2.0 development evidence](0.2.0-epic-2a-evidence.md)
- [0.3.0 installable release readiness](0.3.0-release-readiness.md)
- [0.3.0 development release and known limitations](0.3.0-dev-demo-and-limitations.md)
- [0.3.0 release candidate notes](0.3.0-rc.1-release-notes.md)
- [0.3.0 release notes](0.3.0-release-notes.md)
- [0.4.0 release notes](0.4.0-release-notes.md)
- [0.4.0 release readiness](0.4.0-release-readiness.md)
- [0.5.0 release notes](0.5.0-release-notes.md)
- [0.5.0 release readiness](0.5.0-release-readiness.md)
- [0.4.0-dev-50 isolated Ableton Link Relay](0.4.0-dev-50-isolated-link-relay.md)
- [0.4.0-dev-51 exactly-once AutoLoop output](0.4.0-dev-51-exactly-once-autoloop-output.md)
- [0.4.0-dev-52 SoundSwitch MIDI 1.0 compatibility](0.4.0-dev-52-soundswitch-midi-compatibility.md)
- [0.4.0-dev-53 representative Pro DJ Link simulator](0.4.0-dev-53-representative-prolink-simulator.md)
- [0.4.0-dev-54 isolated realtime integration lanes](0.4.0-dev-54-isolated-realtime-integration-lanes.md)
- [0.4.0-dev-55 deterministic transport epochs and output timing](0.4.0-dev-55-deterministic-transport-and-output-timing.md)
- [0.4.0-dev-56 bounded Live integration evidence](0.4.0-dev-56-bounded-live-integration-evidence.md)
- [0.4.0-dev-57 dynamic Pro DJ Link device recovery](0.4.0-dev-57-dynamic-prolink-device-recovery.md)
- [Unsigned macOS installation instructions](unsigned-macos-installation.txt)

Create a locally verified, unsigned Apple Silicon disk image with:

```bash
./scripts/package-macos-local.sh dev
```

The generated DMG and SHA-256 checksum are written to `build/Releases` and are
ignored by Git. This path intentionally uses no GitHub Actions minutes and no
paid Apple credentials. Its drag target is channel-specific: Production uses
`/Applications/Lumi`, RC uses `/Applications/Lumi/RC` and Dev uses
`/Applications/Lumi/Dev`; it never points prerelease apps at the Applications
root.

The only channels are `dev`, `rc` and `release`. Their versions are respectively
`X.Y.Z-dev-N`, `X.Y.Z-rc-N` and `X.Y.Z`; every app is named **Lumi**. Prerelease
DMGs use the version in the bundle filename so they remain distinguishable.

Before testing schema or migration work, create a checked release backup and—if
needed—seed an empty channel explicitly:

```bash
./scripts/backup-macos-user-data.sh
./scripts/clone-macos-channel-data.sh rc
./scripts/clone-macos-channel-data.sh dev
```

The clone commands refuse to overwrite an existing channel. Close every Lumi
app first; each copied database must pass SQLite integrity validation.

De actuele productversie staat in [`VERSION`](../../VERSION). Git-tags gebruiken
dezelfde versie met een `v`-prefix, bijvoorbeeld `v0.1.0`.
