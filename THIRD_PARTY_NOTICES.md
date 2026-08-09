# Third-party notices

Lumi interoperates with independently installed products. They are not bundled
with the Lumi application or disk image.

## Beat Link Trigger

Beat Link Trigger is an independent Deep Symmetry project licensed under the
Eclipse Public License 2.0. Lumi can receive its explicitly configured MIDI
output through a virtual MIDI port.

- Project: https://github.com/Deep-Symmetry/beat-link-trigger
- License: https://github.com/Deep-Symmetry/beat-link-trigger/blob/master/LICENSE

Beat Link Trigger is a temporary optional integration and is not bundled with
Lumi.

## beat-link

The direct Pro DJ Link bridge uses Deep Symmetry's `beat-link` Java library as
a pinned build dependency. The library repository publishes its source under
the Eclipse Public License 2.0. Before a Lumi package containing the bridge is
distributed, the exact pinned binary, source, transitive licenses and Maven
license metadata are verified and included in the release inventory.

- Project: https://github.com/Deep-Symmetry/beat-link
- License: https://github.com/Deep-Symmetry/beat-link/blob/main/LICENSE.md
- Source distribution: https://github.com/Deep-Symmetry/beat-link/releases

## rekordbox-pdb

Lumi uses the `rekordbox-pdb` Rust parser to read the classic Rekordbox Device
Library database. Lumi's adapter exposes no write operation and opens media
files read-only.

- Project: https://github.com/fragmede/rekordbox-pdb-rs
- License: MIT

## Crate Digger

The development-only Pro DJ Link simulator uses Deep Symmetry's `crate-digger`
Java library to parse the mounted Rekordbox DeviceSQL database and ANLZ beat
grid. It is not part of the production Lumi DMG. Maven metadata for the pinned
0.2.1 artifact identifies EPL-1.0; its exact source and license are retained in
the development-tool release inventory.

- Project: https://github.com/Deep-Symmetry/crate-digger
- Artifact: `org.deepsymmetry:crate-digger:0.2.1`

## Product names

Rekordbox, SoundSwitch, Control One and other product names are used only to
describe optional interoperability. Their names and marks belong to their
respective owners. Lumi is not affiliated with or endorsed by those owners.
