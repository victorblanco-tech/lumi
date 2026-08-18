#!/usr/bin/env bash

set -euo pipefail

simulator_url="${LUMI_SIM_URL:-}"
simulator_token="${LUMI_SIM_TOKEN:-}"

if [[ -z "$simulator_url" || -z "$simulator_token" ]]; then
  echo "ERROR: set LUMI_SIM_URL and LUMI_SIM_TOKEN first." >&2
  exit 1
fi
simulator_url="${simulator_url%/}"

request_get() {
  curl --fail --silent --show-error \
    -H "Authorization: Bearer $simulator_token" \
    "$simulator_url$1"
  echo
}

request_post() {
  curl --fail --silent --show-error \
    -H "Authorization: Bearer $simulator_token" \
    -H 'Content-Type: application/json' \
    --data "$2" \
    "$simulator_url$1"
  echo
}

command="${1:-}"
case "$command" in
  status)
    request_get '/api/v1/status'
    ;;
  tracks)
    query="${2:-}"
    curl --fail --silent --show-error --get \
      -H "Authorization: Bearer $simulator_token" \
      --data-urlencode "q=$query" \
      --data-urlencode 'limit=100' \
      "$simulator_url/api/v1/tracks"
    echo
    ;;
  load)
    request_post '/api/v1/control/load' "{\"trackId\":${2:?track ID required}}"
    ;;
  play|pause)
    request_post "/api/v1/control/$command" '{}'
    ;;
  seek)
    request_post '/api/v1/control/seek' "{\"positionMillis\":${2:?position in milliseconds required}}"
    ;;
  pitch)
    request_post '/api/v1/control/pitch' "{\"pitchPercent\":${2:?pitch percentage required}}"
    ;;
  master|on-air)
    case "${2:-}" in
      on) enabled=true ;;
      off) enabled=false ;;
      *) echo "ERROR: $command expects on or off." >&2; exit 1 ;;
    esac
    request_post "/api/v1/control/$command" "{\"enabled\":$enabled}"
    ;;
  *)
    echo "Usage: $0 {status|tracks [query]|load ID|play|pause|seek MS|pitch PERCENT|master on|off|on-air on|off}" >&2
    exit 2
    ;;
esac
