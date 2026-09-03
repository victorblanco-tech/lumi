#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "Usage: $0 <app-helpers-directory>" >&2
  exit 64
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
helpers_directory="$1"
resources_directory="$(dirname "$helpers_directory")/Resources"
contents_directory="$(dirname "$helpers_directory")"
launch_agents_directory="$contents_directory/Library/LaunchAgents"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

# Package the engine as a per-user LaunchAgent. SMAppService registers this
# plist directly from the containing app bundle; no installer, root helper or
# writes to ~/Library/LaunchAgents are required. Keep every release channel
# isolated through its bundle identifier and Application Support directory.
for required_variable in PRODUCT_BUNDLE_IDENTIFIER LUMI_DATA_DIRECTORY_NAME LUMI_PRODUCT_VERSION LUMI_RELEASE_CHANNEL LUMI_SEED_DEMO_LIBRARY CURRENT_PROJECT_VERSION; do
  if [[ -z "${!required_variable:-}" ]]; then
    echo "ERROR: $required_variable is required to package the Lumi engine LaunchAgent." >&2
    exit 1
  fi
done

profile_directory="release"
service_target_directory="$repository_root/build/engine-service-target"
engine_info_plist_directory="$repository_root/build/generated"
engine_info_plist="$engine_info_plist_directory/lumi-engine-info.plist"
install -d "$engine_info_plist_directory"
plutil -create xml1 "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string $PRODUCT_BUNDLE_IDENTIFIER.engine" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleName string Lumi Engine" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string Lumi Engine" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleExecutable string lumi-engine" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundlePackageType string BNDL" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $LUMI_PRODUCT_VERSION" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleVersion string $CURRENT_PROJECT_VERSION" "$engine_info_plist"
/usr/libexec/PlistBuddy -c "Add :NSRemovableVolumesUsageDescription string Lumi reads trusted Rekordbox USB media to synchronize playlists, analysis, cue points, and playable track locations. Rekordbox and USB files are never changed; source identity is stored locally in Lumi." "$engine_info_plist"
plutil -lint "$engine_info_plist" >/dev/null

# A standalone LaunchAgent executable must carry an embedded Info.plist. The
# Service Management framework rejects an otherwise valid bundled job as
# `notFound` when this Mach-O section is absent.
engine_info_link_argument="-Wl,-sectcreate,__TEXT,__info_plist,$engine_info_plist"
cd "$repository_root"
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=$engine_info_link_argument" \
  cargo build --locked --release -p lumi-engine --target-dir "$service_target_directory"
install -d "$helpers_directory"
install -m 755 "$service_target_directory/$profile_directory/lumi-engine" "$helpers_directory/lumi-engine"
if ! otool -s __TEXT __info_plist "$helpers_directory/lumi-engine" >/dev/null 2>&1; then
  echo "ERROR: lumi-engine is missing its embedded LaunchAgent Info.plist." >&2
  exit 1
fi

# The iPhone Remote gateway has an independent executable identity and launchd
# lifecycle. It is packaged with Lumi but registered only after the user turns
# iPhone Remote on in Integrations.
gateway_target_directory="$repository_root/build/remote-gateway-service-target"
gateway_info_plist="$engine_info_plist_directory/lumi-remote-gateway-info.plist"
plutil -create xml1 "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string $PRODUCT_BUNDLE_IDENTIFIER.remote-gateway" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleName string Lumi Remote Gateway" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string Lumi Remote Gateway" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleExecutable string lumi-remote-gateway" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundlePackageType string BNDL" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $LUMI_PRODUCT_VERSION" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleVersion string $CURRENT_PROJECT_VERSION" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :NSLocalNetworkUsageDescription string Lumi Remote securely connects a paired iPhone to this Mac over the local network." "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :NSBonjourServices array" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :NSBonjourServices:0 string _lumi-remote._tcp" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :NSBonjourServices:1 string _lumi-remote-rc._tcp" "$gateway_info_plist"
/usr/libexec/PlistBuddy -c "Add :NSBonjourServices:2 string _lumi-remote-dev._tcp" "$gateway_info_plist"
plutil -lint "$gateway_info_plist" >/dev/null
gateway_info_link_argument="-Wl,-sectcreate,__TEXT,__info_plist,$gateway_info_plist"
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=$gateway_info_link_argument" \
  cargo build --locked --release -p lumi-remote-gateway --target-dir "$gateway_target_directory"
install -m 755 \
  "$gateway_target_directory/$profile_directory/lumi-remote-gateway" \
  "$helpers_directory/lumi-remote-gateway"
if ! otool -s __TEXT __info_plist "$helpers_directory/lumi-remote-gateway" >/dev/null 2>&1; then
  echo "ERROR: lumi-remote-gateway is missing its embedded LaunchAgent Info.plist." >&2
  exit 1
fi

# Build the exact same bounded USB mode without a standalone bundle identity.
# macOS then attributes removable-volume consent to the foreground Lumi app
# that launches it, instead of to the persistent SMAppService engine. Keep the
# two executables separate: only lumi-engine may be registered with launchd.
cargo build --locked --release -p lumi-engine
install -m 755 "target/$profile_directory/lumi-engine" "$helpers_directory/lumi-usb-worker"
if otool -l "$helpers_directory/lumi-usb-worker" \
  | grep -q 'sectname __info_plist'; then
  echo "ERROR: lumi-usb-worker must not carry the LaunchAgent bundle identity." >&2
  exit 1
fi

launch_agent_label="${PRODUCT_BUNDLE_IDENTIFIER}.engine"
launch_agent_plist="$launch_agents_directory/$launch_agent_label.plist"
rm -rf "$launch_agents_directory"
install -d "$launch_agents_directory"
plutil -create xml1 "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :Label string $launch_agent_label" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :BundleProgram string Contents/Helpers/lumi-engine" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :RunAtLoad bool true" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :KeepAlive bool true" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :ProcessType string Interactive" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :ThrottleInterval integer 2" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables dict" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_SERVICE_MODE string launchd" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_DATA_DIRECTORY_NAME string $LUMI_DATA_DIRECTORY_NAME" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_PRODUCT_VERSION string $LUMI_PRODUCT_VERSION" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_BUILD_NUMBER string $CURRENT_PROJECT_VERSION" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_SEED_DEMO_LIBRARY string $LUMI_SEED_DEMO_LIBRARY" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_AUTO_PUBLISH_MIDI string 1" "$launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_EXIT_AFTER_CLIENT_DISCONNECT string 0" "$launch_agent_plist"
plutil -lint "$launch_agent_plist" >/dev/null

gateway_launch_agent_label="${PRODUCT_BUNDLE_IDENTIFIER}.remote-gateway"
gateway_launch_agent_plist="$launch_agents_directory/$gateway_launch_agent_label.plist"
plutil -create xml1 "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :Label string $gateway_launch_agent_label" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :BundleProgram string Contents/Helpers/lumi-remote-gateway" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :RunAtLoad bool true" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :KeepAlive bool true" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :ProcessType string Background" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :ThrottleInterval integer 5" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables dict" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_DATA_DIRECTORY_NAME string $LUMI_DATA_DIRECTORY_NAME" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_PRODUCT_VERSION string $LUMI_PRODUCT_VERSION" "$gateway_launch_agent_plist"
/usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:LUMI_RELEASE_CHANNEL string $LUMI_RELEASE_CHANNEL" "$gateway_launch_agent_plist"
plutil -lint "$gateway_launch_agent_plist" >/dev/null

# Bundle the Direct Pro DJ Link bridge and a matching Apple Silicon runtime so
# Lumi never depends on a separately installed JDK or Beat Link Trigger.
bridge_jar="$repository_root/bridges/prolink/target/lumi-prolink-bridge.jar"
bridge_sources="$repository_root/bridges/prolink/src"
toolchain_home="${LUMI_PACKAGING_JAVA_HOME:-$repository_root/build/package-toolchains/temurin-21-macos-aarch64/Contents/Home}"
runtime_cache="$repository_root/build/prolink-bridge-runtime"
runtime_modules="java.base,java.desktop,java.prefs,java.sql"
runtime_marker="$runtime_cache/.lumi-modules"

bridge_needs_build=0
if [[ ! -f "$bridge_jar" ]]; then
  bridge_needs_build=1
elif [[ -n "$(find "$bridge_sources" "$repository_root/bridges/prolink/pom.xml" -newer "$bridge_jar" -print -quit)" ]]; then
  bridge_needs_build=1
fi

if [[ "$bridge_needs_build" == "1" ]]; then
  if [[ ! -x "$toolchain_home/bin/java" ]]; then
    echo "ERROR: Java 21 toolchain is required to build the Pro DJ Link bridge." >&2
    exit 1
  fi
  if ! command -v mvn >/dev/null 2>&1; then
    echo "ERROR: Maven is required to build the Pro DJ Link bridge." >&2
    exit 1
  fi
  JAVA_HOME="$toolchain_home" mvn \
    --batch-mode \
    --no-transfer-progress \
    -Dmaven.repo.local="$repository_root/build/maven-repository" \
    --file "$repository_root/bridges/prolink/pom.xml" \
    package \
    -DskipTests
fi

if [[ ! -x "$runtime_cache/bin/java" || ! -f "$runtime_marker" || "$(cat "$runtime_marker" 2>/dev/null || true)" != "$runtime_modules" ]]; then
  if [[ ! -x "$toolchain_home/bin/jlink" ]]; then
    echo "ERROR: Java 21 jlink is required to package the Pro DJ Link bridge." >&2
    exit 1
  fi
  runtime_staging="$(mktemp -d "$repository_root/build/.prolink-runtime.XXXXXX")"
  trap 'rm -rf "$runtime_staging"' EXIT
  "$toolchain_home/bin/jlink" \
    --add-modules "$runtime_modules" \
    --strip-debug \
    --no-header-files \
    --no-man-pages \
    --output "$runtime_staging/runtime"
  if [[ -e "$runtime_cache" ]]; then
    mv "$runtime_cache" "$runtime_cache.invalid.$(date +%s)"
  fi
  printf '%s\n' "$runtime_modules" > "$runtime_staging/runtime/.lumi-modules"
  mv "$runtime_staging/runtime" "$runtime_cache"
  rm -rf "$runtime_staging"
  trap - EXIT
fi

install -d "$resources_directory"
rm -rf "$helpers_directory/prolink"
install -d "$resources_directory/prolink"
install -m 644 "$bridge_jar" "$resources_directory/prolink/lumi-prolink-bridge.jar"
rm -rf "$helpers_directory/prolink-runtime"
rm -rf "$resources_directory/prolink-runtime"
ditto "$runtime_cache" "$resources_directory/prolink-runtime"

# Bundle the separately licensed Ableton Link helper. Acquisition is explicit
# and checksum-pinned so normal local builds never depend on network access.
carabiner_cache="$repository_root/build/carabiner-runtime"
if [[ ! -x "$carabiner_cache/Carabiner" || ! -f "$carabiner_cache/LICENSE.md" ]]; then
  echo "ERROR: managed Ableton Link runtime is missing." >&2
  echo "Run ./scripts/prepare-carabiner-runtime.sh once." >&2
  exit 1
fi
rm -rf "$resources_directory/link"
install -d "$resources_directory/link"
install -m 755 "$carabiner_cache/Carabiner" "$resources_directory/link/Carabiner"
install -m 644 "$carabiner_cache/PROVENANCE.txt" "$resources_directory/link/PROVENANCE.txt"

# Keep the application's own terms and the copyleft runtime notices available
# inside every installed app, not only at the DMG root.
legal_directory="$resources_directory/legal"
rm -rf "$legal_directory"
install -d "$legal_directory"
install -m 644 "$repository_root/LICENSE" "$legal_directory/Lumi-EPL-2.0.txt"
install -m 644 "$repository_root/TRADEMARKS.md" "$legal_directory/TRADEMARKS.md"
install -m 644 "$repository_root/THIRD_PARTY_NOTICES.md" \
  "$legal_directory/THIRD-PARTY-NOTICES.md"
install -m 644 "$carabiner_cache/LICENSE.md" \
  "$legal_directory/Carabiner-GPL-2.0-or-later.md"

remote_tea_jar="$repository_root/build/maven-repository/org/acplt/remotetea/remotetea-oncrpc/1.1.4/remotetea-oncrpc-1.1.4.jar"
if [[ ! -f "$remote_tea_jar" ]]; then
  echo "ERROR: Remote Tea runtime dependency is missing from the pinned Maven repository." >&2
  exit 1
fi
license_staging="$(mktemp -d "$repository_root/build/.remote-tea-license.XXXXXX")"
(
  cd "$license_staging"
  "$toolchain_home/bin/jar" xf "$remote_tea_jar" META-INF/LICENSE.txt
)
install -m 644 "$license_staging/META-INF/LICENSE.txt" \
  "$legal_directory/Remote-Tea-LGPL-2.0.txt"
rm -rf "$license_staging"
