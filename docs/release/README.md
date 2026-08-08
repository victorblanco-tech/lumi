# Release- en deploymentmanagement

- [Release- en deploymentplan](release-and-deployment-plan.md)
- [Releasechecklist](release-checklist.md)
- [0.1.0 app demo and known limitations](0.1.0-demo-and-limitations.md)
- [Epic 1 – 0.1.0 release evidence](0.1.0-epic-1-evidence.md)
- [0.2.0 development demo and known limitations](0.2.0-demo-and-limitations.md)
- [Epic 2A – 0.2.0 development evidence](0.2.0-epic-2a-evidence.md)
- [0.3.0 installable release readiness](0.3.0-release-readiness.md)
- [0.3.0 development release and known limitations](0.3.0-dev-demo-and-limitations.md)
- [Unsigned macOS installation instructions](unsigned-macos-installation.txt)

Create a locally verified, unsigned Apple Silicon disk image with:

```bash
./scripts/package-macos-local.sh
```

The generated DMG and SHA-256 checksum are written to `build/Releases` and are
ignored by Git. This path intentionally uses no GitHub Actions minutes and no
paid Apple credentials.

De actuele productversie staat in [`VERSION`](../../VERSION). Git-tags gebruiken
dezelfde versie met een `v`-prefix, bijvoorbeeld `v0.1.0`.
