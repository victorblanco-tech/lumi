# ADR-0041: Independent product releases inside the Lumi monorepo

- Status: **Accepted**
- Date: **2026-09-02**

## Context

The repository now contains three installable products with different users,
platform gates and delivery rhythms:

- Lumi for macOS, including the engine and Remote Gateway;
- Lumi Remote for iPhone;
- the development-only Pro DJ Link Simulator for a second Mac.

They share contracts and visual foundations, but a simulator fix must not create
a new desktop release and an iPhone UI release must not change the installed
Mac version. Keeping three repositories would make cross-product protocol
changes harder to review atomically and would duplicate licensing, security and
quality infrastructure.

## Decision

All three products remain in this monorepo and have independent version sources,
tags, release notes and GitHub Releases.

| Product | Version source | Tag | Release asset |
|---|---|---|---|
| Lumi for macOS | `/VERSION` | `vX.Y.Z` | macOS DMG, checksum and SBOM |
| Lumi Remote | `/apps/ios/VERSION` | `lumi-remote-vX.Y.Z` | TestFlight/App Store build when enabled |
| Pro DJ Link Simulator | `/tools/prolink-simulator/VERSION` | `prolink-simulator-vX.Y.Z` | self-contained simulator DMG |

Shared libraries and wire contracts do not receive end-user releases. Their
compatibility is represented by an explicit protocol major and tested against
both consuming products. The Remote Gateway ships with the matching Lumi Mac
release because it is a supervised Mac service, not a fourth user product.

Every product uses `X.Y.Z-dev-N`, `X.Y.Z-rc-N` and `X.Y.Z` without Preview,
Debug or named prerelease channels. A product tag triggers only its affected
verification and packaging path. A coordinated protocol change may intentionally
produce separate Mac and iPhone releases from the same commit, each with its own
tag.

Production, RC and Dev keep distinct local identities. Lumi Remote also includes
the paired Mac release channel in discovery and trust state, so a Dev phone build
cannot silently control a Production gateway.

## Consequences

- Shared protocol and UI changes can be atomic and reviewed together.
- Product releases remain understandable and do not force artificial version
  equality.
- Release automation must validate the version associated with the triggering
  tag instead of always reading the root version.
- Release notes and changelogs identify their product explicitly.
- Moving a product to its own repository remains possible later, but is not
  justified while the shared protocol is evolving rapidly.

## Rejected alternatives

### One version for every installable product

Rejected because it couples unrelated release cadence and makes product history
misleading.

### Separate repositories now

Rejected because contract changes would require coordinated cross-repository
pull requests and would fragment security and licensing evidence during beta.
