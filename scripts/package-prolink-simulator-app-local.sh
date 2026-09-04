#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
distribution_root="$repository_root/build/prolink-simulator-app"
app_name="Lumi Pro DJ Link Simulator"
release_version="$(tr -d '[:space:]' < "$repository_root/tools/prolink-simulator/VERSION")"
marketing_version="${release_version%%-*}"
build_number="1"

if [[ "$release_version" =~ -(dev|rc)-([0-9]+)$ ]]; then
  build_number="${BASH_REMATCH[2]}"
fi
if [[ ! "$marketing_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: tools/prolink-simulator/VERSION must use x.y.z, x.y.z-dev-N or x.y.z-rc-N." >&2
  exit 1
fi

case "$(uname -m)" in
  arm64) temurin_architecture="aarch64" ;;
  x86_64) temurin_architecture="x64" ;;
  *) echo "ERROR: unsupported simulator packaging architecture: $(uname -m)" >&2; exit 1 ;;
esac
temurin_cache="$repository_root/build/package-toolchains/temurin-21-macos-${temurin_architecture}"
packaging_java_home="${SIMULATOR_PACKAGING_JAVA_HOME:-$temurin_cache/Contents/Home}"

if [[ ! -x "$packaging_java_home/bin/jpackage" ]]; then
  mkdir -p "$(dirname "$temurin_cache")"
  download_root="$(mktemp -d "$repository_root/build/package-toolchains/.temurin-download.XXXXXX")"
  metadata="$download_root/metadata.json"
  archive="$download_root/temurin.tar.gz"
  curl --fail --location --silent --show-error \
    "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=${temurin_architecture}&heap_size=normal&image_type=jdk&jvm_impl=hotspot&os=mac&project=jdk&vendor=eclipse" \
    --output "$metadata"
  download_url="$(jq -er '.[0].binary.package.link' "$metadata")"
  expected_checksum="$(jq -er '.[0].binary.package.checksum' "$metadata")"
  curl --fail --location --show-error "$download_url" --output "$archive"
  actual_checksum="$(shasum -a 256 "$archive" | awk '{print $1}')"
  if [[ "$actual_checksum" != "$expected_checksum" ]]; then
    echo "ERROR: Temurin JDK checksum mismatch." >&2
    exit 1
  fi
  tar -xzf "$archive" -C "$download_root"
  downloaded_jdk="$(
    find "$download_root" -mindepth 1 -maxdepth 1 -type d -print | while IFS= read -r candidate; do
      if [[ -x "$candidate/Contents/Home/bin/jpackage" ]]; then
        echo "$candidate"
        break
      fi
    done
  )"
  if [[ -z "$downloaded_jdk" || ! -x "$downloaded_jdk/Contents/Home/bin/jpackage" ]]; then
    echo "ERROR: Downloaded Temurin archive does not contain jpackage." >&2
    exit 1
  fi
  if [[ -e "$temurin_cache" ]]; then
    mv "$temurin_cache" "$temurin_cache.invalid.$(date +%s)"
  fi
  mv "$downloaded_jdk" "$temurin_cache"
  rm -rf "$download_root"
fi

export JAVA_HOME="$packaging_java_home"

"$script_dir/verify-prolink-simulator.sh"

mkdir -p "$distribution_root"
staging_root="$(mktemp -d "$distribution_root/.staging.XXXXXX")"
trap 'rm -rf "$staging_root"' EXIT
input_root="$staging_root/input"
icon_output="$staging_root/icon-output"
mkdir -p "$input_root" "$icon_output"

cp "$repository_root/tools/prolink-simulator/target/lumi-prolink-simulator.jar" "$input_root/"

xcrun actool "$repository_root/apps/macos/Lumi/Resources/Assets.xcassets" \
  --compile "$icon_output" \
  --platform macosx \
  --minimum-deployment-target 14.0 \
  --app-icon AppIcon \
  --output-partial-info-plist "$icon_output/Info.plist" >/dev/null

package_architecture="$(uname -m)"
dmg="$distribution_root/Lumi-Pro-DJ-Link-Simulator-${release_version}-macOS-${package_architecture}.dmg"
rm -f "$dmg"
app_image_root="$staging_root/app-image"
# jpackage rejects pre-1.0 versions whose first component is zero. The
# generated Info.plist is replaced with Lumi's real version below.
"$JAVA_HOME/bin/jpackage" \
  --type app-image \
  --dest "$app_image_root" \
  --input "$input_root" \
  --name "$app_name" \
  --main-jar lumi-prolink-simulator.jar \
  --main-class co.victorblan.tech.lumi.prolink.simulator.SimulatorAppMain \
  --app-version "1.0.0" \
  --vendor "VB Tech" \
  --copyright "Copyright © 2026 Victor Blanco" \
  --description "Self-contained USB-backed Pro DJ Link player simulator for Lumi." \
  --icon "$icon_output/AppIcon.icns" \
  --mac-package-identifier co.victorblan.tech.lumi.prolinksimulator \
  --mac-package-name "Lumi Simulator" \
  --mac-app-category public.app-category.developer-tools \
  --java-options "-Dfile.encoding=UTF-8" \
  --java-options "-Dapple.awt.application.appearance=system"

app_bundle="$app_image_root/$app_name.app"
info_plist="$app_bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $marketing_version" "$info_plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build_number" "$info_plist"

external_dependencies="$(
  find "$app_bundle" -type f -print0 | while IFS= read -r -d '' binary; do
    if file "$binary" | grep -q 'Mach-O'; then
      otool -L "$binary" 2>/dev/null | grep -E '/usr/local/|/opt/homebrew/' || true
    fi
  done
)"
if [[ -n "$external_dependencies" ]]; then
  echo "ERROR: Packaged app contains non-portable Homebrew dependencies:" >&2
  echo "$external_dependencies" >&2
  exit 1
fi
codesign --force --deep --sign - "$app_bundle"

dmg_root="$staging_root/dmg"
mkdir -p "$dmg_root"
ditto "$app_bundle" "$dmg_root/$app_name.app"
ln -s /Applications "$dmg_root/Applications"
cp "$repository_root/tools/prolink-simulator/INSTALL.txt" "$dmg_root/READ ME - Installation.txt"
hdiutil create \
  -volname "$app_name" \
  -srcfolder "$dmg_root" \
  -ov \
  -format UDZO \
  "$dmg" >/dev/null

echo "$dmg"
