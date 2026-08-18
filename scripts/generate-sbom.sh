#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
output_path="${1:-}"
cargo_bin_directory="${CARGO_HOME:-${HOME}/.cargo}/bin"

if [[ -d "$cargo_bin_directory" ]]; then
  export PATH="$cargo_bin_directory:$PATH"
fi

if [[ -z "$output_path" ]]; then
  echo "Usage: ./scripts/generate-sbom.sh <output.spdx.json>" >&2
  exit 2
fi

temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/lumi-sbom.XXXXXX")"
cleanup() {
  rm -rf "$temporary_directory"
}
trap cleanup EXIT

cd "$repository_root"
cargo metadata --locked --offline --format-version 1 > "$temporary_directory/cargo-metadata.json"

python3 - "$temporary_directory/cargo-metadata.json" "$output_path" "$(tr -d '[:space:]' < VERSION)" <<'PY'
import datetime
import hashlib
import json
import pathlib
import sys

metadata_path = pathlib.Path(sys.argv[1])
output_path = pathlib.Path(sys.argv[2])
version = sys.argv[3]
metadata = json.loads(metadata_path.read_text())

packages = []
relationships = []
for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
    key = f'{package["name"]}-{package["version"]}'
    identifier = "SPDXRef-Package-" + hashlib.sha256(key.encode()).hexdigest()[:16]
    source = package.get("source") or "NOASSERTION"
    license_value = package.get("license") or "NOASSERTION"
    packages.append({
        "SPDXID": identifier,
        "name": package["name"],
        "versionInfo": package["version"],
        "downloadLocation": source,
        "filesAnalyzed": False,
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": license_value,
        "copyrightText": "NOASSERTION",
        "externalRefs": [{
            "referenceCategory": "PACKAGE-MANAGER",
            "referenceType": "purl",
            "referenceLocator": f'pkg:cargo/{package["name"]}@{package["version"]}',
        }],
    })
    relationships.append({
        "spdxElementId": "SPDXRef-Lumi",
        "relationshipType": "DEPENDS_ON",
        "relatedSpdxElement": identifier,
    })

created = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
document = {
    "spdxVersion": "SPDX-2.3",
    "dataLicense": "CC0-1.0",
    "SPDXID": "SPDXRef-DOCUMENT",
    "name": f"Lumi-{version}",
    "documentNamespace": f"https://tech.victorblan.co/lumi/sbom/{version}/{hashlib.sha256(created.encode()).hexdigest()[:16]}",
    "creationInfo": {
        "created": created,
        "creators": ["Tool: Lumi local release pipeline"],
    },
    "packages": [{
        "SPDXID": "SPDXRef-Lumi",
        "name": "Lumi",
        "versionInfo": version,
        "downloadLocation": "https://github.com/victorblanco-tech/lumi",
        "filesAnalyzed": False,
        "licenseConcluded": "EPL-2.0",
        "licenseDeclared": "EPL-2.0",
        "copyrightText": "Copyright Victor Blanco",
    }, *packages],
    "relationships": [{
        "spdxElementId": "SPDXRef-DOCUMENT",
        "relationshipType": "DESCRIBES",
        "relatedSpdxElement": "SPDXRef-Lumi",
    }, *relationships],
}
output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
PY

python3 -m json.tool "$output_path" >/dev/null
echo "SPDX SBOM generated: $output_path"
