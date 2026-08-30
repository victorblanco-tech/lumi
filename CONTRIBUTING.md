# Contributing to Lumi

Thank you for helping improve Lumi. User-facing fixes, hardware observations,
documentation and focused code changes are all welcome.

## Before opening a change

- Use [GitHub Issues](https://github.com/victorblanco-tech/lumi/issues) for a
  reproducible bug or a concrete feature proposal.
- Do not include music files, rekordbox databases, USB exports, SoundSwitch
  projects, private network addresses or access tokens.
- Keep show-critical behavior deterministic. UI, library and planning work must
  not enter the Pro DJ Link, Ableton Link or realtime MIDI execution lanes.

## Branches and pull requests

- `main` contains production releases.
- `dev` contains the next integrated development version.
- Open normal contributions against `dev`.
- Keep a pull request focused on one logical change.
- Describe user impact, risks and how the change was tested.
- Never push directly to `main`.

Maintainers may work directly on `dev` during active development. Production
releases are merged from `dev` to `main` and tagged separately.

## Commit messages

Use a short Conventional Commit message, for example:

```text
feat: add next-track theme override
fix: preserve output after live workspace navigation
docs: clarify SoundSwitch setup
test: cover hot-cue transport discontinuity
```

## Local verification

Run the repository verification command before opening a pull request:

```bash
./scripts/verify.sh
```

More detail is available in the [development guide](docs/development/README.md).
The release workflow is documented in
[docs/release](docs/release/release-and-deployment-plan.md).
