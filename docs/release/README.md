# Release- en deploymentmanagement

- [Release- en deploymentplan](release-and-deployment-plan.md)
- [Releasechecklist](release-checklist.md)
- [0.1.0 app demo and known limitations](0.1.0-demo-and-limitations.md)
- [Epic 1 – 0.1.0 release evidence](0.1.0-epic-1-evidence.md)
- [0.2.0 development demo and known limitations](0.2.0-demo-and-limitations.md)
- [Epic 2A – 0.2.0 development evidence](0.2.0-epic-2a-evidence.md)
- [0.3.0 installable release readiness](0.3.0-release-readiness.md)
- [0.3.0 development release and known limitations](0.3.0-dev-demo-and-limitations.md)
- [0.3.0 release candidate notes](0.3.0-rc.1-release-notes.md)
- [0.3.0 release notes](0.3.0-release-notes.md)
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
