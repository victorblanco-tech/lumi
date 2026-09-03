#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
icon_dir="$repository_root/apps/ios/LumiRemote/Resources/Assets.xcassets/AppIcon.appiconset"

check_icon() {
  local filename="$1"
  local expected_size="$2"
  local path="$icon_dir/$filename"
  local width
  local height
  local alpha

  [[ -f "$path" ]] || { echo "ERROR: missing iOS app icon $filename." >&2; exit 1; }
  width="$(sips -g pixelWidth "$path" 2>/dev/null | awk '/pixelWidth:/ { print $2 }')"
  height="$(sips -g pixelHeight "$path" 2>/dev/null | awk '/pixelHeight:/ { print $2 }')"
  alpha="$(sips -g hasAlpha "$path" 2>/dev/null | awk '/hasAlpha:/ { print $2 }')"

  [[ "$width" == "$expected_size" && "$height" == "$expected_size" ]] || {
    echo "ERROR: $filename is ${width}x${height}; expected ${expected_size}x${expected_size}." >&2
    exit 1
  }
  [[ "$alpha" == "no" ]] || {
    echo "ERROR: $filename contains alpha; App Store icons must be opaque." >&2
    exit 1
  }
}

[[ -f "$icon_dir/Contents.json" ]] || {
  echo "ERROR: missing iOS AppIcon asset catalog metadata." >&2
  exit 1
}

check_icon lumi-remote-app-icon-40.png 40
check_icon lumi-remote-app-icon-58.png 58
check_icon lumi-remote-app-icon-60.png 60
check_icon lumi-remote-app-icon-80.png 80
check_icon lumi-remote-app-icon-87.png 87
check_icon lumi-remote-app-icon-120-settings.png 120
check_icon lumi-remote-app-icon-120.png 120
check_icon lumi-remote-app-icon-180.png 180
check_icon lumi-remote-app-icon-1024.png 1024

echo "Lumi Remote app icon check passed."
