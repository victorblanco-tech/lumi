# Lumi development environment

Lumi's first build target is Apple Silicon with a macOS 15.0 deployment
minimum. Product code uses Rust 2024 and Swift 6 strict concurrency.

## Required tools

- Apple Silicon Mac;
- latest validated stable full Xcode, with macOS and iOS support;
- Rust installed through rustup;
- Git and GitHub CLI for repository collaboration.

Beta Xcode, additional Apple platform runtimes, Node.js, Docker, Java and
Homebrew are not prerequisites for Epic 1.

## Initial setup

After installing Xcode, open it once and complete its first-launch setup. Then
select it as the active developer directory:

```bash
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
sudo xcodebuild -runFirstLaunch
```

Install Rust using the official rustup installer and keep the default stable
profile. The repository's `rust-toolchain.toml` selects the exact validated
toolchain and required components.

If a newly opened non-interactive shell cannot find Rust, either start a new
shell or load Cargo's environment configuration. Lumi's repository scripts also
resolve the standard rustup installation directory explicitly.

## Verify

The environment check is read-only and safe to repeat:

```bash
./scripts/check-environment.sh
```

To include GitHub authentication in the check:

```bash
LUMI_CHECK_GITHUB_AUTH=1 ./scripts/check-environment.sh
```

Run all local foundation checks and builds with:

```bash
./scripts/verify.sh
```

After a successful build, launch the exact unsigned development app with:

```bash
open build/DerivedData/Build/Products/Debug/Lumi.app
```

The status becomes **Local engine ready** only after the app starts the bundled
Rust helper, authenticates over loopback, and receives its initial snapshot.

Generated Xcode build data is written below the ignored root `build/`
directory rather than mixed with source files.

Repository ownership, dependency direction, and naming rules are documented in
[`repository-structure.md`](repository-structure.md).

The app-scoped Rust process, loopback authentication, and shutdown behavior are
documented in [`local-engine-session.md`](local-engine-session.md).

## Troubleshooting

- `xcodebuild` points at Command Line Tools: rerun `xcode-select --switch`.
- Xcode first-launch is incomplete: open Xcode or run
  `sudo xcodebuild -runFirstLaunch`.
- `rustc` is missing: install Rust through rustup and open a new shell.
- GitHub authentication expired: run `gh auth refresh -h github.com`.
- Simulator discovery fails inside a restricted shell: verify it from Terminal
  with `xcrun simctl list runtimes`.
