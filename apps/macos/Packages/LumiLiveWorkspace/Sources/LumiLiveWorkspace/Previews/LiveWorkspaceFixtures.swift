import Foundation

public enum LiveWorkspaceFixtures {
    public static let readySnapshot = EngineSnapshot(
        endpoint: "127.0.0.1:52841",
        engineVersion: "0.5.0-dev-8",
        protocolVersion: 1,
        snapshotSequence: 42,
        stateRevision: 8,
        operationState: "armed",
        runtime: RuntimeSnapshot(
            model: "singleWriterReducer",
            health: "ready",
            queueCapacity: 256,
            queueDepth: 0,
            processedEvents: 8,
            lastDecision: "phraseChanged"
        ),
        deckSource: DeckSourceSnapshot(
            providerKind: "localPlayback",
            mode: "localPlayback",
            displayName: "Local Playback",
            status: "ready"
        ),
        midiIntegration: MidiOutputIntegrationSnapshot(
            state: "ready",
            sourceName: "Lumi Virtual MIDI",
            protocolName: "MIDI 1.0 UMP",
            sentPulseCount: 4,
            lastEvent: "Triggered Bank 1 → AutoLoop 1",
            lastError: nil,
            activeBank: 1,
            autoPublishEnabled: true,
            timingOffsetMillis: 0
        ),
        midiClockIntegration: MidiClockIntegrationSnapshot(
            state: "running",
            sourceName: "Lumi Clock",
            protocolName: "MIDI Clock · 24 PPQN",
            bpmMilli: 124_000,
            sentTickCount: 96,
            lastEvent: "Clock running at 124.000 BPM",
            lastError: nil
        ),
        abletonLinkIntegration: AbletonLinkIntegrationSnapshot(
            enabled: true,
            state: "running",
            provider: "Carabiner",
            helperVersion: "1.2.0",
            peers: 1,
            source: "localPlayback",
            deckNumber: 1,
            bpmMilli: 124_000,
            beatWithinBar: 1,
            playing: true,
            generation: 1,
            lastBeatAgeMillis: 4,
            phaseErrorMicros: 120,
            lastReanchor: "started",
            lastEvent: "Ableton Link synchronized to Local Playback",
            lastError: nil
        ),
        outputProvider: OutputProviderSnapshot(
            providerKind: "dryRun",
            status: "ready",
            recordCount: 4
        ),
        leaderDeckID: 1,
        decks: [
            DeckSnapshot(
                deckID: 1,
                trackLoadID: 1_001,
                title: "Aurora Signal",
                artist: "Lumi Lab",
                bpmMilli: 124_000,
                colorRGB: 12_273_790,
                pitchClass: "a",
                keyMode: "minor",
                beat: 24,
                playing: true,
                phraseIndex: 0,
                durationBeats: 128,
                phrases: deckPhrases(second: "verse"),
                waveformPreview: waveform(seed: 101),
                hotCues: hotCues(),
                planEligibility: .readyExact,
                localPlayback: LocalPlaybackTrackSnapshot(
                    audioURI: "lumi-demo://aurora-signal",
                    durationMillis: 61_935
                )
            ),
            DeckSnapshot(
                deckID: 2,
                trackLoadID: 2_001,
                title: "Neon Horizon",
                artist: "Lumi Lab",
                bpmMilli: 128_000,
                colorRGB: 4_747_469,
                pitchClass: "c",
                keyMode: "major",
                beat: 0,
                phraseIndex: nil,
                durationBeats: 128,
                phrases: deckPhrases(second: "breakdown"),
                waveformPreview: waveform(seed: 202),
                hotCues: hotCues(),
                planEligibility: .readyExact,
                localPlayback: LocalPlaybackTrackSnapshot(
                    audioURI: "lumi-demo://neon-horizon",
                    durationMillis: 60_000
                )
            )
        ],
        livePlan: PlanSnapshot(
            planID: "8411348566426143282",
            deckID: 1,
            trackLoadID: 1_001,
            trackDurationBeats: 128,
            revision: 1,
            configurationRevision: 1,
            status: "ready",
            themeDecision: ThemeDecisionSnapshot(
                themeID: 3,
                themeName: "Solar Flare",
                reason: "colorPrefer",
                matchedColorRGB: 12_273_790
            ),
            cues: [
                cue(0, 0, 32, "intro", "ambient", 1, "Soft Motion", 1, 1, 3, "Solar Flare"),
                cue(1, 32, 64, "verse", "groove", 3, "Neon Motion", 2, 1, 3, "Solar Flare"),
                cue(2, 64, 96, "build", "build", 6, "Velocity Build", 3, 2, 3, "Solar Flare"),
                cue(3, 96, 128, "drop", "impact", 7, "Full Energy", 4, 1, 3, "Solar Flare")
            ]
        ),
        nextPlan: PlanSnapshot(
            planID: "14113485664261432828",
            deckID: 2,
            trackLoadID: 2_001,
            trackDurationBeats: 128,
            revision: 1,
            configurationRevision: 1,
            status: "ready",
            themeDecision: ThemeDecisionSnapshot(
                themeID: 2,
                themeName: "Deep Ocean",
                reason: "colorPrefer",
                matchedColorRGB: 4_747_469
            ),
            cues: [
                cue(0, 0, 32, "intro", "ambient", 1, "Soft Motion", 1, 1),
                cue(1, 32, 64, "breakdown", "break", 10, "Slow Wave", 5, 2),
                cue(2, 64, 96, "build", "build", 6, "Velocity Build", 3, 2),
                cue(3, 96, 128, "drop", "impact", 7, "Full Energy", 4, 1)
            ]
        ),
        planningOptions: planningOptions,
        timeline: [
            TimelineEntrySnapshot(
                sequence: 20,
                occurredAt: 1_000,
                source: "deckSource",
                type: "phraseChanged",
                result: "scheduled",
                reason: "phraseExecutionScheduled"
            ),
            TimelineEntrySnapshot(
                sequence: 21,
                occurredAt: 1_000,
                source: "output",
                type: "outputEffectRecorded",
                result: "recorded",
                reason: "outputEffectRecorded"
            )
        ]
    )

    public static let ready = LiveWorkspacePresenter.ready(readySnapshot)
    public static let live = LiveWorkspacePresenter.ready(
        replacingOperationState(in: readySnapshot, with: "live")
    )
    public static let libraryBacked = LiveWorkspacePresenter.ready(libraryBackedSnapshot())
    public static let loading = LiveWorkspacePresenter.starting()
    public static let stale = LiveWorkspacePresenter.stale(readySnapshot)
    public static let disconnected = LiveWorkspacePresenter.disconnected()
    public static let degraded = LiveWorkspacePresenter.ready(
        replacingSource(in: readySnapshot, status: "reconnecting")
    )
    public static let fallback = LiveWorkspacePresenter.ready(fallbackSnapshot())
    public static let edited = LiveWorkspacePresenter.ready(
        editedSnapshot(),
        planInteraction: .succeeded("Plan revision 3 saved.")
    )
    public static let editedPaused = LiveWorkspacePresenter.ready(
        replacingOperationState(in: editedSnapshot(), with: "paused"),
        planInteraction: .succeeded("Plan revision 3 saved.")
    )
    public static let revisionConflict = LiveWorkspacePresenter.ready(
        readySnapshot,
        planInteraction: .rejected(
            "Plan changed elsewhere. Lumi refreshed the latest revision."
        )
    )
    public static let revisionConflictOff = LiveWorkspacePresenter.ready(
        replacingOperationState(in: readySnapshot, with: "off"),
        planInteraction: .rejected(
            "Plan changed elsewhere. Lumi refreshed the latest revision."
        )
    )

    public static func libraryBackedSnapshot() -> EngineSnapshot {
        let nextDeck = DeckSnapshot(
            deckID: 2,
            trackLoadID: 2_002,
            title: "Horizon Lines",
            artist: "Lumi Demo Library",
            bpmMilli: 128_000,
            colorRGB: 4_747_469,
            pitchClass: "c",
            keyMode: "major",
            beat: 0,
            phraseIndex: nil,
            durationBeats: 128,
            phrases: deckPhrases(second: "breakdown"),
            waveformPreview: waveform(seed: 303),
            planEligibility: .readyExact,
            localPlayback: LocalPlaybackTrackSnapshot(
                audioURI: "lumi-demo://horizon-lines",
                durationMillis: 60_000
            )
        )
        let roles = ["Intro / Outro", "Breakdown 1", "Buildup 1", "Drop"]
        let roleIDs = ["intro-outro", "breakdown-1", "buildup-1", "drop"]
        let cues = readySnapshot.nextPlan?.cues.enumerated().map { index, cue in
            PlanCueSnapshot(
                phraseIndex: cue.phraseIndex,
                startBeat: cue.startBeat,
                endBeat: cue.endBeat,
                origin: cue.origin,
                locked: cue.locked,
                reason: cue.reason,
                action: cue.action,
                libraryResolution: PlanCueLibraryResolutionSnapshot(
                    roleID: roleIDs[index],
                    roleName: roles[index],
                    strategy: index == 1 ? "fixedVariant" : "auto",
                    variantID: index == 1 ? "variant-2" : "variant-1",
                    catalogRevision: 1,
                    resolutionReason: index == 1 ? "exactVariant" : "automatic",
                    entryID: "theme-2--\(roleIDs[index])--variant-\(index == 1 ? 2 : 1)",
                    entryName: "Deep Ocean · \(roles[index]) · Variant \(index == 1 ? 2 : 1)"
                )
            )
        } ?? []
        let plan = PlanSnapshot(
            planID: "16571449367899180180",
            deckID: 2,
            trackLoadID: 2_002,
            trackDurationBeats: 128,
            revision: 1,
            configurationRevision: 1,
            status: "ready",
            themeDecision: readySnapshot.nextPlan?.themeDecision,
            libraryTrack: PlanLibraryTrackSnapshot(
                providerKind: "demo",
                sourceID: "lumi-demo-library",
                sourceName: "Lumi Demo Library",
                sourceTrackID: "horizon-lines",
                analysisRevision: "horizon-lines-v1",
                timelineRevision: 2
            ),
            cues: cues
        )
        return EngineSnapshot(
            endpoint: readySnapshot.endpoint,
            engineVersion: readySnapshot.engineVersion,
            protocolVersion: readySnapshot.protocolVersion,
            snapshotSequence: 45,
            stateRevision: 10,
            operationState: readySnapshot.operationState,
            runtime: readySnapshot.runtime,
            deckSource: readySnapshot.deckSource,
            midiIntegration: readySnapshot.midiIntegration,
            midiClockIntegration: readySnapshot.midiClockIntegration,
            abletonLinkIntegration: readySnapshot.abletonLinkIntegration,
            simulation: readySnapshot.simulation,
            outputProvider: readySnapshot.outputProvider,
            leaderDeckID: readySnapshot.leaderDeckID,
            decks: [readySnapshot.decks[0], nextDeck],
            livePlan: readySnapshot.livePlan,
            nextPlan: plan,
            planningOptions: readySnapshot.planningOptions,
            timeline: readySnapshot.timeline
        )
    }

    private static func editedSnapshot() -> EngineSnapshot {
        let editedCues: [PlanCueSnapshot] = readySnapshot.nextPlan?.cues.map { cue in
            guard cue.phraseIndex == 1 else { return cue }
            return PlanCueSnapshot(
                phraseIndex: cue.phraseIndex,
                startBeat: cue.startBeat,
                endBeat: cue.endBeat,
                origin: "user",
                locked: true,
                reason: cue.reason,
                action: .applyLook(
                    themeID: 2,
                    themeName: "Deep Ocean",
                    sceneID: 9,
                    sceneName: "Deep Space",
                    category: "break",
                    loopBank: 5,
                    loopSlot: 1
                )
            )
        } ?? []
        let plan = PlanSnapshot(
            planID: "14113485664261432828",
            deckID: 2,
            trackLoadID: 2_001,
            trackDurationBeats: 128,
            revision: 3,
            configurationRevision: 1,
            status: "ready",
            themeDecision: ThemeDecisionSnapshot(
                themeID: 2,
                themeName: "Deep Ocean",
                reason: "planInstanceUserChoice",
                matchedColorRGB: nil
            ),
            cues: editedCues
        )
        return EngineSnapshot(
            endpoint: readySnapshot.endpoint,
            engineVersion: readySnapshot.engineVersion,
            protocolVersion: readySnapshot.protocolVersion,
            snapshotSequence: 44,
            stateRevision: 10,
            operationState: readySnapshot.operationState,
            runtime: readySnapshot.runtime,
            deckSource: readySnapshot.deckSource,
            midiIntegration: readySnapshot.midiIntegration,
            midiClockIntegration: readySnapshot.midiClockIntegration,
            abletonLinkIntegration: readySnapshot.abletonLinkIntegration,
            simulation: readySnapshot.simulation,
            outputProvider: readySnapshot.outputProvider,
            leaderDeckID: readySnapshot.leaderDeckID,
            decks: readySnapshot.decks,
            livePlan: readySnapshot.livePlan,
            nextPlan: plan,
            planningOptions: readySnapshot.planningOptions,
            timeline: readySnapshot.timeline
        )
    }

    private static func fallbackSnapshot() -> EngineSnapshot {
        let plan = PlanSnapshot(
            planID: "14113485664261432828",
            deckID: 2,
            trackLoadID: 2_001,
            trackDurationBeats: 128,
            revision: 2,
            configurationRevision: 1,
            status: "fallback",
            cues: [
                PlanCueSnapshot(
                    phraseIndex: 0,
                    startBeat: 0,
                    endBeat: 128,
                    origin: "fallback",
                    locked: false,
                    reason: .missingPhraseAnalysis,
                    action: .holdCurrentLook
                )
            ]
        )
        return EngineSnapshot(
            endpoint: readySnapshot.endpoint,
            engineVersion: readySnapshot.engineVersion,
            protocolVersion: readySnapshot.protocolVersion,
            snapshotSequence: 43,
            stateRevision: 9,
            operationState: readySnapshot.operationState,
            runtime: readySnapshot.runtime,
            deckSource: readySnapshot.deckSource,
            midiIntegration: readySnapshot.midiIntegration,
            midiClockIntegration: readySnapshot.midiClockIntegration,
            abletonLinkIntegration: readySnapshot.abletonLinkIntegration,
            simulation: readySnapshot.simulation,
            outputProvider: readySnapshot.outputProvider,
            leaderDeckID: readySnapshot.leaderDeckID,
            decks: readySnapshot.decks,
            livePlan: readySnapshot.livePlan,
            nextPlan: plan,
            planningOptions: readySnapshot.planningOptions,
            timeline: readySnapshot.timeline
        )
    }

    private static func replacingSource(
        in snapshot: EngineSnapshot,
        status: String
    ) -> EngineSnapshot {
        EngineSnapshot(
            endpoint: snapshot.endpoint,
            engineVersion: snapshot.engineVersion,
            protocolVersion: snapshot.protocolVersion,
            snapshotSequence: snapshot.snapshotSequence,
            stateRevision: snapshot.stateRevision,
            operationState: snapshot.operationState,
            runtime: snapshot.runtime,
            deckSource: DeckSourceSnapshot(
                providerKind: snapshot.deckSource.providerKind,
                mode: snapshot.deckSource.mode,
                displayName: snapshot.deckSource.displayName,
                status: status
            ),
            simulation: snapshot.simulation,
            outputProvider: snapshot.outputProvider,
            leaderDeckID: snapshot.leaderDeckID,
            decks: snapshot.decks,
            livePlan: snapshot.livePlan,
            nextPlan: snapshot.nextPlan,
            planningOptions: snapshot.planningOptions,
            timeline: snapshot.timeline
        )
    }

    private static func replacingOperationState(
        in snapshot: EngineSnapshot,
        with operationState: String
    ) -> EngineSnapshot {
        EngineSnapshot(
            endpoint: snapshot.endpoint,
            engineVersion: snapshot.engineVersion,
            protocolVersion: snapshot.protocolVersion,
            snapshotSequence: snapshot.snapshotSequence,
            stateRevision: snapshot.stateRevision,
            operationState: operationState,
            runtime: snapshot.runtime,
            deckSource: snapshot.deckSource,
            midiIntegration: snapshot.midiIntegration,
            midiClockIntegration: snapshot.midiClockIntegration,
            abletonLinkIntegration: snapshot.abletonLinkIntegration,
            simulation: snapshot.simulation,
            outputProvider: snapshot.outputProvider,
            leaderDeckID: snapshot.leaderDeckID,
            decks: snapshot.decks,
            livePlan: snapshot.livePlan,
            nextPlan: snapshot.nextPlan,
            planningOptions: snapshot.planningOptions,
            timeline: snapshot.timeline
        )
    }

    private static func cue(
        _ index: UInt64,
        _ start: UInt64,
        _ end: UInt64,
        _ phrase: String,
        _ category: String,
        _ sceneID: UInt64,
        _ scene: String,
        _ bank: UInt64,
        _ slot: UInt64,
        _ themeID: UInt64 = 2,
        _ themeName: String = "Deep Ocean"
    ) -> PlanCueSnapshot {
        let role: (id: String, name: String) = switch phrase {
        case "intro": ("intro-outro", "Intro / Outro")
        case "verse": ("bridge", "Bridge")
        case "breakdown": ("breakdown-1", "Breakdown 1")
        case "build": ("buildup-1", "Buildup 1")
        case "drop": ("drop", "Drop")
        default: (phrase, phrase.capitalized)
        }
        return PlanCueSnapshot(
            phraseIndex: index,
            startBeat: start,
            endBeat: end,
            origin: "automatic",
            locked: false,
            reason: .phraseCategoryMatched(phraseKind: phrase, category: category),
            action: .applyLook(
                themeID: themeID,
                themeName: themeName,
                sceneID: sceneID,
                sceneName: scene,
                category: category,
                loopBank: bank,
                loopSlot: slot
            ),
            libraryResolution: PlanCueLibraryResolutionSnapshot(
                roleID: role.id,
                roleName: role.name,
                strategy: "auto",
                variantID: "variant-1",
                catalogRevision: 1,
                resolutionReason: "automatic",
                entryID: "theme-\(themeID)--\(role.id)--variant-1",
                entryName: scene,
                bankNumber: bank,
                autoloopNumber: slot,
                modifierChoices: index == 0 ? [
                    PlanModifierChoiceSnapshot(
                        id: "static-dark",
                        name: "Moving Heads Off",
                        kind: "atmosphere",
                        scope: "phrase",
                        midiChannel: 12,
                        midiNote: 64
                    )
                ] : []
            )
        )
    }

    private static func deckPhrases(second: String) -> [DeckPhraseSnapshot] {
        [
            DeckPhraseSnapshot(
                index: 0,
                startBeat: 0,
                endBeat: 32,
                kind: "intro",
                roleID: "intro-outro",
                roleName: "Intro / Outro"
            ),
            DeckPhraseSnapshot(
                index: 1,
                startBeat: 32,
                endBeat: 64,
                kind: second,
                roleID: second == "breakdown" ? "breakdown-1" : "bridge",
                roleName: second == "breakdown" ? "Breakdown 1" : "Bridge"
            ),
            DeckPhraseSnapshot(
                index: 2,
                startBeat: 64,
                endBeat: 96,
                kind: "build",
                roleID: "buildup-1",
                roleName: "Buildup 1"
            ),
            DeckPhraseSnapshot(
                index: 3,
                startBeat: 96,
                endBeat: 128,
                kind: "drop",
                roleID: "drop",
                roleName: "Drop"
            )
        ]
    }

    private static func waveform(seed: UInt64) -> DeckWaveformPreviewSnapshot {
        let points = (0..<192).map { index in
            let mixed = seed &* 6_364_136_223_846_793_005
                &+ UInt64(index) &* 1_442_695_040_888_963_407
            return DeckWaveformPointSnapshot(
                low: UInt8(4 + mixed % 28),
                mid: UInt8(3 + mixed.rotatedLeft(17) % 29),
                high: UInt8(2 + mixed.rotatedLeft(37) % 30)
            )
        }
        return DeckWaveformPreviewSnapshot(source: "library", style: "rgb", points: points)
    }

    private static func hotCues() -> [DeckHotCueSnapshot] {
        [
            DeckHotCueSnapshot(
                index: 1,
                timeMillis: 15_484,
                name: "",
                colorRGB: 0x30_5A_FF
            ),
            DeckHotCueSnapshot(
                index: 2,
                timeMillis: 30_968,
                name: "",
                colorRGB: 0xFF_A0_00
            ),
            DeckHotCueSnapshot(
                index: 3,
                timeMillis: 46_451,
                loopEndMillis: 54_193,
                name: "",
                colorRGB: 0xE6_28_28
            )
        ]
    }

    private static let planningOptions = PlanningOptionsSnapshot(
        themes: [
            ThemeOptionSnapshot(id: 1, name: "Electric Bloom"),
            ThemeOptionSnapshot(id: 2, name: "Deep Ocean"),
            ThemeOptionSnapshot(id: 3, name: "Solar Flare"),
            ThemeOptionSnapshot(id: 4, name: "Ultraviolet")
        ],
        scenes: [
            SceneOptionSnapshot(id: 1, name: "Soft Motion", category: "ambient", loopBank: 1, loopSlot: 1),
            SceneOptionSnapshot(id: 2, name: "Star Wash", category: "ambient", loopBank: 1, loopSlot: 2),
            SceneOptionSnapshot(id: 3, name: "Neon Motion", category: "groove", loopBank: 2, loopSlot: 1),
            SceneOptionSnapshot(id: 4, name: "Prism Sweep", category: "groove", loopBank: 2, loopSlot: 2),
            SceneOptionSnapshot(id: 5, name: "Rising Pulse", category: "build", loopBank: 3, loopSlot: 1),
            SceneOptionSnapshot(id: 6, name: "Velocity Build", category: "build", loopBank: 3, loopSlot: 2),
            SceneOptionSnapshot(id: 7, name: "Full Energy", category: "impact", loopBank: 4, loopSlot: 1),
            SceneOptionSnapshot(id: 8, name: "Color Impact", category: "impact", loopBank: 4, loopSlot: 2),
            SceneOptionSnapshot(id: 9, name: "Deep Space", category: "break", loopBank: 5, loopSlot: 1),
            SceneOptionSnapshot(id: 10, name: "Slow Wave", category: "break", loopBank: 5, loopSlot: 2)
        ]
    )
}

private extension UInt64 {
    func rotatedLeft(_ amount: UInt64) -> UInt64 {
        let shift = amount % 64
        return (self << shift) | (self >> ((64 - shift) % 64))
    }
}
