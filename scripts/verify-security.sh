#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"

mode="${1:-all}"
case "$mode" in
  all)
    failed=0
    for gate in rust maven licenses; do
      if ! bash "$0" "$gate"; then
        echo "ERROR: security gate '$gate' failed." >&2
        failed=1
      fi
    done
    exit "$failed"
    ;;
  licenses)
    exec "$script_dir/verify-license-inventory.sh"
    ;;
  rust|maven) ;;
  *) echo "Usage: $0 [all|rust|maven|licenses]" >&2; exit 2 ;;
esac

if [[ "$mode" == rust ]]; then
cargo_audit_executable="${LUMI_CARGO_AUDIT_EXECUTABLE:-}"
if [[ -z "$cargo_audit_executable" ]]; then
  cargo_audit_executable="$(command -v cargo-audit || true)"
fi
if [[ -z "$cargo_audit_executable" && -x "${CARGO_HOME:-${HOME}/.cargo}/bin/cargo-audit" ]]; then
  cargo_audit_executable="${CARGO_HOME:-${HOME}/.cargo}/bin/cargo-audit"
fi
if [[ -z "$cargo_audit_executable" || ! -x "$cargo_audit_executable" ]]; then
  echo "ERROR: cargo-audit is required. Install it with: cargo install cargo-audit --locked" >&2
  exit 1
fi
exec "$cargo_audit_executable" audit --file "$repository_root/Cargo.lock"
fi

for required_tool in curl jq mvn; do
  if ! command -v "$required_tool" >/dev/null 2>&1; then
    echo "ERROR: '$required_tool' is required for the local security gate." >&2
    exit 1
  fi
done

temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/lumi-security.XXXXXX")"
cleanup() {
  rm -rf "$temporary_directory"
}
trap cleanup EXIT

dependency_rows="$temporary_directory/maven-dependencies.tsv"
: >"$dependency_rows"
for module in bridges/prolink tools/prolink-simulator; do
  tree_file="$temporary_directory/$(basename "$module").tree"
  mvn \
    --batch-mode \
    --no-transfer-progress \
    -Dmaven.repo.local="$repository_root/build/maven-repository" \
    --file "$repository_root/$module/pom.xml" \
    -Dscope=runtime \
    -DoutputType=text \
    -DoutputFile="$tree_file" \
    dependency:tree >/dev/null
  awk -F: '
    {
      line = $0
      sub(/^[^[:alnum:]]*/, "", line)
      count = split(line, part, ":")
      if (count >= 5) {
        print part[1] ":" part[2] "\t" part[count - 1]
      }
    }
  ' "$tree_file" >>"$dependency_rows"
done

sort -u "$dependency_rows" -o "$dependency_rows"
if [[ ! -s "$dependency_rows" ]]; then
  echo "ERROR: Maven returned no runtime dependencies; refusing an empty audit." >&2
  exit 1
fi
query_file="$temporary_directory/osv-query.json"
response_file="$temporary_directory/osv-response.json"
jq -Rn \
  '[inputs | split("\t") | {package: {ecosystem: "Maven", name: .[0]}, version: .[1]}] | {queries: .}' \
  <"$dependency_rows" >"$query_file"

curl \
  --fail \
  --silent \
  --show-error \
  --max-time 30 \
  --header 'Content-Type: application/json' \
  --data-binary "@$query_file" \
  https://api.osv.dev/v1/querybatch >"$response_file"

# Never treat a truncated/error response as a clean audit.
jq -e --slurpfile query "$query_file" '
  (.results | type == "array") and
  (.results | length == ($query[0].queries | length)) and
  all(.results[]; type == "object" and (has("error") | not) and
    ((has("vulns") | not) or (.vulns | type == "array")))
' "$response_file" >/dev/null

finding_count="$(jq '[.results[] | .vulns // [] | .[]] | unique_by(.id) | length' "$response_file")"
if [[ "$finding_count" != "0" ]]; then
  echo "ERROR: OSV reported $finding_count unique Maven advisories:" >&2
  jq -r \
    '[.results[]? | .vulns // [] | .[]] | unique_by(.id)[] | "  \(.id): \(.summary // \"no summary\")"' \
    "$response_file" >&2
  exit 1
fi

echo "Maven dependency security gate passed."
