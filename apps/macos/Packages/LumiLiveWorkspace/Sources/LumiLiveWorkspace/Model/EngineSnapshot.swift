import Foundation

public struct EngineSnapshot: Equatable, Sendable {
    public let endpoint: String
    public let engineVersion: String
    public let protocolVersion: Int
    public let snapshotSequence: UInt64
    public let stateRevision: UInt64
    public let operationState: String
    public let runtime: RuntimeSnapshot
    public let deckSource: DeckSourceSnapshot
    public let simulation: SimulationSnapshot
    public let outputProvider: OutputProviderSnapshot
    public let leaderDeckID: UInt64
    public let decks: [DeckSnapshot]
    public let nextPlan: PlanSnapshot?
    public let planningOptions: PlanningOptionsSnapshot
    public let timeline: [TimelineEntrySnapshot]

    public init(
        endpoint: String,
        engineVersion: String,
        protocolVersion: Int,
        snapshotSequence: UInt64,
        stateRevision: UInt64,
        operationState: String = "off",
        runtime: RuntimeSnapshot,
        deckSource: DeckSourceSnapshot,
        simulation: SimulationSnapshot = .init(speed: 1, paused: false),
        outputProvider: OutputProviderSnapshot = .init(
            providerKind: "dryRun",
            status: "ready",
            recordCount: 0
        ),
        leaderDeckID: UInt64,
        decks: [DeckSnapshot],
        nextPlan: PlanSnapshot?,
        planningOptions: PlanningOptionsSnapshot,
        timeline: [TimelineEntrySnapshot] = []
    ) {
        self.endpoint = endpoint
        self.engineVersion = engineVersion
        self.protocolVersion = protocolVersion
        self.snapshotSequence = snapshotSequence
        self.stateRevision = stateRevision
        self.operationState = operationState
        self.runtime = runtime
        self.deckSource = deckSource
        self.simulation = simulation
        self.outputProvider = outputProvider
        self.leaderDeckID = leaderDeckID
        self.decks = decks
        self.nextPlan = nextPlan
        self.planningOptions = planningOptions
        self.timeline = timeline
    }
}

public struct SimulationSnapshot: Equatable, Sendable {
    public let speed: UInt64
    public let paused: Bool

    public init(speed: UInt64, paused: Bool) {
        self.speed = speed
        self.paused = paused
    }
}

public struct OutputProviderSnapshot: Equatable, Sendable {
    public let providerKind: String
    public let status: String
    public let recordCount: UInt64

    public init(providerKind: String, status: String, recordCount: UInt64) {
        self.providerKind = providerKind
        self.status = status
        self.recordCount = recordCount
    }
}

public struct TimelineEntrySnapshot: Equatable, Identifiable, Sendable {
    public let sequence: UInt64
    public let occurredAt: UInt64
    public let source: String
    public let type: String
    public let result: String
    public let reason: String

    public var id: UInt64 { sequence }

    public init(
        sequence: UInt64,
        occurredAt: UInt64,
        source: String,
        type: String,
        result: String,
        reason: String
    ) {
        self.sequence = sequence
        self.occurredAt = occurredAt
        self.source = source
        self.type = type
        self.result = result
        self.reason = reason
    }
}

public struct RuntimeSnapshot: Equatable, Sendable {
    public let model: String
    public let health: String
    public let queueCapacity: UInt64
    public let queueDepth: UInt64
    public let processedEvents: UInt64
    public let lastDecision: String

    public init(
        model: String,
        health: String,
        queueCapacity: UInt64,
        queueDepth: UInt64,
        processedEvents: UInt64,
        lastDecision: String
    ) {
        self.model = model
        self.health = health
        self.queueCapacity = queueCapacity
        self.queueDepth = queueDepth
        self.processedEvents = processedEvents
        self.lastDecision = lastDecision
    }
}

public struct DeckSourceSnapshot: Equatable, Sendable {
    public let providerKind: String
    public let status: String

    public init(providerKind: String, status: String) {
        self.providerKind = providerKind
        self.status = status
    }
}

public struct DeckSnapshot: Equatable, Identifiable, Sendable {
    public let deckID: UInt64
    public let trackLoadID: UInt64
    public let title: String
    public let artist: String
    public let bpmMilli: UInt64
    public let colorRGB: UInt64?
    public let pitchClass: String
    public let keyMode: String
    public let beat: UInt64
    public let phraseIndex: UInt64?

    public var id: UInt64 { deckID }

    public init(
        deckID: UInt64,
        trackLoadID: UInt64,
        title: String,
        artist: String,
        bpmMilli: UInt64,
        colorRGB: UInt64? = nil,
        pitchClass: String,
        keyMode: String,
        beat: UInt64,
        phraseIndex: UInt64?
    ) {
        self.deckID = deckID
        self.trackLoadID = trackLoadID
        self.title = title
        self.artist = artist
        self.bpmMilli = bpmMilli
        self.colorRGB = colorRGB
        self.pitchClass = pitchClass
        self.keyMode = keyMode
        self.beat = beat
        self.phraseIndex = phraseIndex
    }
}

public struct PlanSnapshot: Equatable, Sendable {
    public let planID: String
    public let deckID: UInt64
    public let trackLoadID: UInt64
    public let trackDurationBeats: UInt64
    public let revision: UInt64
    public let configurationRevision: UInt64
    public let status: String
    public let themeDecision: ThemeDecisionSnapshot?
    public let cues: [PlanCueSnapshot]

    public init(
        planID: String,
        deckID: UInt64,
        trackLoadID: UInt64,
        trackDurationBeats: UInt64,
        revision: UInt64,
        configurationRevision: UInt64,
        status: String,
        themeDecision: ThemeDecisionSnapshot? = nil,
        cues: [PlanCueSnapshot]
    ) {
        self.planID = planID
        self.deckID = deckID
        self.trackLoadID = trackLoadID
        self.trackDurationBeats = trackDurationBeats
        self.revision = revision
        self.configurationRevision = configurationRevision
        self.status = status
        self.themeDecision = themeDecision
        self.cues = cues
    }
}

public struct ThemeDecisionSnapshot: Equatable, Sendable {
    public let themeID: UInt64
    public let themeName: String
    public let reason: String
    public let matchedColorRGB: UInt64?

    public init(
        themeID: UInt64,
        themeName: String,
        reason: String,
        matchedColorRGB: UInt64?
    ) {
        self.themeID = themeID
        self.themeName = themeName
        self.reason = reason
        self.matchedColorRGB = matchedColorRGB
    }
}

public struct PlanCueSnapshot: Equatable, Identifiable, Sendable {
    public let phraseIndex: UInt64
    public let startBeat: UInt64
    public let endBeat: UInt64
    public let origin: String
    public let locked: Bool
    public let reason: PlanReasonSnapshot
    public let action: PlanActionSnapshot

    public var id: UInt64 { phraseIndex }

    public init(
        phraseIndex: UInt64,
        startBeat: UInt64,
        endBeat: UInt64,
        origin: String,
        locked: Bool,
        reason: PlanReasonSnapshot,
        action: PlanActionSnapshot
    ) {
        self.phraseIndex = phraseIndex
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.origin = origin
        self.locked = locked
        self.reason = reason
        self.action = action
    }
}

public enum PlanReasonSnapshot: Equatable, Sendable {
    case phraseCategoryMatched(phraseKind: String, category: String)
    case missingPhraseAnalysis
}

public enum PlanActionSnapshot: Equatable, Sendable {
    case applyLook(
        themeID: UInt64,
        themeName: String,
        sceneID: UInt64,
        sceneName: String,
        category: String,
        loopBank: UInt64,
        loopSlot: UInt64
    )
    case holdCurrentLook
}

public struct PlanningOptionsSnapshot: Equatable, Sendable {
    public let themes: [ThemeOptionSnapshot]
    public let scenes: [SceneOptionSnapshot]

    public init(themes: [ThemeOptionSnapshot], scenes: [SceneOptionSnapshot]) {
        self.themes = themes
        self.scenes = scenes
    }
}

public struct ThemeOptionSnapshot: Equatable, Identifiable, Sendable {
    public let id: UInt64
    public let name: String

    public init(id: UInt64, name: String) {
        self.id = id
        self.name = name
    }
}

public struct SceneOptionSnapshot: Equatable, Identifiable, Sendable {
    public let id: UInt64
    public let name: String
    public let category: String
    public let loopBank: UInt64
    public let loopSlot: UInt64

    public init(
        id: UInt64,
        name: String,
        category: String,
        loopBank: UInt64,
        loopSlot: UInt64
    ) {
        self.id = id
        self.name = name
        self.category = category
        self.loopBank = loopBank
        self.loopSlot = loopSlot
    }
}
