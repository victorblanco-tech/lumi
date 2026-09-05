# Lumi Remote 0.1.1 — Public Beta

The iPhone companion to **Lumi 0.6.2 for Mac**. Follow both Players and adjust
future lighting choices from the booth while the Mac runs the show.

<p align="center">
  <img src="https://raw.githubusercontent.com/victorblanco-tech/lumi/lumi-remote-v0.1.1/docs/assets/screenshots/lumi-remote-live-device.png" alt="Lumi Remote with two Players and their Light Plans" width="390">
</p>

## What's changed

- Choose the actual Lumi **Phrase Type** for a future phrase, alongside Theme
  and AutoLoop, using the configured model from the Mac.
- Adjust and save the shared lighting offset from **−250 to +250 ms**. The
  picker remains stable during live updates. A running show's change applies
  at the next phrase and remains saved after restarting Lumi.
- See whether this phone is **Controller** or **View only**. Reconnecting or
  changing show mode no longer transfers control to another device.
- Clearer stale/reconnecting states and command failures: controls wait for
  fresh authoritative state instead of accepting unreliable queued changes.

The accepted RGB waveform and live rendering are retained.

## Install on an iPhone

Use **iOS 18+**, **Xcode**, your Apple Account and **Lumi 0.6.2+** on the Mac.
Check out the `lumi-remote-v0.1.1` source tag, select the **Release** Run
configuration, choose your signing team and run on the connected iPhone.

[Follow the complete installation and pairing guide](https://github.com/victorblanco-tech/lumi/blob/lumi-remote-v0.1.1/docs/user-guide/iphone-remote.md#install-the-current-beta-with-xcode).

**There is no TestFlight or App Store build yet.** The attached ZIP is only an
iOS Simulator validation app for a Mac, not an IPA or physical-iPhone installer.
Free Personal Team installations require renewal through Xcode after seven days.
Use the Production Mac app with this Release configuration; Dev/RC channels
remain separate. Update both products to receive the new shared controls.

[Lumi 0.6.2 for Mac](https://github.com/victorblanco-tech/lumi/releases/tag/v0.6.2)
· [Privacy](https://github.com/victorblanco-tech/lumi/blob/lumi-remote-v0.1.1/docs/privacy.md)
