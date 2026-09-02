# Story E9-02: Local discovery, pairing and device trust

- Status: **Planned**
- Priority: **P0 security**
- Target: `0.6.0-dev`
- Components: Remote Gateway, macOS Integrations, iOS Remote Client

## User outcome

The DJ can deliberately pair an iPhone on the local booth network, see which
device has control and revoke it without accounts or internet.

## Scope

- advertise and browse `_lumi-remote._tcp` through Bonjour;
- add clear Local Network permission copy;
- establish TLS with installation-certificate pinning;
- implement one-use expiring QR invitations and matching short code;
- require explicit Mac approval;
- store per-device credentials in Keychain and revocable Mac trust state;
- expose last seen, permission and revoke/transfer actions;
- rate-limit discovery metadata, pairing and authentication failures.

## Acceptance

- discovery alone reveals no track or user data;
- an unpaired or revoked phone cannot request state or issue commands;
- replaying an invitation or credential after revocation fails;
- a certificate or release-channel mismatch never silently reconnects;
- pairing and reconnect work on a physical iPhone with Wi-Fi and fail clearly on
  a client-isolated network.

