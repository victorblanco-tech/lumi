#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

"$script_dir/check-version.sh"
"$script_dir/check-structure.sh"
"$script_dir/check-architecture.sh"

cd "$repository_root"

python3 -m json.tool apps/macos/Lumi/Resources/Localizable.xcstrings >/dev/null
python3 -m json.tool \
  apps/macos/Packages/LumiLiveWorkspace/Localization/Localizable.xcstrings \
  >/dev/null
for protocol_document in \
  contracts/protocol/v1/manifest.json \
  contracts/protocol/v1/envelope.schema.json \
  contracts/protocol/v1/fixtures/*.json; do
  python3 -m json.tool "$protocol_document" >/dev/null
done

cargo test --locked --all-features \
  -p lumi-domain \
  -p lumi-planner \
  -p lumi-library \
  -p lumi-library-sqlite \
  -p lumi-engine

for swift_package in \
  LumiProtocol \
  LumiDesignSystem \
  LumiLiveWorkspace \
  LumiLibraryWorkspace; do
  swift test \
    -Xswiftc -warnings-as-errors \
    --package-path "apps/macos/Packages/$swift_package"
done

echo "Local functional regression gate passed."
