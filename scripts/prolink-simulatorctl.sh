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
    if [[ $# -ge 3 ]]; then player="${2:?player required}"; track_id="${3:?track ID required}"; else player=1; track_id="${2:?track ID required}"; fi
    request_post '/api/v1/control/load' "{\"playerNumber\":$player,\"trackId\":$track_id}"
    ;;
  play|pause)
    player="${2:-1}"
    request_post "/api/v1/control/$command" "{\"playerNumber\":$player}"
    ;;
  seek)
    if [[ $# -ge 3 ]]; then player="${2:?player required}"; position="${3:?position required}"; else player=1; position="${2:?position required}"; fi
    request_post '/api/v1/control/seek' "{\"playerNumber\":$player,\"positionMillis\":$position}"
    ;;
  pitch)
    if [[ $# -ge 3 ]]; then player="${2:?player required}"; pitch="${3:?pitch required}"; else player=1; pitch="${2:?pitch required}"; fi
    request_post '/api/v1/control/pitch' "{\"playerNumber\":$player,\"pitchPercent\":$pitch}"
    ;;
  master|on-air)
    if [[ $# -ge 3 ]]; then player="${2:?player required}"; toggle="${3:-}"; else player=1; toggle="${2:-}"; fi
    case "$toggle" in
      on) enabled=true ;;
      off) enabled=false ;;
      *) echo "ERROR: $command expects on or off." >&2; exit 1 ;;
    esac
    request_post "/api/v1/control/$command" "{\"playerNumber\":$player,\"enabled\":$enabled}"
    ;;
  loop)
    request_post '/api/v1/control/loop' "{\"playerNumber\":${2:?player required},\"startMillis\":${3:?start required},\"endMillis\":${4:?end required}}"
    ;;
  loop-off|precise-burst)
    player="${2:-1}"
    request_post "/api/v1/control/$command" "{\"playerNumber\":$player}"
    ;;
  auto-mix)
    case "${2:-}" in
      on) request_post '/api/v1/control/auto-mix' "{\"enabled\":true,\"intervalSeconds\":${3:-30}}" ;;
      off) request_post '/api/v1/control/auto-mix' "{\"enabled\":false,\"intervalSeconds\":${3:-30}}" ;;
      *) echo "ERROR: auto-mix expects on or off." >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Usage: $0 {status|tracks [query]|load [PLAYER] ID|play [PLAYER]|pause [PLAYER]|seek [PLAYER] MS|pitch [PLAYER] PERCENT|master [PLAYER] on|off|on-air [PLAYER] on|off|loop PLAYER START_MS END_MS|loop-off [PLAYER]|precise-burst [PLAYER]|auto-mix on|off [SECONDS]}" >&2
    exit 2
    ;;
esac
