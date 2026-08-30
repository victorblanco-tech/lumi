#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(dirname "$script_dir")"
toolchain_home="${LUMI_PACKAGING_JAVA_HOME:-$repository_root/build/package-toolchains/temurin-21-macos-aarch64/Contents/Home}"
maven_repository="$repository_root/build/maven-repository"
output_directory="$repository_root/build/java-runtime-sources"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/lumi-java-sources.XXXXXX")"
dependency_list="$temporary_root/dependencies.txt"
source_archive_name="Lumi-Pro-DJ-Link-Java-dependencies-complete-source.tar.gz"

cleanup() {
  rm -rf "$temporary_root"
}
trap cleanup EXIT

if [[ ! -x "$toolchain_home/bin/java" ]]; then
  echo "ERROR: the pinned Java 21 packaging toolchain is missing." >&2
  exit 1
fi
if ! command -v mvn >/dev/null 2>&1; then
  echo "ERROR: Maven is required to acquire Java dependency sources." >&2
  exit 1
fi

JAVA_HOME="$toolchain_home" mvn \
  --batch-mode \
  --no-transfer-progress \
  -Dmaven.repo.local="$maven_repository" \
  --file "$repository_root/bridges/prolink/pom.xml" \
  dependency:resolve-sources \
  -DincludeScope=runtime

JAVA_HOME="$toolchain_home" mvn \
  --batch-mode \
  --no-transfer-progress \
  -Dmaven.repo.local="$maven_repository" \
  --file "$repository_root/bridges/prolink/pom.xml" \
  dependency:list \
  -DincludeScope=runtime \
  -DexcludeTransitive=false \
  -DoutputAbsoluteArtifactFilename=true \
  -DoutputFile="$dependency_list" \
  -DappendOutput=false

bundle_root="$temporary_root/java-runtime-sources"
install -d "$bundle_root/source-jars"
manifest="$bundle_root/MANIFEST.txt"
{
  echo "Lumi Pro DJ Link Java runtime dependency sources"
  echo
  echo "Generated from bridges/prolink/pom.xml."
  echo "Each source JAR is the preferred source artifact published for the exact"
  echo "runtime dependency version included in the Lumi bridge."
  echo
} > "$manifest"

dependency_count=0
while IFS= read -r line; do
  [[ "$line" == *':compile:'*'.jar'* ]] || continue
  coordinate="$(printf '%s' "$line" | sed -E 's/^[[:space:]]*([^:]+:[^:]+):jar:([^:]+):compile:.*/\1:\2/')"
  binary_jar="$(printf '%s' "$line" | sed -E 's/^[[:space:]]*[^:]+:[^:]+:jar:[^:]+:compile:([^ ]+).*/\1/')"
  source_jar="${binary_jar%.jar}-sources.jar"
  if [[ ! -f "$source_jar" ]]; then
    echo "ERROR: no source JAR was resolved for $coordinate." >&2
    exit 1
  fi
  source_name="$(basename "$source_jar")"
  install -m 644 "$source_jar" "$bundle_root/source-jars/$source_name"
  echo "$coordinate  $(shasum -a 256 "$source_jar" | awk '{print $1}')  $source_name" >> "$manifest"
  dependency_count=$((dependency_count + 1))
done < "$dependency_list"

if [[ "$dependency_count" -ne 13 ]]; then
  echo "ERROR: expected 13 Java runtime source artifacts, found $dependency_count." >&2
  exit 1
fi

COPYFILE_DISABLE=1 tar \
  -czf "$temporary_root/$source_archive_name" \
  -C "$temporary_root" \
  "$(basename "$bundle_root")"

install -d "$output_directory"
install -m 644 "$temporary_root/$source_archive_name" \
  "$output_directory/$source_archive_name"

remote_tea_source="$maven_repository/org/acplt/remotetea/remotetea-oncrpc/1.1.4/remotetea-oncrpc-1.1.4-sources.jar"
license_extract="$temporary_root/remote-tea-license"
install -d "$license_extract"
(
  cd "$license_extract"
  "$toolchain_home/bin/jar" xf "$remote_tea_source" META-INF/LICENSE.txt
)
install -m 644 "$license_extract/META-INF/LICENSE.txt" \
  "$output_directory/Remote-Tea-LGPL-2.0.txt"

echo "Prepared Java runtime corresponding source:"
echo "  $output_directory/$source_archive_name"
