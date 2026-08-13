#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
channel="${1:-dev}"
release_directory="${2:-$repository_root/build/Releases}"
canonical_version="$(tr -d '[:space:]' < "$repository_root/VERSION")"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

case "$channel" in
  dev)
    build_configuration="Dev"
    app_name="Lumi"
    expected_display_name="Lumi Dev"
    expected_bundle_identifier="co.victorblan.tech.lumi.dev"
    expected_data_directory="Lumi Dev"
    expected_version_pattern='^[0-9]+\.[0-9]+\.[0-9]+-dev-[1-9][0-9]*$'
    install_directory="/Applications/Lumi/Dev"
    install_shortcut_name="Applications - Lumi - Dev"
    ;;
  rc)
    build_configuration="RC"
    app_name="Lumi"
    expected_display_name="Lumi RC"
    expected_bundle_identifier="co.victorblan.tech.lumi.rc"
    expected_data_directory="Lumi RC"
    expected_version_pattern='^[0-9]+\.[0-9]+\.[0-9]+-rc-[1-9][0-9]*$'
    install_directory="/Applications/Lumi/RC"
    install_shortcut_name="Applications - Lumi - RC"
    ;;
  release)
    build_configuration="Release"
    app_name="Lumi"
    expected_display_name="Lumi"
    expected_bundle_identifier="co.victorblan.tech.lumi"
    expected_data_directory="Lumi"
    expected_version_pattern='^[0-9]+\.[0-9]+\.[0-9]+$'
    install_directory="/Applications/Lumi"
    install_shortcut_name="Applications - Lumi"
    ;;
  *)
    echo "Usage: $0 [dev|rc|release] [output-directory]" >&2
    exit 64
    ;;
esac

derived_data="$repository_root/build/${build_configuration}DerivedData"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: local macOS packaging requires macOS." >&2
  exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "ERROR: the first Lumi package must be built on Apple Silicon." >&2
  exit 1
fi

if [[ ! "$canonical_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "ERROR: VERSION '$canonical_version' is not valid SemVer." >&2
  exit 1
fi
if [[ ! "$canonical_version" =~ $expected_version_pattern ]]; then
  echo "ERROR: '$channel' packaging does not accept version '$canonical_version'." >&2
  exit 1
fi

build_number="$(git -C "$repository_root" rev-list --count HEAD)"
source_revision="$(git -C "$repository_root" rev-parse --short=12 HEAD)"
if [[ ! "$build_number" =~ ^[1-9][0-9]*$ ]]; then
  echo "ERROR: could not derive a monotone numeric build number." >&2
  exit 1
fi

if [[ "${LUMI_ALLOW_DIRTY_PACKAGE:-0}" != "1" ]] \
  && [[ -n "$(git -C "$repository_root" status --porcelain --untracked-files=no)" ]]; then
  echo "ERROR: packaging requires a clean tracked worktree." >&2
  echo "Commit the release input or set LUMI_ALLOW_DIRTY_PACKAGE=1 for a disposable local build." >&2
  exit 1
fi

"$script_dir/check-environment.sh"
"$script_dir/check-version.sh"
"$script_dir/check-structure.sh"
"$script_dir/check-architecture.sh"

mkdir -p "$release_directory"

xcodebuild \
  -project "$repository_root/apps/macos/Lumi.xcodeproj" \
  -scheme Lumi \
  -configuration "$build_configuration" \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$derived_data" \
  CODE_SIGNING_ALLOWED=NO \
  CURRENT_PROJECT_VERSION="$build_number" \
  GCC_TREAT_WARNINGS_AS_ERRORS=YES \
  -quiet \
  build

source_app="$derived_data/Build/Products/$build_configuration/$app_name.app"
if [[ ! -d "$source_app" ]]; then
  echo "ERROR: $build_configuration build did not produce $app_name.app." >&2
  exit 1
fi

temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/lumi-package.XXXXXX")"
mount_directory="$temporary_root/mount"
mounted=0

cleanup() {
  if [[ "$mounted" == "1" ]]; then
    hdiutil detach "$mount_directory" -quiet || true
  fi
  rm -rf "$temporary_root"
}
trap cleanup EXIT

staging_directory="$temporary_root/Lumi-$canonical_version"
case "$channel" in
  dev)
    packaged_bundle_name="Lumi Dev $canonical_version.app"
    ;;
  rc)
    packaged_bundle_name="Lumi RC $canonical_version.app"
    ;;
  release)
    packaged_bundle_name="Lumi.app"
    ;;
esac
packaged_app="$staging_directory/$packaged_bundle_name"
packaged_helper="$packaged_app/Contents/Helpers/lumi-engine"
packaged_prolink_bridge="$packaged_app/Contents/Resources/prolink/lumi-prolink-bridge.jar"
packaged_prolink_java="$packaged_app/Contents/Resources/prolink-runtime/bin/java"
packaged_link_helper="$packaged_app/Contents/Resources/link/Carabiner"
mkdir -p "$staging_directory"
ditto "$source_app" "$packaged_app"

if [[ ! -x "$packaged_helper" ]]; then
  echo "ERROR: packaged app does not contain an executable lumi-engine helper." >&2
  exit 1
fi
if [[ ! -f "$packaged_prolink_bridge" || ! -x "$packaged_prolink_java" ]]; then
  echo "ERROR: packaged app does not contain the self-contained Pro DJ Link bridge." >&2
  exit 1
fi
if [[ ! -x "$packaged_link_helper" ]]; then
  echo "ERROR: packaged app does not contain the managed Ableton Link helper." >&2
  exit 1
fi

if ! file "$packaged_app/Contents/MacOS/$app_name" | grep -q 'arm64'; then
  echo "ERROR: packaged $app_name executable is not Apple Silicon arm64." >&2
  exit 1
fi
if ! file "$packaged_helper" | grep -q 'arm64'; then
  echo "ERROR: packaged lumi-engine helper is not Apple Silicon arm64." >&2
  exit 1
fi

# Xcode 26 emits the Dev executable together with Lumi.debug.dylib and
# __preview.dylib. Re-sign every nested Mach-O member with the same ad-hoc
# identity before sealing the app. A hardened-runtime ad-hoc app cannot load
# those separately signed libraries because ad-hoc signatures have no stable
# Team ID, so unsigned local packages deliberately omit `--options runtime`.
while IFS= read -r packaged_file; do
  if file "$packaged_file" | grep -q 'Mach-O'; then
    codesign --force --sign - --timestamp=none "$packaged_file"
  fi
done < <(find "$packaged_app/Contents" -type f -print)
codesign --force --sign - --timestamp=none "$packaged_app"
codesign --verify --deep --strict --verbose=2 "$packaged_app"

main_signature="$(codesign -dvvv "$packaged_app/Contents/MacOS/$app_name" 2>&1)"
if grep -q 'flags=.*runtime' <<< "$main_signature"; then
  echo "ERROR: ad-hoc packaged executable unexpectedly retains hardened runtime." >&2
  exit 1
fi

packaged_info_plist="$packaged_app/Contents/Info.plist"
packaged_version="$(/usr/libexec/PlistBuddy -c 'Print :LumiProductVersion' "$packaged_info_plist")"
packaged_marketing_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$packaged_info_plist")"
packaged_build_number="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$packaged_info_plist")"
packaged_bundle_identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$packaged_info_plist")"
packaged_display_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleDisplayName' "$packaged_info_plist")"
packaged_channel="$(/usr/libexec/PlistBuddy -c 'Print :LumiReleaseChannel' "$packaged_info_plist")"
packaged_data_directory="$(/usr/libexec/PlistBuddy -c 'Print :LumiDataDirectoryName' "$packaged_info_plist")"
expected_marketing_version="${canonical_version%%-*}"

if [[ "$packaged_version" != "$canonical_version" ]]; then
  echo "ERROR: packaged version '$packaged_version' differs from '$canonical_version'." >&2
  exit 1
fi
if [[ "$packaged_marketing_version" != "$expected_marketing_version" ]]; then
  echo "ERROR: packaged marketing version '$packaged_marketing_version' differs from '$expected_marketing_version'." >&2
  exit 1
fi
if [[ "$packaged_build_number" != "$build_number" ]]; then
  echo "ERROR: packaged build '$packaged_build_number' differs from '$build_number'." >&2
  exit 1
fi
if [[ "$packaged_bundle_identifier" != "$expected_bundle_identifier" ]]; then
  echo "ERROR: packaged bundle '$packaged_bundle_identifier' differs from '$expected_bundle_identifier'." >&2
  exit 1
fi
if [[ "$packaged_display_name" != "$expected_display_name" ]]; then
  echo "ERROR: packaged display name '$packaged_display_name' differs from '$expected_display_name'." >&2
  exit 1
fi
if [[ "$packaged_channel" != "$channel" ]]; then
  echo "ERROR: packaged channel '$packaged_channel' differs from '$channel'." >&2
  exit 1
fi
if [[ "$packaged_data_directory" != "$expected_data_directory" ]]; then
  echo "ERROR: packaged data directory '$packaged_data_directory' differs from '$expected_data_directory'." >&2
  exit 1
fi

ln -s "$install_directory" "$staging_directory/$install_shortcut_name"
cp "$repository_root/docs/release/unsigned-macos-installation.txt" \
  "$staging_directory/README - Install $app_name.txt"
cp "$repository_root/LICENSE" "$staging_directory/LICENSE.txt"
cp "$repository_root/TRADEMARKS.md" "$staging_directory/TRADEMARKS.md"
cp "$repository_root/THIRD_PARTY_NOTICES.md" \
  "$staging_directory/THIRD-PARTY-NOTICES.md"
{
  echo "Lumi source code"
  echo
  echo "Lumi is available under the Eclipse Public License 2.0."
  echo "Preferred source form: https://github.com/victorblanco-tech/lumi"
  echo
  echo "If that repository is private when you receive this build, request"
  echo "corresponding source access from the person who distributed it."
} > "$staging_directory/SOURCE-AND-LICENSE.txt"
{
  echo "$app_name $canonical_version"
  echo "Channel $channel"
  echo "Build $build_number"
  echo "Source revision $source_revision"
  echo "Architecture arm64"
  echo "Signing ad hoc (not Developer ID / notarized)"
} > "$staging_directory/BUILD-INFO.txt"

artifact_name="Lumi-$canonical_version-arm64.dmg"
temporary_dmg="$temporary_root/$artifact_name"
final_dmg="$release_directory/$artifact_name"
checksum_file="$final_dmg.sha256"

hdiutil create \
  -volname "$app_name $canonical_version" \
  -srcfolder "$staging_directory" \
  -format UDZO \
  -ov \
  "$temporary_dmg" \
  -quiet
hdiutil verify "$temporary_dmg" -quiet

mkdir -p "$mount_directory"
hdiutil attach "$temporary_dmg" \
  -nobrowse \
  -readonly \
  -mountpoint "$mount_directory" \
  -quiet
mounted=1
codesign --verify --deep --strict --verbose=2 "$mount_directory/$packaged_bundle_name"
test -x "$mount_directory/$packaged_bundle_name/Contents/Helpers/lumi-engine"
if [[ "$(readlink "$mount_directory/$install_shortcut_name")" != "$install_directory" ]]; then
  echo "ERROR: packaged install shortcut does not target '$install_directory'." >&2
  exit 1
fi
hdiutil detach "$mount_directory" -quiet
mounted=0

mv "$temporary_dmg" "$final_dmg"
(
  cd "$release_directory"
  shasum -a 256 "$artifact_name" > "$(basename "$checksum_file")"
)

echo "Local macOS package passed:"
echo "  $final_dmg"
echo "  $checksum_file"
echo "Version: $canonical_version"
echo "Channel: $channel"
echo "Build: $build_number"
echo "Revision: $source_revision"
echo "Signing: ad hoc (not Developer ID / notarized)"
