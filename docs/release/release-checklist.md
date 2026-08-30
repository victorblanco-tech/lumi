# Lumi production release checklist

Use this checklist for a production release distributed through GitHub.

## 1. Freeze the release

- [ ] Choose the production version and freeze its scope.
- [ ] Confirm that `dev` contains every intended fix and no unrelated work.
- [ ] Create a checked backup of the Dev database and configuration.
- [ ] Change `VERSION` and all derived product versions to `X.Y.Z` without a
      prerelease suffix.
- [ ] Update `CHANGELOG.md`, release notes and known limitations.
- [ ] Document every database or configuration migration.

## 2. Verify the product

- [ ] `./scripts/verify.sh` passes locally.
- [ ] `./scripts/verify-security.sh` passes locally.
- [ ] Pro DJ Link simulator regressions pass.
- [ ] The complete physical reference chain passes: both players, mixer,
      SoundSwitch, Control One and DMX output.
- [ ] Cold start, master handoff, Pause/Start, Hot Cue, Beat Jump and live Theme
      edit behave correctly.
- [ ] Ableton Link follows the master BPM and exactly one peer remains active.
- [ ] AutoLoop and verified Static Look actions remain on time and exactly once.
- [ ] USB reconnect, changed beatgrid review and selected-playlist sync pass.
- [ ] Backup, restore and clean library rebuild pass with a copied dataset.
- [ ] Lumi closes cleanly without leaving the engine, MIDI or Link in a broken
      state.

## 3. Verify the public repository

- [ ] README, user guide, screenshots and system requirements match the build.
- [ ] License, trademark notice, third-party notices, contribution guide and
      security policy are present.
- [ ] No music, real USB databases, SoundSwitch projects, credentials, tokens,
      local backups, DMGs or private screenshots are tracked.
- [ ] Dependency and Git-history scans show no release blocker.
- [ ] GitHub description and topics are current.
- [ ] `main` is selected as the public default branch.
- [ ] Branch protection prevents direct changes to `main`.
- [ ] Private vulnerability reporting and Dependabot alerts are enabled.

## 4. Build the production artifact

- [ ] Build the production DMG from the exact release commit.
- [ ] Confirm the app is Apple Silicon-only and targets macOS 15 or newer.
- [ ] Verify the production bundle identifier and isolated production database.
- [ ] Install through the DMG into `/Applications/Lumi/Lumi.app` on a clean test
      account.
- [ ] Complete and verify the documented unsigned-app `Open Anyway` path.
- [ ] Create and independently compare the SHA-256 checksum.
- [ ] Generate the software bill of materials and archive the local build log.

Paid Developer ID signing and notarization are optional for the first public
release, but the release notes must clearly state when the build is unsigned and
not notarized.

## 5. Publish

- [ ] Merge the verified release commit from `dev` to `main` without squashing
      away its ancestry.
- [ ] Confirm the `main` commit matches the verified source revision.
- [ ] Create tag `vX.Y.Z` on that exact `main` commit.
- [ ] Create the GitHub Release with release notes, DMG and checksum.
- [ ] Download the published assets and repeat checksum and installation smoke
      tests.
- [ ] Make the repository public only after the release assets and user guide
      are ready together.
- [ ] Publish GitHub Pages from `main` and `/docs`.
- [ ] Set the Pages URL as the repository homepage.

## 6. Continue development

- [ ] Synchronize `main` back into `dev`.
- [ ] Change `dev` to the next planned `X.Y.Z-dev-1` version.
- [ ] Keep the previous DMG and checksum available for rollback.
- [ ] Monitor installation problems and show-critical reports before starting a
      large new feature.
