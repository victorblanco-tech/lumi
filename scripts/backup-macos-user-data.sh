#!/usr/bin/env bash

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: Lumi user-data backup requires macOS." >&2
  exit 1
fi

application_support="$HOME/Library/Application Support"
source_directory="$application_support/Lumi"
source_database="$source_directory/library.sqlite"
backup_root="${1:-$application_support/Lumi Backups}"
timestamp="$(date -u '+%Y%m%dT%H%M%SZ')"
backup_directory="$backup_root/$timestamp"
release_preferences_domain="co.victorblan.tech.lumi"

if pgrep -x "Lumi" >/dev/null || pgrep -x "lumi-engine" >/dev/null; then
  echo "ERROR: close every Lumi channel before backing up its data." >&2
  exit 1
fi

if [[ ! -f "$source_database" ]]; then
  echo "ERROR: Lumi release database not found at '$source_database'." >&2
  exit 1
fi

mkdir -p "$backup_directory"
sqlite3 "$source_database" ".timeout 5000" ".backup '$backup_directory/library.sqlite'"

integrity_result="$(sqlite3 "$backup_directory/library.sqlite" 'PRAGMA integrity_check;')"
if [[ "$integrity_result" != "ok" ]]; then
  echo "ERROR: database backup failed integrity validation: $integrity_result" >&2
  exit 1
fi

if defaults read "$release_preferences_domain" >/dev/null 2>&1 \
  && defaults export "$release_preferences_domain" \
    "$backup_directory/preferences.plist" >/dev/null 2>&1; then
  echo "Preferences: $backup_directory/preferences.plist"
else
  echo "Preferences: no release preferences domain exists yet"
fi

echo "Database: $backup_directory/library.sqlite"
echo "Backup completed: $backup_directory"
