---
layout: default
title: Lumi privacy
description: Privacy information for Lumi and Lumi Remote.
---

# Privacy

Lumi and Lumi Remote are local-first applications. Victor Blanco / VB Tech does
not operate a Lumi account service, cloud relay, advertising network or usage
analytics service, and the apps do not send your library or show data to the
developer.

## Data used locally

Lumi on macOS stores the library metadata, waveform and beatgrid analysis, Lumi
phrases, Light Plans, integration configuration and trusted-device information
needed for the workflows selected by the user. Music files and rekordbox media
remain under the user's control.

Lumi Remote communicates directly with a paired Lumi Mac on the same local
network. Its Live projection can contain track metadata, RGB waveform data,
Hot Cues, phrases, Light Plans and integration status. The iPhone stores its
pairing credential in Apple Keychain. The Mac stores only the corresponding
credential verifier and paired-device information.

## Data collection

The applications contain no advertising SDK, third-party analytics SDK or
developer-operated telemetry upload. Lumi Remote does not use location,
contacts, photos, microphone or camera data; the system Camera app may be used
to scan a one-use pairing QR code.

Apple may collect App Store, TestFlight or crash information according to the
user's Apple privacy and diagnostics settings. A user may separately choose to
send information through GitHub Issues or another support channel; that
submission is governed by the selected service and should never include music,
USB databases, credentials, QR codes, tokens or private show files.

## Security and deletion

Remote sessions use TLS with a certificate pinned during one-use pairing.
Paired-device access can be revoked from Lumi on the Mac. Removing Lumi Remote
from the iPhone removes its local app data; Lumi's macOS data-management tools
provide explicit backup and library reset workflows.

Security issues should be reported privately through the repository's
[security advisory form](https://github.com/victorblanco-tech/lumi/security/advisories/new).

Last updated: 3 September 2026.
