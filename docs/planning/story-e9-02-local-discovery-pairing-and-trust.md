# Story E9-02: Local discovery, pairing and device trust

- Status: **Implementation complete; physical-device acceptance pending**
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

## Implemented evidence

- release-specific Bonjour service names and minimal validated TXT metadata;
- native iOS Bonjour browser with clear permission/failure states;
- one-use five-minute invitation, matching short code and explicit approval
  registry;
- bounded authentication attempt limiter and maximum paired-device count;
- per-device credential verifier on Mac and channel-scoped Keychain store on
  iPhone;
- release-channel, protocol, certificate-fingerprint and QR payload validation.
- persistent protected Mac installation identity and rustls TLS listener;
- exact leaf-certificate pinning in the native iPhone transport;
- authenticated, protected loopback administration for status, invitation,
  approval, revocation and Controller transfer;
- Mac `Integrations > iPhone Remote` management UI with QR, matching short code,
  paired-device state and explicit service enablement;
- controller ownership persists across gateway restart and every trust mutation
  disconnects existing sessions for mandatory reauthentication.
- headed acceptance completed Bonjour discovery, matching-code QR pairing,
  explicit Mac approval and deliberate Controller transfer in the signed iPhone
  Simulator;
- the approved credential survived app termination and restored the pinned-TLS
  session from the channel-scoped Keychain without a second invitation;
- CoreSimulator uses the explicitly advertised ephemeral port over host
  loopback, while physical iPhones keep the normal Bonjour service endpoint.

## Remaining gate

Validate the complete flow on a physical iPhone, including Local Network
permission, client-isolated Wi-Fi failure, certificate mismatch, revocation and
two-device Controller transfer. Discovery never enables controls by itself.
