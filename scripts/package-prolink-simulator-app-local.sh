#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
distribution_root="$repository_root/build/prolink-simulator-app"
app_name="Lumi Pro DJ Link Simulator"
version="0.4.0"

if [[ -z "${JAVA_HOME:-}" ]]; then
  for java_candidate in \
    /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home \
    /usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home; do
    if [[ -x "$java_candidate/bin/jpackage" ]]; then
      export JAVA_HOME="$java_candidate"
      break
    fi
  done
fi
if [[ -z "${JAVA_HOME:-}" || ! -x "$JAVA_HOME/bin/jpackage" ]]; then
  echo "ERROR: OpenJDK 21 with jpackage is required to package the simulator app." >&2
  exit 1
fi

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

dmg="$distribution_root/Lumi-Pro-DJ-Link-Simulator-${version}-macOS-arm64.dmg"
rm -f "$dmg"
app_image_root="$staging_root/app-image"
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
  --description "Development-only USB-backed Pro DJ Link player simulator for Lumi." \
  --icon "$icon_output/AppIcon.icns" \
  --mac-package-identifier co.victorblan.tech.lumi.prolinksimulator \
  --mac-package-name "Lumi Simulator" \
  --mac-app-category public.app-category.developer-tools \
  --java-options "-Dfile.encoding=UTF-8" \
  --java-options "-Dapple.awt.application.appearance=system"

app_bundle="$app_image_root/$app_name.app"
info_plist="$app_bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$info_plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion 1" "$info_plist"
codesign --force --deep --sign - "$app_bundle"

dmg_root="$staging_root/dmg"
mkdir -p "$dmg_root"
ditto "$app_bundle" "$dmg_root/$app_name.app"
ln -s /Applications "$dmg_root/Applications"
hdiutil create \
  -volname "$app_name" \
  -srcfolder "$dmg_root" \
  -ov \
  -format UDZO \
  "$dmg" >/dev/null

echo "$dmg"
