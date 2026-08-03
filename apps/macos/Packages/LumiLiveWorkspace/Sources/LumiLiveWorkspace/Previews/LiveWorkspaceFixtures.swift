import Foundation

public enum LiveWorkspaceFixtures {
    public static let readySnapshot = EngineSnapshot(
        endpoint: "127.0.0.1:52841",
        engineVersion: "0.1.0-dev",
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
        deckSource: DeckSourceSnapshot(providerKind: "simulator", status: "ready"),
        simulation: SimulationSnapshot(speed: 64, paused: false),
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
                pitchClass: "a",
                keyMode: "minor",
                beat: 24,
                phraseIndex: 0
            ),
            DeckSnapshot(
                deckID: 2,
                trackLoadID: 2_001,
                title: "Neon Horizon",
                artist: "Lumi Lab",
                bpmMilli: 128_000,
                pitchClass: "c",
                keyMode: "major",
                beat: 0,
                phraseIndex: nil
            )
        ],
        nextPlan: PlanSnapshot(
            planID: "14113485664261432828",
            deckID: 2,
            trackLoadID: 2_001,
            trackDurationBeats: 128,
            revision: 1,
            configurationRevision: 1,
            status: "ready",
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
                result: "simulated",
                reason: "outputEffectRecorded"
            )
        ]
    )

    public static let ready = LiveWorkspacePresenter.ready(readySnapshot)
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
    public static let revisionConflict = LiveWorkspacePresenter.ready(
        readySnapshot,
        planInteraction: .rejected(
            "Plan changed elsewhere. Lumi refreshed the latest revision."
        )
    )

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
                    themeName: "Electric Bloom",
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
            simulation: readySnapshot.simulation,
            outputProvider: readySnapshot.outputProvider,
            leaderDeckID: readySnapshot.leaderDeckID,
            decks: readySnapshot.decks,
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
            simulation: readySnapshot.simulation,
            outputProvider: readySnapshot.outputProvider,
            leaderDeckID: readySnapshot.leaderDeckID,
            decks: readySnapshot.decks,
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
                status: status
            ),
            simulation: snapshot.simulation,
            outputProvider: snapshot.outputProvider,
            leaderDeckID: snapshot.leaderDeckID,
            decks: snapshot.decks,
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
        _ slot: UInt64
    ) -> PlanCueSnapshot {
        PlanCueSnapshot(
            phraseIndex: index,
            startBeat: start,
            endBeat: end,
            origin: "automatic",
            locked: false,
            reason: .phraseCategoryMatched(phraseKind: phrase, category: category),
            action: .applyLook(
                themeID: 2,
                themeName: "Electric Bloom",
                sceneID: sceneID,
                sceneName: scene,
                category: category,
                loopBank: bank,
                loopSlot: slot
            )
        )
    }

    private static let planningOptions = PlanningOptionsSnapshot(
        themes: [
            ThemeOptionSnapshot(id: 1, name: "Midnight Drive"),
            ThemeOptionSnapshot(id: 2, name: "Electric Bloom")
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
