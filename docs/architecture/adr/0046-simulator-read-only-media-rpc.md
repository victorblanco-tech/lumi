# ADR 0046 — Unprivileged, destination-aware simulator media RPC

Date: 2026-10-09. Status: implemented for simulator 0.4.1-dev-5; two-Mac hardware acceptance pending.

## Decision

The simulator must exercise the production `MediaIdentityProbeMain` without a
trusted-result injection, HTTP marker shortcut, hardcoded LAN address, root
process, privileged installation helper, Docker change or system NFS export.

One ordinary child process binds wildcard UDP 111 and dispatches the narrow
portmapper GETPORT, MOUNT v1 `/C/`, NFS v2 LOOKUP and READ operations used by the
production reader. RPC program/version multiplexing permits the same socket
to serve all three programs. `IP_PKTINFO` provides the destination address;
reply metadata explicitly selects that same source address. macOS permits
this wildcard bind without privileged-port entitlement. This has been tested
as UID 501 on the development Mac, including the unmodified production reader.
No system setting is changed to make the bind work. A port conflict fails
visibly and never stops an existing service.

The server accepts only configured destination IPs and private/link-local
or loopback clients. Packets and credentials are length-bounded, throughput
is capped, and only `.lumi-media.json` can be read (1–4096 bytes, no symlinks,
regular files only). There is no write, directory listing or registration API.
This is an unauthenticated LAN protocol like the CDJ service: use only trusted
networks. Wildcard listening is broader than per-IP sockets; destination
allowlisting is mandatory and covered by tests.

The child retains read-only directory descriptors for the selected USB roots.
Each insertion receives fresh opaque filehandles. Eject invalidates those
handles, not the already loaded Player track. Marker replacement invalidates
old filehandles, allowing the production reader's recheck to reject stale data.
Missing/timeout/malformed marker faults are opt-in, capped at 30 seconds and
do not affect the independent realtime traffic scheduler.

The Java parent synchronizes slot generations on an independent 25 ms worker;
this does not repeatedly read USB files. RPC requests perform bounded reads.
Closing stdin, terminating the parent or normal shutdown ends the child.
No launch agent/daemon, root service or permanent software is installed.

## Network topology

At startup, active private/link-local IPv4 broadcast endpoints are enumerated.
Primary selection follows the macOS default interface; no `en0`/`en1` or user
IP is compiled in. A different interface/address with the same broadcast
network can supply Player 2. This is a candidate topology, not proof against
AP client isolation; the cross-Mac reachability test remains required.

With one endpoint, both Players can transmit and Player 2 loads from Player 1
over LINK. Only one physical USB source is allowed. With two endpoints, each
Player transmits from its own address and can own an independent USB.
UI and authenticated status expose the selected addresses and limitation.

Address ownership is fixed for the lifetime of a running session. A lost
address is not silently reassigned to another Player. Recovery on the same
address can resume; a changed address requires stopping and starting the
simulator to rediscover the topology. Hot topology migration is not claimed
in this build; retaining source identity safely takes precedence.

## Evidence and remaining acceptance

Native tests exercise exact bytes, destination/source pairing, cross-USB
handle rejection, eject/reinsert, replacement, missing/symlink/oversize files,
bounded reads, write rejection, timeout/recovery and the unmodified production
reader on port 111. Fixtures are explicitly synthetic and disposable.
Java tests cover packet USB states, LINK source retention and network selection.
UI acceptance uses a test-only harness (not shipped) and the actual control page.

The development Mac has one active LAN interface and no attached export.
Two physical interfaces on the Mac mini, its real export and downstream Lumi
show acceptance must be tested after installation. Local loopback + actual
host-LAN tests are not presented as that two-machine acceptance.
