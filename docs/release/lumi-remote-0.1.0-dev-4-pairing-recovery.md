# Lumi Remote 0.1.0-dev-4 – pairing recovery

This iPhone development build makes deliberate re-pairing recover cleanly from
an obsolete credential retained in Keychain.

## Fixed

- A newly scanned, valid invitation now takes precedence over a stored
  credential for the same Lumi installation.
- Unreadable stale Keychain state is bypassed only while that explicit pairing
  invitation is active.
- The replacement credential is written to Keychain only after Mac approval,
  pinned-TLS authentication and a successful pairing response.
- CoreSimulator uses the gateway port advertised in Bonjour TXT metadata with
  an explicit host-loopback endpoint. Physical iPhones continue to use the
  Bonjour service endpoint and normal multi-interface path selection.
- The iOS target declares its Keychain access group, and certificate-pin
  evaluation runs independently from the connection state queue.

## Evidence

- The client-selection regression test supplies both a stale credential and a
  fresh invitation and verifies that only the pairing hello is emitted.
- The complete Remote client and iPhone app build remain part of the fast Apple
  verification gate.
- Headed acceptance paired the signed iPhone Simulator build with a packaged
  Lumi Mac, transferred the single Controller lease, rendered portrait and
  landscape Live views, followed a simulated Player and accepted `ARM`, `START`
  and confirmed `OFF` exactly once.
- Terminating and relaunching the iPhone app reconnected without a new QR scan,
  proving that the scoped credential survived in Keychain.
