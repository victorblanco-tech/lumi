#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "Usage: $0 <rc|dev>" >&2
  exit 64
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: Lumi channel cloning requires macOS." >&2
  exit 1
fi

case "$1" in
  rc)
    target_directory_name="Lumi RC"
    target_preferences_domain="co.victorblan.tech.lumi.rc"
    ;;
  dev)
    target_directory_name="Lumi Dev"
    target_preferences_domain="co.victorblan.tech.lumi.dev"
    ;;
  *)
    echo "Usage: $0 <rc|dev>" >&2
    exit 64
    ;;
esac

if pgrep -x "Lumi" >/dev/null || pgrep -x "lumi-engine" >/dev/null; then
  echo "ERROR: close every Lumi channel before cloning data." >&2
  exit 1
fi

application_support="$HOME/Library/Application Support"
source_database="$application_support/Lumi/library.sqlite"
target_directory="$application_support/$target_directory_name"
target_database="$target_directory/library.sqlite"
release_preferences_domain="co.victorblan.tech.lumi"
temporary_preferences="$(mktemp "${TMPDIR:-/tmp}/lumi-preferences.XXXXXX.plist")"

cleanup() {
  rm -f "$temporary_preferences"
}
trap cleanup EXIT

if [[ ! -f "$source_database" ]]; then
  echo "ERROR: Lumi release database not found at '$source_database'." >&2
  exit 1
fi
if [[ -e "$target_database" ]]; then
  echo "ERROR: target database already exists at '$target_database'; nothing was overwritten." >&2
  exit 1
fi
if defaults read "$target_preferences_domain" >/dev/null 2>&1; then
  echo "ERROR: target preferences '$target_preferences_domain' already exist; nothing was overwritten." >&2
  exit 1
fi

mkdir -p "$target_directory"
sqlite3 "$source_database" ".timeout 5000" ".backup '$target_database'"

integrity_result="$(sqlite3 "$target_database" 'PRAGMA integrity_check;')"
if [[ "$integrity_result" != "ok" ]]; then
  echo "ERROR: cloned database failed integrity validation: $integrity_result" >&2
  exit 1
fi

if defaults read "$release_preferences_domain" >/dev/null 2>&1 \
  && defaults export "$release_preferences_domain" "$temporary_preferences" >/dev/null 2>&1; then
  defaults import "$target_preferences_domain" "$temporary_preferences" >/dev/null
fi

echo "Created isolated $1 channel database: $target_database"
echo "Copied release preferences when available: $target_preferences_domain"
