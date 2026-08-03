import Foundation

public enum LiveWorkspaceFixtures {
    public static let readySnapshot = EngineSnapshot(
        endpoint: "127.0.0.1:52841",
        engineVersion: "0.0.5-dev",
        protocolVersion: 1,
        snapshotSequence: 42,
        stateRevision: 8,
        runtime: RuntimeSnapshot(
            model: "singleWriterReducer",
            health: "ready",
            queueCapacity: 256,
            queueDepth: 0,
            processedEvents: 8,
            lastDecision: "phraseChanged"
        ),
        deckSource: DeckSourceSnapshot(providerKind: "simulator", status: "ready"),
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
            deckID: 2,
            trackLoadID: 2_001,
            trackDurationBeats: 128,
            revision: 1,
            configurationRevision: 1,
            status: "ready",
            cues: [
                cue(0, 0, 32, "intro", "ambient", "Soft Motion", 1, 1),
                cue(1, 32, 64, "breakdown", "break", "Slow Wave", 5, 2),
                cue(2, 64, 96, "build", "build", "Velocity Build", 3, 2),
                cue(3, 96, 128, "drop", "impact", "Full Energy", 4, 1)
            ]
        )
    )

    public static let ready = LiveWorkspacePresenter.ready(readySnapshot)
    public static let loading = LiveWorkspacePresenter.starting()
    public static let stale = LiveWorkspacePresenter.stale(readySnapshot)
    public static let disconnected = LiveWorkspacePresenter.disconnected()
    public static let degraded = LiveWorkspacePresenter.ready(
        replacingSource(in: readySnapshot, status: "reconnecting")
    )
    public static let fallback = LiveWorkspacePresenter.ready(fallbackSnapshot())

    private static func fallbackSnapshot() -> EngineSnapshot {
        let plan = PlanSnapshot(
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
            runtime: readySnapshot.runtime,
            deckSource: readySnapshot.deckSource,
            leaderDeckID: readySnapshot.leaderDeckID,
            decks: readySnapshot.decks,
            nextPlan: plan
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
            runtime: snapshot.runtime,
            deckSource: DeckSourceSnapshot(
                providerKind: snapshot.deckSource.providerKind,
                status: status
            ),
            leaderDeckID: snapshot.leaderDeckID,
            decks: snapshot.decks,
            nextPlan: snapshot.nextPlan
        )
    }

    private static func cue(
        _ index: UInt64,
        _ start: UInt64,
        _ end: UInt64,
        _ phrase: String,
        _ category: String,
        _ scene: String,
        _ bank: UInt64,
        _ slot: UInt64
    ) -> PlanCueSnapshot {
        PlanCueSnapshot(
            phraseIndex: index,
            startBeat: start,
            endBeat: end,
            origin: "automatic",
            reason: .phraseCategoryMatched(phraseKind: phrase, category: category),
            action: .applyLook(
                themeName: "Electric Bloom",
                sceneName: scene,
                category: category,
                loopBank: bank,
                loopSlot: slot
            )
        )
    }
}
