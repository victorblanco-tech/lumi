# ADR-0036: Phrase Role-owned visual identity

Status: **Accepted**  
Date: **2026-08-22**

## Context

Phrase colors appeared in the Track Editor, Live Decks, Light Plans and the
SoundSwitch mapping surfaces, but those clients historically chose their own
hard-coded colors. A Phrase Role is a stable Lumi-owned concept and its visual
identity must therefore remain the same in every workflow and after restart.

## Decision

Each `PhraseRole` owns one revisioned 24-bit sRGB `colorRgb` value. The Rust
engine is the single writer and persists the value in the Library database.
`Settings > Phrase Model` is the only editing surface. A color mutation uses the
same optimistic catalog revision as rename, reorder and archive operations.

The authoritative engine snapshot publishes the color with each role. Native
clients derive one `LumiPhraseColorPalette` from that snapshot and inject it
into every phrase-aware presentation:

- Phrase Model settings;
- Track Editor detail and overview lanes;
- Live Deck phrase lanes and the phrase marker on AutoLoop plan segments;
- Light Plans role and preview rows;
- SoundSwitch Banks & AutoLoops and Virtual Controller mappings.

Built-in roles receive recognizable defaults during the schema-v15 migration.
Custom roles receive a neutral teal default. Unknown or legacy snapshot roles
also render with that safe fallback. Track colors and Hot Cue colors remain
separate source-owned concepts.

## Consequences

- changing a role color updates every connected screen after the authoritative
  snapshot refresh;
- no feature may introduce a local `roleID -> Color` switch;
- role rename, source sync and phrase-timeline edits never alter the color;
- backups naturally include phrase colors because they contain the Library
  database;
- the color is presentation metadata only and cannot change planning or
  realtime lighting execution.

