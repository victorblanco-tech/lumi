#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  classify-ci-changes.sh --all
  classify-ci-changes.sh --from-git <base> <head>
  classify-ci-changes.sh <path> [<path> ...]

Writes docs, rust, apple and full as key=value lines. Set
LUMI_CI_OUTPUT_FILE to append the same values to a GitHub Actions output file.
EOF
}

declare -a changed_paths=()
force_full=0
git_range=0

case "${1:-}" in
  --all)
    force_full=1
    shift
    ;;
  --from-git)
    if [[ $# -ne 3 ]]; then
      usage >&2
      exit 2
    fi
    git_range=1
    while IFS= read -r changed_path; do
      changed_paths+=("$changed_path")
    done < <(git diff --name-only --diff-filter=ACDMRTUXB "$2" "$3")
    shift 3
    ;;
  --help|-h)
    usage
    exit 0
    ;;
  "")
    usage >&2
    exit 2
    ;;
  *)
    changed_paths=("$@")
    ;;
esac

docs=0
rust=0
apple=0
full=0

if [[ "$force_full" == "1" ]]; then
  docs=1
  rust=1
  apple=1
  full=1
elif [[ ${#changed_paths[@]} -eq 0 && "$git_range" == "1" ]]; then
  # A main merge commit synchronized back to dev can change commit history
  # without changing the repository tree. The stable gate still runs, but
  # there is no affected platform to rebuild.
  :
else
  for path in "${changed_paths[@]}"; do
    path="${path#./}"
    case "$path" in
      README.md|CHANGELOG.md|CODE_OF_CONDUCT.md|CONTRIBUTING.md|SECURITY.md|THIRD_PARTY_NOTICES.md|TRADEMARKS.md|LICENSE|docs/*|.github/ISSUE_TEMPLATE/*|.github/PULL_REQUEST_TEMPLATE.md|.github/release.yml)
        docs=1
        ;;
      apps/macos/*|bridges/prolink/*|tools/prolink-simulator/*)
        apple=1
        ;;
      engine/*|contracts/*|fixtures/*|Cargo.toml|Cargo.lock|rust-toolchain.toml|rust-toolchain)
        rust=1
        apple=1
        ;;
      *)
        # Build, workflow, version, script and unfamiliar changes are never
        # optimized away. This is the safety net for new repository areas.
        rust=1
        apple=1
        full=1
        ;;
    esac
  done
fi

result="docs=$docs
rust=$rust
apple=$apple
full=$full"
printf '%s\n' "$result"

if [[ -n "${LUMI_CI_OUTPUT_FILE:-}" ]]; then
  printf '%s\n' "$result" >> "$LUMI_CI_OUTPUT_FILE"
fi
