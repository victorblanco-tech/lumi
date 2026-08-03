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
    public let content: LiveWorkspaceContent?
    public let diagnostic: String?
    public let planInteraction: PlanInteractionPresentation

    public init(
        condition: LiveWorkspaceCondition,
        engine: ProviderPresentation,
        runtime: ProviderPresentation,
        source: ProviderPresentation,
        content: LiveWorkspaceContent?,
        diagnostic: String? = nil,
        planInteraction: PlanInteractionPresentation = .idle
    ) {
        self.condition = condition
        self.engine = engine
        self.runtime = runtime
        self.source = source
        self.content = content
        self.diagnostic = diagnostic
        self.planInteraction = planInteraction
    }
}

public struct LiveWorkspaceContent: Equatable, Sendable {
    public let liveDeck: DeckSnapshot
    public let nextDeck: DeckSnapshot
    public let plan: PlanSnapshot?
    public let sourceName: String
    public let stateRevision: UInt64
    public let planningOptions: PlanningOptionsSnapshot

    public init(
        liveDeck: DeckSnapshot,
        nextDeck: DeckSnapshot,
        plan: PlanSnapshot?,
        sourceName: String,
        stateRevision: UInt64,
        planningOptions: PlanningOptionsSnapshot
    ) {
        self.liveDeck = liveDeck
        self.nextDeck = nextDeck
        self.plan = plan
        self.sourceName = sourceName
        self.stateRevision = stateRevision
        self.planningOptions = planningOptions
    }
}

public enum PlanInteractionPresentation: Equatable, Sendable {
    case idle
    case submitting
    case succeeded(String)
    case rejected(String)
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
        planInteraction: PlanInteractionPresentation = .idle
    ) -> LiveWorkspaceState {
        state(
            from: snapshot,
            forceCondition: nil,
            diagnostic: nil,
            planInteraction: planInteraction
        )
    }

    public static func stale(_ snapshot: EngineSnapshot) -> LiveWorkspaceState {
        state(
            from: snapshot,
            forceCondition: .stale,
            diagnostic: "Showing the last complete snapshot while Lumi reconnects.",
            planInteraction: .idle
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
        planInteraction: PlanInteractionPresentation
    ) -> LiveWorkspaceState {
        let content = content(from: snapshot)
        let derivedCondition: LiveWorkspaceCondition
        if snapshot.runtime.health != "ready" || snapshot.deckSource.status != "ready" {
            derivedCondition = .degraded
        } else if content?.plan?.status == "fallback" {
            derivedCondition = .fallback
        } else if content?.plan == nil {
            derivedCondition = .degraded
        } else {
            derivedCondition = .ready
        }
        let condition = forceCondition ?? derivedCondition
        let healthyProviderCondition: ProviderCondition = condition == .stale ? .stale : .ready

        let runtime = snapshot.runtime
        return LiveWorkspaceState(
            condition: condition,
            engine: ProviderPresentation(
                detail: "\(snapshot.endpoint) · engine \(snapshot.engineVersion) · protocol v\(snapshot.protocolVersion) · snapshot #\(snapshot.snapshotSequence)",
                condition: healthyProviderCondition
            ),
            runtime: ProviderPresentation(
                detail: "Processed events: \(runtime.processedEvents) · queue \(runtime.queueDepth)/\(runtime.queueCapacity) · revision #\(snapshot.stateRevision) · last: \(runtime.lastDecision)",
                condition: runtime.health == "ready" ? healthyProviderCondition : .degraded
            ),
            source: ProviderPresentation(
                detail: snapshot.deckSource.providerKind.capitalized,
                condition: snapshot.deckSource.status == "ready" ? healthyProviderCondition : .degraded
            ),
            content: content,
            diagnostic: diagnostic ?? defaultDiagnostic(for: derivedCondition),
            planInteraction: planInteraction
        )
    }

    private static func content(from snapshot: EngineSnapshot) -> LiveWorkspaceContent? {
        guard let liveDeck = snapshot.decks.first(where: { $0.deckID == snapshot.leaderDeckID }),
              let nextDeck = snapshot.decks.first(where: { $0.deckID != snapshot.leaderDeckID }) else {
            return nil
        }
        return LiveWorkspaceContent(
            liveDeck: liveDeck,
            nextDeck: nextDeck,
            plan: snapshot.nextPlan,
            sourceName: snapshot.deckSource.providerKind.capitalized,
            stateRevision: snapshot.stateRevision,
            planningOptions: snapshot.planningOptions
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
