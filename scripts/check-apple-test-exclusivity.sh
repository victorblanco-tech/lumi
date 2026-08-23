#!/usr/bin/env bash

set -euo pipefail

# The native integration suite owns fixed CoreMIDI endpoint names. A normal
# installed Lumi process holding those endpoints makes the test result invalid.
# Never terminate a potentially active show automatically.
set +e
running_lumi="$(pgrep -fl '/Applications/(Lumi[^/]*\.app|Lumi/.+\.app)/Contents/(MacOS/Lumi|Helpers/lumi-engine)' 2>&1)"
pgrep_status=$?
set -e

if [[ "$pgrep_status" -eq 0 ]]; then
  echo "ERROR: stop every installed Lumi Dev, RC and Prod app/service before Apple verification." >&2
  echo "A running instance owns the CoreMIDI endpoints used by LumiEngineClient tests:" >&2
  echo "$running_lumi" >&2
  exit 1
fi
if [[ "$pgrep_status" -ne 1 ]]; then
  echo "ERROR: unable to verify exclusive CoreMIDI test ownership: $running_lumi" >&2
  exit 1
fi

# SMAppService launch agents deliberately use a short argv[0], so pgrep may
# only see `Contents/Helpers/lumi-engine` and miss the installed bundle path.
# Check the registered channel labels as the authoritative second guard.
running_services="$(launchctl list | awk '
  $1 ~ /^[0-9]+$/ && $3 ~ /^co\.victorblan\.tech\.lumi(\.dev|\.rc)?\.engine$/ {
    print $1 " " $3
  }
')"
if [[ -n "$running_services" ]]; then
  echo "ERROR: stop every installed Lumi Dev, RC and Prod service before Apple verification." >&2
  echo "Active launch services own the CoreMIDI endpoints used by LumiEngineClient tests:" >&2
  echo "$running_services" >&2
  exit 1
fi

echo "Apple integration test ownership is available."
