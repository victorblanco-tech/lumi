import Foundation

public struct EngineSnapshot: Equatable, Sendable {
    public let endpoint: String
    public let engineVersion: String
    public let protocolVersion: Int
    public let snapshotSequence: UInt64
    public let stateRevision: UInt64
    public let runtime: RuntimeSnapshot
    public let deckSource: DeckSourceSnapshot
    public let leaderDeckID: UInt64
    public let decks: [DeckSnapshot]
    public let nextPlan: PlanSnapshot?

    public init(
        endpoint: String,
        engineVersion: String,
        protocolVersion: Int,
        snapshotSequence: UInt64,
        stateRevision: UInt64,
        runtime: RuntimeSnapshot,
        deckSource: DeckSourceSnapshot,
        leaderDeckID: UInt64,
        decks: [DeckSnapshot],
        nextPlan: PlanSnapshot?
    ) {
        self.endpoint = endpoint
        self.engineVersion = engineVersion
        self.protocolVersion = protocolVersion
        self.snapshotSequence = snapshotSequence
        self.stateRevision = stateRevision
        self.runtime = runtime
        self.deckSource = deckSource
        self.leaderDeckID = leaderDeckID
        self.decks = decks
        self.nextPlan = nextPlan
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
        self.pitchClass = pitchClass
        self.keyMode = keyMode
        self.beat = beat
        self.phraseIndex = phraseIndex
    }
}

public struct PlanSnapshot: Equatable, Sendable {
    public let deckID: UInt64
    public let trackLoadID: UInt64
    public let trackDurationBeats: UInt64
    public let revision: UInt64
    public let configurationRevision: UInt64
    public let status: String
    public let cues: [PlanCueSnapshot]

    public init(
        deckID: UInt64,
        trackLoadID: UInt64,
        trackDurationBeats: UInt64,
        revision: UInt64,
        configurationRevision: UInt64,
        status: String,
        cues: [PlanCueSnapshot]
    ) {
        self.deckID = deckID
        self.trackLoadID = trackLoadID
        self.trackDurationBeats = trackDurationBeats
        self.revision = revision
        self.configurationRevision = configurationRevision
        self.status = status
        self.cues = cues
    }
}

public struct PlanCueSnapshot: Equatable, Identifiable, Sendable {
    public let phraseIndex: UInt64
    public let startBeat: UInt64
    public let endBeat: UInt64
    public let origin: String
    public let reason: PlanReasonSnapshot
    public let action: PlanActionSnapshot

    public var id: UInt64 { phraseIndex }

    public init(
        phraseIndex: UInt64,
        startBeat: UInt64,
        endBeat: UInt64,
        origin: String,
        reason: PlanReasonSnapshot,
        action: PlanActionSnapshot
    ) {
        self.phraseIndex = phraseIndex
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.origin = origin
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
        themeName: String,
        sceneName: String,
        category: String,
        loopBank: UInt64,
        loopSlot: UInt64
    )
    case holdCurrentLook
}
