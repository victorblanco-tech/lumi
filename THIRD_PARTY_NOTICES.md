# Third-party notices

Lumi interoperates with independently installed DJ and lighting products and
also bundles the pinned open-source runtime components listed below. The
independently installed commercial products themselves are not included in the
Lumi application or disk image.

## beat-link

The direct Pro DJ Link bridge uses Deep Symmetry's `beat-link` Java library as
a pinned runtime dependency. Version 8.0.0 and the related Deep Symmetry
libraries `electro` 0.1.4 and `crate-digger` 0.2.1 declare the Eclipse Public
License 1.0. They are bundled in Lumi's Java Pro DJ Link bridge; Beat Link
Trigger itself is neither required nor bundled.

- Project: https://github.com/Deep-Symmetry/beat-link
- License: https://github.com/Deep-Symmetry/beat-link/blob/v8.0.0/LICENSE.md
- Source distribution: https://github.com/Deep-Symmetry/beat-link/releases
- Crate Digger: https://github.com/Deep-Symmetry/crate-digger

The bridge also contains these pinned runtime dependencies:

- Remote Tea ONC/RPC 1.1.4 — LGPL-2.0;
- Apache Commons Math 3.6.1 — Apache-2.0;
- SQLite JDBC 3.49.0.0 — Apache-2.0;
- Kaitai Struct Runtime 0.10 — MIT;
- SLF4J 1.7.36 — MIT;
- API Guardian 1.1.2 — Apache-2.0;
- Jackson Core, Annotations and Databind 2.18.9 — Apache-2.0.

## Carabiner and Ableton Link

Lumi packages a pinned Carabiner executable as a separately executed helper for
publishing the Lumi-owned timing authority to an Ableton Link session. Lumi
communicates with the helper only over its documented loopback TCP protocol;
Beat Link Trigger is not required. Carabiner and the included Ableton Link code
are distributed under GPL-2.0-or-later. A public Lumi binary distribution must
include their license text and the complete corresponding source for the exact
pinned build, including source submodules. Provenance and checksums alone do not
replace that source obligation.

- Carabiner: https://github.com/Deep-Symmetry/carabiner/tree/v1.2.0
- Carabiner license: https://github.com/Deep-Symmetry/carabiner/blob/v1.2.0/LICENSE.md
- Ableton Link: https://github.com/Ableton/link
- Ableton Link license: https://github.com/Ableton/link/blob/master/LICENSE.md

## OneLibrary and SQLCipher

Lumi reads current Rekordbox OneLibrary USB databases through a bundled
SQLCipher build. The database is opened read-only. The OneLibrary schema and
shared format-key implementation were validated against the MIT-licensed
`onelibrary-connect` project; no runtime package from that project is bundled.

- OneLibrary reference: https://github.com/chrisle/onelibrary-connect (MIT)
- SQLCipher: https://github.com/sqlcipher/sqlcipher (BSD-style license)
- OpenSSL: https://www.openssl.org/source/license.html (Apache License 2.0)

## Bundled Java runtime

The self-contained Pro DJ Link bridge includes a reduced OpenJDK runtime. Its
license and module notices are retained in the runtime's `legal` directory.
OpenJDK is distributed under GPL-2.0 with the Classpath Exception; individual
modules can contain additional notices listed in that directory.

## Local Remote Gateway security and discovery

Lumi's separately supervised local iPhone Remote Gateway bundles Rust crates
for encrypted transport, certificate generation and Bonjour discovery. The
direct dependencies are:

- rustls 0.23.43 — Apache-2.0 OR ISC OR MIT;
- tokio-rustls 0.26.4 — MIT OR Apache-2.0;
- rcgen 0.14.10 — MIT OR Apache-2.0;
- ring 0.17.14 — Apache-2.0 AND ISC;
- mdns-sd 0.21.1 — Apache-2.0 OR MIT.

Their exact versions and transitive dependencies are pinned by `Cargo.lock`.
The relevant project and license files are distributed by their respective
crates and source repositories.

## Product names

Rekordbox, SoundSwitch, Control One and other product names are used only to
describe optional interoperability. Their names and marks belong to their
respective owners. Lumi is not affiliated with or endorsed by those owners.
