#!/usr/bin/env bash
set -euo pipefail

# iOS Simulator reads entitlements embedded in the Mach-O __TEXT section.
# Checking only the source .entitlements file or codesign output misses a
# CODE_SIGNING_ALLOWED=NO build that launches but fails every Keychain request.
app="${1:?Usage: check-ios-simulator-entitlements.sh APP_PATH}"
info="$app/Info.plist"
bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$info")"
executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$info")"
entitlements="$(mktemp -t lumi-simulator-entitlements)"
trap 'rm -f "$entitlements"' EXIT

otool -arch "$(uname -m)" -X -s __TEXT __entitlements "$app/$executable_name" \
  | awk '/^[0-9a-fA-F]+[[:space:]]/ {
      for (i = 2; i <= NF; i++) {
        # otool renders either bytes or little-endian words, depending on the
        # selected Mach-O slice. iOS Simulator arm64/x86_64 are little-endian.
        for (j = length($i) - 1; j > 0; j -= 2) printf "%s", substr($i, j, 2)
      }
    }' \
  | xxd -r -p > "$entitlements"

plutil -lint "$entitlements" >/dev/null || {
  echo 'ERROR: the simulator app has no usable embedded entitlements; Keychain cannot work.' >&2
  exit 1
}
identifier="$(/usr/libexec/PlistBuddy -c 'Print :application-identifier' "$entitlements")"
access_group="$(/usr/libexec/PlistBuddy -c 'Print :keychain-access-groups:0' "$entitlements")"
if [[ "$identifier" != "$bundle_id" && "$identifier" != *".$bundle_id" ]]; then
  echo 'ERROR: the simulator application identifier does not match this app.' >&2
  exit 1
fi
if [[ "$identifier" != "$access_group" || "$identifier" == *'$('* ]]; then
  echo 'ERROR: simulator Keychain entitlements are missing, unresolved or not app-scoped.' >&2
  exit 1
fi
echo "Simulator app-scoped Keychain entitlements verified: $bundle_id"
