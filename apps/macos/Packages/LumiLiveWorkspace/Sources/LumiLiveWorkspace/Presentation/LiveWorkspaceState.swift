import Foundation

public enum LiveWorkspaceCondition: String, Equatable, Sendable {
    case empty
    case loading
    case ready
    case fallback
    case stale
    case degraded
    case disconnected
    case error
}

public enum ProviderCondition: String, Equatable, Sendable {
    case empty
    case loading
    case ready
    case stale
    case degraded
    case error
}

public struct ProviderPresentation: Equatable, Sendable {
    public let detail: String
    public let condition: ProviderCondition

    public init(detail: String, condition: ProviderCondition) {
        self.detail = detail
        self.condition = condition
    }
}

public struct LiveWorkspaceState: Equatable, Sendable {
    public let condition: LiveWorkspaceCondition
    public let engine: ProviderPresentation
    public let runtime: ProviderPresentation
    public let source: ProviderPresentation
    public let planner: ProviderPresentation
    public let output: ProviderPresentation
    public let lightingMidi: ProviderPresentation
    public let playbackClock: ProviderPresentation
    public let content: LiveWorkspaceContent?
    public let diagnostic: String?
    public let planInteraction: PlanInteractionPresentation
    public let sessionInteraction: SessionInteractionPresentation

    public init(
        condition: LiveWorkspaceCondition,
        engine: ProviderPresentation,
        runtime: ProviderPresentation,
        source: ProviderPresentation,
        planner: ProviderPresentation,
        output: ProviderPresentation,
        lightingMidi: ProviderPresentation = .init(detail: "Status unavailable", condition: .empty),
        playbackClock: ProviderPresentation = .init(detail: "Status unavailable", condition: .empty),
        content: LiveWorkspaceContent?,
        diagnostic: String? = nil,
        planInteraction: PlanInteractionPresentation = .idle,
        sessionInteraction: SessionInteractionPresentation = .idle
    ) {
        self.condition = condition
        self.engine = engine
        self.runtime = runtime
        self.source = source
        self.planner = planner
        self.output = output
        self.lightingMidi = lightingMidi
        self.playbackClock = playbackClock
        self.content = content
        self.diagnostic = diagnostic
        self.planInteraction = planInteraction
        self.sessionInteraction = sessionInteraction
    }
}

public struct LiveWorkspaceContent: Equatable, Sendable {
    public let liveDeck: DeckSnapshot?
    public let nextDeck: DeckSnapshot?
    public let decks: [DeckSnapshot]
    public let leaderDeckID: UInt64?
    public let livePlan: PlanSnapshot?
    public let plan: PlanSnapshot?
    public let sourceName: String
    public let sourceMode: String
    public let stateRevision: UInt64
    public let planningOptions: PlanningOptionsSnapshot
    public let operationState: String
    public let lightingTimingOffsetMillis: Int
    public let pendingLightingTimingOffsetMillis: Int?
    public let simulation: SimulationSnapshot?
    public let timeline: [TimelineEntrySnapshot]

    public init(
        liveDeck: DeckSnapshot?,
        nextDeck: DeckSnapshot?,
        decks: [DeckSnapshot],
        leaderDeckID: UInt64?,
        livePlan: PlanSnapshot? = nil,
        plan: PlanSnapshot?,
        sourceName: String,
        sourceMode: String = "localPlayback",
        stateRevision: UInt64,
        planningOptions: PlanningOptionsSnapshot,
        operationState: String,
        lightingTimingOffsetMillis: Int = 0,
        pendingLightingTimingOffsetMillis: Int? = nil,
        simulation: SimulationSnapshot? = nil,
        timeline: [TimelineEntrySnapshot]
    ) {
        self.liveDeck = liveDeck
        self.nextDeck = nextDeck
        self.decks = decks
        self.leaderDeckID = leaderDeckID
        self.livePlan = livePlan
        self.plan = plan
        self.sourceName = sourceName
        self.sourceMode = sourceMode
        self.stateRevision = stateRevision
        self.planningOptions = planningOptions
        self.operationState = operationState
        self.lightingTimingOffsetMillis = lightingTimingOffsetMillis
        self.pendingLightingTimingOffsetMillis = pendingLightingTimingOffsetMillis
        self.simulation = simulation
        self.timeline = timeline
    }
}

public enum PlanInteractionPresentation: Equatable, Sendable {
    case idle
    case submitting
    case succeeded(String)
    case rejected(String)
}

public enum SessionInteractionPresentation: Equatable, Sendable {
    case idle
    case submitting
    case succeeded(String)
    case rejected(String)
}

public enum SessionCommandRequest: Equatable, Sendable {
    case setOperationState(String, expectedStateRevision: UInt64)
    case setLocalPlaybackLeader(UInt64, expectedStateRevision: UInt64)
    case selectDeckSourceMode(String, expectedStateRevision: UInt64)
}

public enum LocalPlaybackRequest: Equatable, Sendable {
    case togglePlayback(deckID: UInt64)
    case stop(deckID: UInt64)
    case seek(deckID: UInt64, progress: Double)
}

public struct PlanMutationContext: Equatable, Sendable {
    public let planID: String
    public let trackLoadID: UInt64
    public let expectedPlanRevision: UInt64

    public init(planID: String, trackLoadID: UInt64, expectedPlanRevision: UInt64) {
        self.planID = planID
        self.trackLoadID = trackLoadID
        self.expectedPlanRevision = expectedPlanRevision
    }
}

public enum PlanMutationRequest: Equatable, Sendable {
    case selectTheme(context: PlanMutationContext, themeID: UInt64)
    case selectThemeFromPhrase(
        context: PlanMutationContext,
        phraseIndex: UInt64,
        themeID: UInt64
    )
    case selectScene(
        context: PlanMutationContext,
        phraseIndex: UInt64,
        sceneID: UInt64
    )
    case setCueLock(
        context: PlanMutationContext,
        phraseIndex: UInt64,
        locked: Bool
    )
    case regeneratePlan(context: PlanMutationContext)

    public var context: PlanMutationContext {
        switch self {
        case let .selectTheme(context, _),
             let .selectThemeFromPhrase(context, _, _),
             let .selectScene(context, _, _),
             let .setCueLock(context, _, _),
             let .regeneratePlan(context):
            context
        }
    }
}

public enum LiveWorkspacePresenter {
    public static func stopped() -> LiveWorkspaceState {
        waiting(
            condition: .empty,
            engineDetail: "Local engine stopped",
            providerCondition: .empty
        )
    }

    public static func starting() -> LiveWorkspaceState {
        waiting(
            condition: .loading,
            engineDetail: "Starting local engine…",
            providerCondition: .loading
        )
    }

    public static func connecting(to endpoint: String) -> LiveWorkspaceState {
        waiting(
            condition: .loading,
            engineDetail: "Connecting to \(endpoint)…",
            providerCondition: .loading
        )
    }

    public static func ready(
        _ snapshot: EngineSnapshot,
        planInteraction: PlanInteractionPresentation = .idle,
        sessionInteraction: SessionInteractionPresentation = .idle
    ) -> LiveWorkspaceState {
        state(
            from: snapshot,
            forceCondition: nil,
            diagnostic: nil,
            planInteraction: planInteraction,
            sessionInteraction: sessionInteraction
        )
    }

    public static func stale(_ snapshot: EngineSnapshot) -> LiveWorkspaceState {
        state(
            from: snapshot,
            forceCondition: .stale,
            diagnostic: "Showing the last complete snapshot while Lumi reconnects.",
            planInteraction: .idle,
            sessionInteraction: .idle
        )
    }

    public static func disconnected() -> LiveWorkspaceState {
        waiting(
            condition: .disconnected,
            engineDetail: "Local engine disconnected",
            providerCondition: .error,
            diagnostic: "Live data is unavailable. Retry the local engine connection."
        )
    }

    public static func failed(_ message: String) -> LiveWorkspaceState {
        waiting(
            condition: .error,
            engineDetail: message,
            providerCondition: .error,
            diagnostic: "The local engine could not start."
        )
    }

    private static func state(
        from snapshot: EngineSnapshot,
        forceCondition: LiveWorkspaceCondition?,
        diagnostic: String?,
        planInteraction: PlanInteractionPresentation,
        sessionInteraction: SessionInteractionPresentation
    ) -> LiveWorkspaceState {
        let content = content(from: snapshot)
        let derivedCondition: LiveWorkspaceCondition
        if snapshot.runtime.health != "ready"
            || snapshot.deckSource.status != "ready"
            || snapshot.midiIntegration?.state != "ready"
            || !["ready", "running"].contains(snapshot.midiClockIntegration?.state ?? "stopped") {
            derivedCondition = .degraded
        } else if snapshot.decks.isEmpty {
            derivedCondition = .empty
        } else if snapshot.decks.contains(where: { $0.planEligibility == .autoHeld })
            || [snapshot.livePlan, snapshot.nextPlan]
                .compactMap({ $0 })
                .contains(where: { $0.status == "fallback" }) {
            derivedCondition = .fallback
        } else {
            derivedCondition = .ready
        }
        let condition = forceCondition ?? derivedCondition
        let healthyProviderCondition: ProviderCondition = condition == .stale ? .stale : .ready

        let runtime = snapshot.runtime
        return LiveWorkspaceState(
            condition: condition,
            engine: ProviderPresentation(
                detail: "\(snapshot.endpoint) · engine \(snapshot.engineVersion) · protocol v\(snapshot.protocolVersion)",
                condition: healthyProviderCondition
            ),
            runtime: ProviderPresentation(
                detail: "\(runtime.model) · revision #\(snapshot.stateRevision)",
                condition: runtime.health == "ready" ? healthyProviderCondition : .degraded
            ),
            source: ProviderPresentation(
                detail: deckSourceDetail(snapshot),
                condition: snapshot.deckSource.status == "ready" ? healthyProviderCondition : .degraded
            ),
            planner: ProviderPresentation(
                detail: plannerDetail(snapshot),
                condition: plannerCondition(snapshot, healthy: healthyProviderCondition)
            ),
            output: ProviderPresentation(
                detail: "\(snapshot.outputProvider.providerKind) · \(snapshot.outputProvider.recordCount) records",
                condition: snapshot.outputProvider.status == "ready" ? healthyProviderCondition : .degraded
            ),
            lightingMidi: ProviderPresentation(
                detail: lightingMidiDetail(snapshot),
                condition: snapshot.midiIntegration?.state == "ready"
                    ? healthyProviderCondition : .degraded
            ),
            playbackClock: ProviderPresentation(
                detail: playbackClockDetail(snapshot),
                condition: ["ready", "running"].contains(
                    snapshot.midiClockIntegration?.state ?? "stopped"
                ) ? healthyProviderCondition : .degraded
            ),
            content: content,
            diagnostic: diagnostic ?? defaultDiagnostic(for: derivedCondition),
            planInteraction: planInteraction,
            sessionInteraction: sessionInteraction
        )
    }

    private static func lightingMidiDetail(_ snapshot: EngineSnapshot) -> String {
        guard let midi = snapshot.midiIntegration else { return "Status unavailable" }
        if let error = midi.lastError { return "\(midi.sourceName) · \(error)" }
        let bank = midi.activeBank.map { " · bank \($0) active" } ?? ""
        let offset = String(format: "%+d ms", midi.timingOffsetMillis)
        let pending = midi.pendingTimingOffsetMillis.map {
            " · \(String(format: "%+d ms", $0)) pending for next phrase"
        } ?? ""
        return "\(midi.sourceName) · auto-publish \(midi.autoPublishEnabled ? "on" : "off") · \(midi.sentPulseCount) pulses\(bank) · timing \(offset) applied\(pending) · bank pre-roll \(midi.bankPreRollMillis) ms"
    }

    private static func playbackClockDetail(_ snapshot: EngineSnapshot) -> String {
        guard let clock = snapshot.midiClockIntegration else { return "Status unavailable" }
        if let error = clock.lastError { return "\(clock.sourceName) · \(error)" }
        let bpm = clock.bpmMilli.map { String(format: " · %.3f BPM", Double($0) / 1_000) } ?? ""
        return "\(clock.sourceName) · \(clock.state)\(bpm) · \(clock.sentTickCount) ticks"
    }

    private static func deckSourceDetail(_ snapshot: EngineSnapshot) -> String {
        guard snapshot.deckSource.mode == "connectedDecks",
              let input = snapshot.deckInputIntegration else {
            return snapshot.deckSource.displayName
        }
        let isDirectProLink = input.protocolName == "lumi-prolink-bridge"
        let endpoint = if isDirectProLink {
            "Direct Pro DJ Link"
        } else {
            input.destinationName ?? "MIDI input unavailable"
        }
        let messageKind = isDirectProLink ? "bridge events" : "MIDI messages"
        let lastDeck = input.lastDeckID.map { " · last deck \($0)" } ?? ""
        return "\(snapshot.deckSource.displayName) · \(endpoint) · \(input.receivedMessageCount) \(messageKind) · \(input.committedFrameCount) frames\(lastDeck)"
    }

    private static func plannerDetail(_ snapshot: EngineSnapshot) -> String {
        let plans = [snapshot.livePlan, snapshot.nextPlan].compactMap { $0 }
        if snapshot.decks.isEmpty { return "Waiting for a loaded track" }
        if plans.isEmpty { return "Automatic planning held" }
        return "\(plans.count) deck plan\(plans.count == 1 ? "" : "s") ready"
    }

    private static func plannerCondition(
        _ snapshot: EngineSnapshot,
        healthy: ProviderCondition
    ) -> ProviderCondition {
        if snapshot.decks.isEmpty { return .empty }
        let plannedDeckIDs = Set([snapshot.livePlan, snapshot.nextPlan].compactMap { $0?.deckID })
        return snapshot.decks.allSatisfy { deck in
            deck.planEligibility != .autoHeld && plannedDeckIDs.contains(deck.deckID)
        } ? healthy : .degraded
    }

    private static func content(from snapshot: EngineSnapshot) -> LiveWorkspaceContent? {
        let liveDeck = snapshot.leaderDeckID.flatMap { leaderDeckID in
            snapshot.decks.first(where: { $0.deckID == leaderDeckID })
        }
        let nextDeck = snapshot.decks.first(where: { $0.deckID != snapshot.leaderDeckID })
        return LiveWorkspaceContent(
            liveDeck: liveDeck,
            nextDeck: nextDeck,
            decks: snapshot.decks.sorted { $0.deckID < $1.deckID },
            leaderDeckID: snapshot.leaderDeckID,
            livePlan: snapshot.livePlan,
            plan: snapshot.nextPlan,
            sourceName: snapshot.deckSource.displayName,
            sourceMode: snapshot.deckSource.mode,
            stateRevision: snapshot.stateRevision,
            planningOptions: snapshot.planningOptions,
            operationState: snapshot.operationState,
            lightingTimingOffsetMillis: snapshot.midiIntegration?.timingOffsetMillis ?? 0,
            pendingLightingTimingOffsetMillis: snapshot.midiIntegration?.pendingTimingOffsetMillis,
            simulation: snapshot.simulation,
            timeline: snapshot.timeline
        )
    }

    private static func waiting(
        condition: LiveWorkspaceCondition,
        engineDetail: String,
        providerCondition: ProviderCondition,
        diagnostic: String? = nil
    ) -> LiveWorkspaceState {
        LiveWorkspaceState(
            condition: condition,
            engine: ProviderPresentation(
                detail: engineDetail,
                condition: providerCondition
            ),
            runtime: ProviderPresentation(
                detail: "Waiting for the authoritative engine snapshot",
                condition: providerCondition == .error ? .empty : providerCondition
            ),
            source: ProviderPresentation(
                detail: "Waiting for a deck source",
                condition: providerCondition == .error ? .empty : providerCondition
            ),
            planner: ProviderPresentation(
                detail: "Waiting for the planner",
                condition: providerCondition == .error ? .empty : providerCondition
            ),
            output: ProviderPresentation(
                detail: "Waiting for an output provider",
                condition: providerCondition == .error ? .empty : providerCondition
            ),
            lightingMidi: ProviderPresentation(
                detail: "Waiting for the lighting MIDI source",
                condition: providerCondition == .error ? .empty : providerCondition
            ),
            playbackClock: ProviderPresentation(
                detail: "Waiting for the playback clock",
                condition: providerCondition == .error ? .empty : providerCondition
            ),
            content: nil,
            diagnostic: diagnostic
        )
    }

    private static func defaultDiagnostic(
        for condition: LiveWorkspaceCondition
    ) -> String? {
        switch condition {
        case .fallback:
            "Phrase analysis is incomplete. Lumi prepared a safe hold plan."
        case .degraded:
            "Live data is available, but one or more providers need attention."
        default:
            nil
        }
    }
}
