#!/usr/bin/env bash
set -euo pipefail

expected="${EXPECTED_MACOS_MAJOR:?Set EXPECTED_MACOS_MAJOR to 26 or 27}"
case "$expected" in 26|27) ;; *) echo "Unsupported compatibility target" >&2; exit 2 ;; esac
actual="$(sw_vers -productVersion)"
architecture="$(uname -m)"
xcode="$(xcodebuild -version)"
sdk="$(xcrun --sdk macosx --show-sdk-version)"
printf 'macOS: %s\nArchitecture: %s\n%s\nmacOS SDK: %s\n' "$actual" "$architecture" "$xcode" "$sdk"
[[ "${actual%%.*}" == "$expected" ]] || {
  echo "ERROR: runner is not macOS $expected; refusing misleading compatibility evidence." >&2
  exit 1
}
[[ "$architecture" == arm64 ]] || { echo "ERROR: Apple Silicon is required." >&2; exit 1; }
if [[ "$expected" == 27 ]]; then
  [[ "$xcode" == "Xcode 27"* && "${sdk%%.*}" == 27 ]] || {
    echo "ERROR: macOS 27 compatibility requires Xcode 27 and the macOS 27 SDK." >&2
    exit 1
  }
fi
# Compiler caches must not cross OS, Xcode build, or SDK versions.
toolchain="$(printf '%s\n' "$actual" "$xcode" "$sdk" | shasum -a 256 | cut -d ' ' -f 1)"
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  printf 'toolchain=%s\n' "$toolchain" >> "$GITHUB_OUTPUT"
fi
