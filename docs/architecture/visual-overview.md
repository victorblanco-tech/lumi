# Lumi – functionele architectuurplaten

Status: **Accepted baseline**
Datum: **2026-08-02**

Deze platen tonen achtereenvolgens het totale DJ- en lichtlandschap, de interne
werking van Lumi en het functionele gat dat Lumi invult.

## Plaat 1 – Totale oplossing

```mermaid
flowchart TB
  subgraph preparation["Voorbereiding"]
    direction LR
    RB["Rekordbox library<br/>beatgrids, phrases, kleuren"]
    EXPORT["USB / metadata-export"]
    RB --> EXPORT
  end

  subgraph booth["DJ booth – PRO DJ LINK"]
    direction LR
    DECKA["Deck A<br/>current / live"]
    DECKB["Deck B<br/>loaded / next"]
    MIXER["DJ mixer<br/>on-air / master-context"]
    LAN[("PRO DJ LINK LAN")]
    PA["Audio / PA"]

    DECKA ==>|"audio"| MIXER
    DECKB ==>|"audio"| MIXER
    MIXER ==>|"master audio"| PA
    DECKA --- LAN
    DECKB --- LAN
    MIXER --- LAN
  end

  subgraph mac["Mac – timing, planning en licht"]
    direction LR
    BLT["Beat Link Trigger<br/>+ ingebouwde Carabiner"]
    LINK[("Ableton Link session")]
    LUMI[["Lumi Engine<br/>Live + Next Lighting Plans"]]
    MACUI["Lumi voor macOS"]
    IAC["Virtuele MIDI-output"]
    SS[["SoundSwitch<br/>banks, Autoloops, Static Looks"]]

    BLT ==>|"tempo + beat + bar"| LINK
    LINK ==>|"continue sync"| SS
    BLT -->|"deck-, track-, beat- en statusdata"| LUMI
    MACUI <-->|"lokale IPC"| LUMI
    LUMI -->|"bank + scene op phrasegrens"| IAC
    IAC --> SS
  end

  PHONE["Lumi Remote<br/>native iPhone-app"]
  CONTROL["Fysieke controller<br/>Control One of andere MIDI-controller"]

  subgraph output["SoundSwitch-outputdomein"]
    direction LR
    DMX["DMX-interface<br/>bijv. Control One"]
    FIXTURES["Fixtures / lichtshow"]
    DMX -->|"DMX"| FIXTURES
  end

  EXPORT -->|"muziek naar decks"| DECKA
  EXPORT -->|"muziek naar decks"| DECKB
  EXPORT -->|"lokale metadata"| LUMI
  LAN --> BLT
  PHONE <-->|"versleuteld lokaal wifi/LAN"| LUMI
  CONTROL -->|"directe handmatige override"| SS
  SS --> DMX
```

De twee paden naar SoundSwitch zijn parallel en beide noodzakelijk:

- **Ableton Link** houdt BPM, beat en bar continu gelijk met de PRO DJ LINK-
  spelers;
- **Lumi MIDI** kiest op phrasegrenzen welke bank, Autoloop of gemanagede Static
  Look SoundSwitch moet uitvoeren.

DMX-interface en fixtures vallen volledig binnen het SoundSwitch-domein. Lumi
hoeft niet te weten of de DMX-output via Control One of een andere interface
loopt.

## Plaat 2 – Lumi specifiek

```mermaid
flowchart TB
  subgraph clients["Native clients"]
    MACAPP["Lumi macOS<br/>SwiftUI"]
    IOSAPP["Lumi Remote<br/>SwiftUI / iPhone"]
  end

  subgraph engine["lumi-engine – Rust LaunchAgent"]
    API["Versiegebonden Control API<br/>commands, snapshots, events"]

    subgraph inputs["Deck source providers"]
      SIM["Simulator"]
      REPLAY["Replay"]
      LIVE["DeckSourceProvider<br/>Beat Link eerst; native/anders later"]
    end

    META["Rekordbox library-import"]

    QUEUE[("Begrensde eventqueue")]
    REDUCER[["Single-writer reducer<br/>centrale runtime-state"]]

    subgraph planning["Planning Engine"]
      direction LR
      MATCH["Track matching"]
      RULES["Theme, kleur, rotatie<br/>en variation rules"]
      PREFLIGHT["Preflight en fallbacks"]
    end

    PLANS[("Lighting Plan Store<br/>ACTIVE + NEXT per deck")]
    LEADER["Lighting leader"]

    subgraph execution["Execution Engine"]
      direction LR
      GATE{"Operationele gate<br/>OFF / ARMED / LIVE / PAUSED"}
      BOUNDARY["Phrase-boundary lookup"]
      SCENE["Self-contained ApplyScene<br/>bank → delay → Autoloop"]
    end

    OUTPUT["LightingOutputProvider<br/>SoundSwitch MIDI eerst"]
    PROFILE["Targetprofiel<br/>semantische actie → MIDI"]
    TRANSPORT["MidiTransportProvider<br/>CoreMIDI eerst"]
    LOGS[("Config, revisions,<br/>sessies en logs")]

    SIM --> QUEUE
    REPLAY --> QUEUE
    META --> QUEUE
    LIVE --> QUEUE
    API -->|"user commands"| QUEUE
    QUEUE --> REDUCER
    REDUCER --> MATCH
    REDUCER --> LEADER
    MATCH --> RULES
    RULES --> PREFLIGHT
    PREFLIGHT -->|"READY plan"| PLANS
    LEADER -->|"activeert passend plan"| PLANS
    PLANS --> BOUNDARY
    REDUCER --> GATE
    GATE --> BOUNDARY
    BOUNDARY --> SCENE
    SCENE --> OUTPUT
    OUTPUT --> PROFILE
    PROFILE --> TRANSPORT
    TRANSPORT -->|"effectresultaat"| QUEUE
    REDUCER <--> LOGS
    PLANS <--> LOGS
    REDUCER -->|"state + plan events"| API
  end

  SS["SoundSwitch MIDI-input"]

  MACAPP <-->|"lokale IPC"| API
  IOSAPP <-->|"Bonjour + pairing + TLS op LAN"| API
  TRANSPORT -->|"virtuele MIDI-poort"| SS
```

De Planning Engine doet het creatieve werk vooraf. De Execution Engine voert in
`LIVE` alleen een reeds gevalideerde cue uit. UI's, source-adapters en
outputproviders muteren nooit rechtstreeks de centrale state. Beat Link en
SoundSwitch zijn de eerste providerimplementaties en geen dependencies van de
corecontracten.

## Plaat 3 – De Lumi-usecase: het ontbrekende stuk

```mermaid
flowchart TB
  subgraph without["Zonder Lumi"]
    direction LR
    NEXT1["Andere deck<br/>volgende track is al geladen"]
    PHRASES1["Rekordbox / PRO DJ LINK<br/>track- en phrasecontext"]
    LINK1["Ableton Link<br/>BPM + beat + bar"]
    LIBRARY1["SoundSwitch<br/>banks en Autoloops"]
    GAP{{"Ontbrekende beslislaag<br/>welk theme en welke scene<br/>hoort bij de volgende phrase?"}}
    HUMAN["DJ moet vooruitdenken<br/>en handmatig kiezen"]
    SS1["SoundSwitch voert uit"]

    NEXT1 --> PHRASES1
    PHRASES1 -.-> GAP
    LIBRARY1 -.-> GAP
    GAP -.-> HUMAN
    HUMAN -->|"handmatige bank / loop"| SS1
    LINK1 ==>|"houdt Autoloop op de beat"| SS1
  end

  subgraph with["Met Lumi – plan vóór uitvoering"]
    direction LR
    NEXT2["Track geladen op vrij deck"]
    PLAN["Lumi bouwt Next Lighting Plan<br/>theme + cue per phrase-instance"]
    REVIEW["DJ ziet plan op iPhone<br/>accepteert, tunet of lockt"]
    READY["Gevalideerd READY plan"]
    LEADER2["Deck wordt lighting leader"]
    EXEC["Lumi ApplyScene<br/>op iedere phrasegrens"]
    SS2["SoundSwitch<br/>voert bank / Autoloop uit"]
    LINK2["Ableton Link<br/>BPM + beat + bar"]
    LIGHT["Muzikaal passende<br/>DMX-lichtshow"]

    NEXT2 --> PLAN
    PLAN --> REVIEW
    REVIEW --> READY
    READY --> LEADER2
    LEADER2 --> EXEC
    EXEC -->|"WAT verandert op de phrasegrens"| SS2
    LINK2 ==>|"WANNEER beats en bars lopen"| SS2
    SS2 --> LIGHT
  end

  SS1 -.->|"Lumi voegt de ontbrekende planlaag toe"| NEXT2
```

Ableton Link lost de continue synchronisatie op, maar kent de trackstructuur en
creatieve intentie niet. SoundSwitch bezit de lichtcontent, maar plant niet
vooruit op de geladen Rekordbox-track. Lumi verbindt precies die twee domeinen:
het plant **wat** straks moet spelen en laat SoundSwitch via zijn bestaande Link-
sync bepalen **hoe** dat exact op de beat blijft lopen.

## Technische bronbasis

- [Beat Link Trigger – Working with Ableton Link](https://blt-guide.deepsymmetry.org/beat-link-trigger/8.0.0/Link.html)
- [Beat Link Trigger – Triggers en playerstatus](https://blt-guide.deepsymmetry.org/beat-link-trigger/8.0.0/Triggers.html)
- [SoundSwitch – Ableton Link](https://support.soundswitch.com/en/support/solutions/articles/69000847102-soundswitch-utilizing-ableton-link-with-soundswitch)
- [SoundSwitch – Autoloops](https://support.soundswitch.com/en/support/solutions/articles/69000847100-soundswitch-autoloops-explained)
