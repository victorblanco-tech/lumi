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

# Cargo metadata does not describe the separately packaged Java bridge, jlink
# runtime, native SQLCipher/OpenSSL sources or the Carabiner process. Keep their
# pinned runtime inventory in the same release SBOM so the published artifact is
# not represented as Rust-only.
additional_components = [
    ("beat-link", "8.0.0", "EPL-1.0", "pkg:maven/org.deepsymmetry/beat-link@8.0.0", "https://github.com/Deep-Symmetry/beat-link/tree/v8.0.0"),
    ("electro", "0.1.4", "EPL-1.0", "pkg:maven/org.deepsymmetry/electro@0.1.4", "https://github.com/Deep-Symmetry/electro"),
    ("commons-math3", "3.6.1", "Apache-2.0", "pkg:maven/org.apache.commons/commons-math3@3.6.1", "https://commons.apache.org/proper/commons-math/"),
    ("crate-digger", "0.2.1", "EPL-1.0", "pkg:maven/org.deepsymmetry/crate-digger@0.2.1", "https://github.com/Deep-Symmetry/crate-digger"),
    ("remotetea-oncrpc", "1.1.4", "LGPL-2.0-only", "pkg:maven/org.acplt.remotetea/remotetea-oncrpc@1.1.4", "https://repo1.maven.org/maven2/org/acplt/remotetea/remotetea-oncrpc/1.1.4/"),
    ("sqlite-jdbc", "3.49.0.0", "Apache-2.0", "pkg:maven/io.github.willena/sqlite-jdbc@3.49.0.0", "https://github.com/Willena/sqlite-jdbc-crypt"),
    ("kaitai-struct-runtime", "0.10", "MIT", "pkg:maven/io.kaitai/kaitai-struct-runtime@0.10", "https://github.com/kaitai-io/kaitai_struct_java_runtime"),
    ("slf4j-api", "1.7.36", "MIT", "pkg:maven/org.slf4j/slf4j-api@1.7.36", "https://www.slf4j.org/"),
    ("slf4j-simple", "1.7.36", "MIT", "pkg:maven/org.slf4j/slf4j-simple@1.7.36", "https://www.slf4j.org/"),
    ("apiguardian-api", "1.1.2", "Apache-2.0", "pkg:maven/org.apiguardian/apiguardian-api@1.1.2", "https://github.com/apiguardian-team/apiguardian"),
    ("jackson-databind", "2.18.9", "Apache-2.0", "pkg:maven/com.fasterxml.jackson.core/jackson-databind@2.18.9", "https://github.com/FasterXML/jackson-databind"),
    ("jackson-annotations", "2.18.9", "Apache-2.0", "pkg:maven/com.fasterxml.jackson.core/jackson-annotations@2.18.9", "https://github.com/FasterXML/jackson-annotations"),
    ("jackson-core", "2.18.9", "Apache-2.0", "pkg:maven/com.fasterxml.jackson.core/jackson-core@2.18.9", "https://github.com/FasterXML/jackson-core"),
    ("Eclipse Temurin OpenJDK", "21.0.12", "GPL-2.0-only WITH Classpath-exception-2.0", "pkg:generic/eclipse-temurin@21.0.12", "https://github.com/adoptium/temurin21-binaries"),
    ("Carabiner", "1.2.0", "GPL-2.0-or-later", "pkg:github/Deep-Symmetry/carabiner@v1.2.0", "https://github.com/Deep-Symmetry/carabiner/tree/v1.2.0"),
    ("Ableton Link", "41d9aa111f702e78b6fbaee9d3e06dda1db6420d", "GPL-2.0-or-later", "pkg:github/Ableton/link@41d9aa111f702e78b6fbaee9d3e06dda1db6420d", "https://github.com/Ableton/link/tree/41d9aa111f702e78b6fbaee9d3e06dda1db6420d"),
    ("gflags", "e171aa2d15ed9eb17054558e0b3a6a413bb01067", "BSD-3-Clause", "pkg:github/gflags/gflags@e171aa2d15ed9eb17054558e0b3a6a413bb01067", "https://github.com/gflags/gflags/tree/e171aa2d15ed9eb17054558e0b3a6a413bb01067"),
    ("ASIO standalone", "c465349fa5cd91a64bb369f5131ceacab2c0c1c3", "BSL-1.0", "pkg:github/chriskohlhoff/asio@c465349fa5cd91a64bb369f5131ceacab2c0c1c3", "https://github.com/chriskohlhoff/asio/tree/c465349fa5cd91a64bb369f5131ceacab2c0c1c3"),
    ("SQLCipher amalgamation", "libsqlite3-sys-0.38.1-bundle", "BSD-3-Clause", "pkg:generic/sqlcipher@libsqlite3-sys-0.38.1-bundle", "https://crates.io/crates/libsqlite3-sys/0.38.1"),
    ("OpenSSL", "3.6.3", "Apache-2.0", "pkg:generic/openssl@3.6.3", "https://github.com/openssl/openssl/tree/openssl-3.6.3"),
]

for name, component_version, license_value, purl, download_location in additional_components:
    key = f"external-{name}-{component_version}"
    identifier = "SPDXRef-Package-" + hashlib.sha256(key.encode()).hexdigest()[:16]
    packages.append({
        "SPDXID": identifier,
        "name": name,
        "versionInfo": component_version,
        "downloadLocation": download_location,
        "filesAnalyzed": False,
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": license_value,
        "copyrightText": "NOASSERTION",
        "externalRefs": [{
            "referenceCategory": "PACKAGE-MANAGER",
            "referenceType": "purl",
            "referenceLocator": purl,
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
