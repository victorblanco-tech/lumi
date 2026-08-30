# Public Beta readiness

Use this gate before changing the Lumi repository from private to public or
publishing a binary as a public-beta release.

## Product and communication

- [x] README and documentation identify Lumi as Public Beta.
- [x] Field-test scope, fallback expectations and safe issue-reporting guidance
      are documented.
- [x] The reference hardware setup is described as evidence, not as a universal
      compatibility claim.
- [ ] GitHub release title and notes identify the binary as Public Beta.

## Source and repository hygiene

- [x] Current-tree and full-history scans found no committed access token,
      private key or bearer-token URL.
- [x] Production artifacts contain no user database, music, USB database,
      SoundSwitch project or personalized mapping.
- [x] Historical local filesystem examples are replaced with portable commands.
- [ ] The repository default branch is changed from `dev` to `main` before the
      visibility switch.
- [ ] `main` protection and private vulnerability reporting are enabled.

## Licensing and redistribution

- [x] Lumi source carries EPL-2.0 plus a separate trademark notice.
- [x] Third-party notices identify the actual production runtime components and
      their licenses.
- [x] Carabiner remains a separately executed helper behind a loopback protocol;
      it is not linked into the EPL-2.0 Lumi binaries.
- [x] The packager includes the exact Carabiner/Ableton Link GPL license text
      and complete corresponding source, including submodules.
- [x] The packager includes the Remote Tea LGPL-2.0 notice and the exact source
      artifacts for every Java bridge runtime dependency.
- [x] The SPDX release inventory covers Rust, Java, the Java runtime,
      SQLCipher/OpenSSL and the Carabiner/Ableton Link helper.
- [ ] A clean public-beta DMG built after these changes passes the release gate.

A disposable `0.6.0-dev-1` DMG passed the complete packaging and mounted-image
verification after these gates were implemented. A new production artifact
must still be built and published; the existing private `0.5.0` DMG predates
the complete source packaging.

The unchecked licensing items are public-distribution blockers. The existing
private 0.5.0 artifact may be used for the owner's local testing, but repository
visibility must remain private until a compliant replacement artifact is ready.
This checklist is an engineering compliance record, not formal legal advice.
