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

echo "Apple integration test ownership is available."
