# Lumi Remote 0.1.0 Public Beta

Lumi Remote is the native iPhone booth companion for Lumi. It opens directly
into Live Decks and keeps the current Master and prepared next Player visible
without putting the phone in any show-critical execution path.

<p align="center">
  <img src="https://raw.githubusercontent.com/victorblanco-tech/lumi/lumi-remote-v0.1.0/docs/assets/screenshots/lumi-remote-live-device.png" alt="Lumi Remote on iPhone showing two live Players and their Light Plans" width="390">
</p>

## Included

- portrait Master-first and landscape side-by-side Player layouts;
- smooth, pinch-zoomable RGB waveform with beatgrid and Hot Cues;
- proportional Lumi phrase and Light Plan timelines with **Active** and **Next**
  emphasis;
- real Player number, hardware model, track metadata, BPM and remaining time;
- persistent Pro DJ Link, Light Output and Ableton Link health;
- `Off`, `Arm`, `Start` and `Pause`, Link and timing-offset controls;
- future-phrase Theme, AutoLoop and lock changes for the current Controller;
- local Bonjour discovery, pinned-TLS pairing, Keychain credentials and
  reconnect after foregrounding;
- one Controller plus additional view-only paired devices.

## Safety boundary

Lumi on the Mac remains authoritative. The Remote Gateway is independently
supervised and cannot sit in the Pro DJ Link, SoundSwitch MIDI or Ableton Link
paths. Losing the iPhone connection queues no command and does not interrupt the
show.

## Public beta limitations

- iOS 18 or newer and Lumi 0.6.0 or newer are required;
- Mac and iPhone must be on the same non-isolated local network;
- Local Playback, Library, USB Sync and Track Editor remain Mac-only;
- internet and cloud relay are not supported;
- this first beta still needs broader device, Wi-Fi and multi-phone field
  coverage.

Install the physical-iPhone beta from source with Xcode and your own Apple
Account by following the
[iPhone installation guide](https://github.com/victorblanco-tech/lumi/blob/lumi-remote-v0.1.0/docs/user-guide/iphone-remote.md#install-the-current-beta-with-xcode).
Free Personal Team provisioning expires after seven days. There is no
TestFlight invitation yet; the optional GitHub Simulator artifact is for
validation and is not installable on an iPhone.

Read the [Lumi Remote guide](https://github.com/victorblanco-tech/lumi/blob/lumi-remote-v0.1.0/docs/user-guide/iphone-remote.md),
[privacy information](https://github.com/victorblanco-tech/lumi/blob/lumi-remote-v0.1.0/docs/privacy.md) and
[public beta guidance](https://github.com/victorblanco-tech/lumi/blob/lumi-remote-v0.1.0/docs/public-beta.md)
before testing a live setup.
