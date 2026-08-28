# E6-07 – Explicit Theme Strategy

Status: **Done** | Priority: **P0** | Effort: **5**

## User value

As a user whose SoundSwitch Banks represent visual Themes, I can explicitly
configure how Lumi selects one coherent Theme for a complete track and how it
rotates Themes across consecutive current and next-track plans.

## Acceptance criteria

- The built-in SoundSwitch profile states explicitly that Banks 1–4 are Lumi
  Themes and preserves the user's Bank names as Theme names.
- Automatic planning selects exactly one base Theme before playback; automatic
  phrase changes never cross Themes.
- A deliberate Live override may change Theme from a future phrase onward.
- Every Theme can be enabled, weighted and configured as Track Color `Neutral`,
  `Prefer` or `Only` using the actual Rekordbox OneLibrary color catalog.
- `Prefer` increases a matching Theme's effective weight while retaining a safe
  fallback; matching `Only` Themes form the exclusive eligible set.
- A non-matching `Only` Theme is never used as a last-resort fallback, while an
  absent Track Color still allows Neutral and Prefer Themes to participate.
- Automatic planning prefers the eligible Themes with the best exact Phrase
  Role coverage and requires an exact opening mapping.
- A later unmapped Phrase Role becomes a visible provider no-op: Lumi sends no
  replacement command and the active AutoLoop continues. It never substitutes
  another Phrase Role or crosses Themes.
- Theme cooldown includes committed and reserved next-track plans. If every
  otherwise valid Theme is recent, the restriction is relaxed deterministically.
- Automatic Plan Preview uses the same Theme Strategy as runtime and exposes the
  selected Bank, Theme and reason. A manual Preview Theme remains an explicit
  override.
- Existing policies deserialize with their pre-feature behavior until the user
  explicitly saves Theme Strategy; no Bank, AutoLoop or modifier mapping is lost.
- Theme evaluation remains compile-time only and cannot enter Pro DJ Link,
  Ableton Link or realtime MIDI lanes.

## Verification

- planner coverage for one-Theme-per-track, full cooldown windows and color-only
  exclusion;
- policy compatibility coverage for stored pre-Theme-Strategy JSON;
- Swift decoder, payload and workspace coverage;
- existing canonical engine/output transcripts remain unchanged before policy
  activation.
