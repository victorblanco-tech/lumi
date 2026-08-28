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
    public let deckInputIntegration: DeckInputIntegrationSnapshot?
    public let midiIntegration: MidiOutputIntegrationSnapshot?
    public let midiClockIntegration: MidiClockIntegrationSnapshot?
    public let abletonLinkIntegration: AbletonLinkIntegrationSnapshot?
    public let simulation: SimulationSnapshot?
    public let outputProvider: OutputProviderSnapshot
    public let leaderDeckID: UInt64?
    public let decks: [DeckSnapshot]
    public let livePlan: PlanSnapshot?
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
        deckInputIntegration: DeckInputIntegrationSnapshot? = nil,
        midiIntegration: MidiOutputIntegrationSnapshot? = nil,
        midiClockIntegration: MidiClockIntegrationSnapshot? = nil,
        abletonLinkIntegration: AbletonLinkIntegrationSnapshot? = nil,
        simulation: SimulationSnapshot? = nil,
        outputProvider: OutputProviderSnapshot = .init(
            providerKind: "dryRun",
            status: "ready",
            recordCount: 0
        ),
        leaderDeckID: UInt64?,
        decks: [DeckSnapshot],
        livePlan: PlanSnapshot? = nil,
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
        self.deckInputIntegration = deckInputIntegration
        self.midiIntegration = midiIntegration
        self.midiClockIntegration = midiClockIntegration
        self.abletonLinkIntegration = abletonLinkIntegration
        self.simulation = simulation
        self.outputProvider = outputProvider
        self.leaderDeckID = leaderDeckID
        self.decks = decks
        self.livePlan = livePlan
        self.nextPlan = nextPlan
        self.planningOptions = planningOptions
        self.timeline = timeline
    }

    public func optimisticallySettingLocalPlaybackLeader(_ deckID: UInt64) -> Self {
        guard deckSource.mode == "localPlayback",
              leaderDeckID != deckID,
              decks.contains(where: { $0.deckID == deckID }) else {
            return self
        }
        let availablePlans = [livePlan, nextPlan].compactMap { $0 }
        return Self(
            endpoint: endpoint,
            engineVersion: engineVersion,
            protocolVersion: protocolVersion,
            snapshotSequence: snapshotSequence,
            stateRevision: stateRevision,
            operationState: operationState,
            runtime: runtime,
            deckSource: deckSource,
            deckInputIntegration: deckInputIntegration,
            midiIntegration: midiIntegration,
            midiClockIntegration: midiClockIntegration,
            abletonLinkIntegration: abletonLinkIntegration,
            simulation: simulation,
            outputProvider: outputProvider,
            leaderDeckID: deckID,
            decks: decks,
            livePlan: availablePlans.first(where: { $0.deckID == deckID }),
            nextPlan: availablePlans.first(where: { $0.deckID != deckID }),
            planningOptions: planningOptions,
            timeline: timeline
        )
    }

    public func optimisticallySettingOperationState(_ state: String) -> Self {
        guard ["off", "armed", "live", "paused"].contains(state),
              operationState != state else {
            return self
        }
        return Self(
            endpoint: endpoint,
            engineVersion: engineVersion,
            protocolVersion: protocolVersion,
            snapshotSequence: snapshotSequence,
            stateRevision: stateRevision,
            operationState: state,
            runtime: runtime,
            deckSource: deckSource,
            deckInputIntegration: deckInputIntegration,
            midiIntegration: midiIntegration,
            midiClockIntegration: midiClockIntegration,
            abletonLinkIntegration: abletonLinkIntegration,
            simulation: simulation,
            outputProvider: outputProvider,
            leaderDeckID: leaderDeckID,
            decks: decks,
            livePlan: livePlan,
            nextPlan: nextPlan,
            planningOptions: planningOptions,
            timeline: timeline
        )
    }
}

public struct MidiOutputIntegrationSnapshot: Equatable, Sendable {
    public let state: String
    public let sourceName: String
    public let protocolName: String
    public let sentPulseCount: UInt64
    public let lastEvent: String?
    public let lastError: String?
    public let activeBank: UInt64?
    public let autoPublishEnabled: Bool
    public let timingOffsetMillis: Int
    public let pendingTimingOffsetMillis: Int?
    public let bankPreRollMillis: UInt64
    public let realtimeLane: RealtimeMidiOutputLaneSnapshot?

    public init(
        state: String,
        sourceName: String,
        protocolName: String,
        sentPulseCount: UInt64,
        lastEvent: String?,
        lastError: String?,
        activeBank: UInt64?,
        autoPublishEnabled: Bool,
        timingOffsetMillis: Int,
        pendingTimingOffsetMillis: Int? = nil,
        bankPreRollMillis: UInt64 = 50,
        realtimeLane: RealtimeMidiOutputLaneSnapshot? = nil
    ) {
        self.state = state
        self.sourceName = sourceName
        self.protocolName = protocolName
        self.sentPulseCount = sentPulseCount
        self.lastEvent = lastEvent
        self.lastError = lastError
        self.activeBank = activeBank
        self.autoPublishEnabled = autoPublishEnabled
        self.timingOffsetMillis = timingOffsetMillis
        self.pendingTimingOffsetMillis = pendingTimingOffsetMillis
        self.bankPreRollMillis = bankPreRollMillis
        self.realtimeLane = realtimeLane
    }
}

public struct RealtimeMidiOutputLaneSnapshot: Equatable, Sendable {
    public let queueCapacity: UInt64
    public let queueDepth: UInt64
    public let queueHighWater: UInt64
    public let saturationCount: UInt64
    public let latencySampleCount: UInt64
    public let latencyP95Micros: UInt64
    public let lastDispatchLatenessMicros: UInt64
    public let lateDispatchCount: UInt64

    public var isHealthy: Bool {
        saturationCount == 0 && (latencySampleCount == 0 || latencyP95Micros <= 20_000)
    }
}

public struct MidiClockIntegrationSnapshot: Equatable, Sendable {
    public let state: String
    public let sourceName: String
    public let protocolName: String
    public let bpmMilli: UInt64?
    public let sentTickCount: UInt64
    public let lastEvent: String?
    public let lastError: String?

    public init(
        state: String,
        sourceName: String,
        protocolName: String,
        bpmMilli: UInt64?,
        sentTickCount: UInt64,
        lastEvent: String?,
        lastError: String?
    ) {
        self.state = state
        self.sourceName = sourceName
        self.protocolName = protocolName
        self.bpmMilli = bpmMilli
        self.sentTickCount = sentTickCount
        self.lastEvent = lastEvent
        self.lastError = lastError
    }
}

public struct AbletonLinkIntegrationSnapshot: Equatable, Sendable {
    public let enabled: Bool
    public let state: String
    public let provider: String
    public let helperVersion: String?
    public let peers: UInt64
    public let source: String?
    public let deckNumber: UInt64?
    public let bpmMilli: UInt64?
    public let beatWithinBar: UInt64?
    public let playing: Bool
    public let generation: UInt64?
    public let lastBeatAgeMillis: UInt64?
    public let phaseErrorMicros: Int?
    public let lastReanchor: String?
    public let lastEvent: String?
    public let lastError: String?
}

public struct DeckInputIntegrationSnapshot: Equatable, Sendable {
    public let state: String
    public let destinationName: String?
    public let protocolName: String
    public let protocolVersion: UInt64
    public let receivedMessageCount: UInt64
    public let invalidWordCount: UInt64
    public let committedFrameCount: UInt64
    public let ignoredMessageCount: UInt64
    public let duplicateFrameCount: UInt64
    public let lastDeckID: UInt64?
    public let lastFrameSequence: UInt64?
    public let precisePositionMessageCount: UInt64
    public let authoritativePositionCount: UInt64
    public let positionDiscontinuityCount: UInt64
    public let positionAuthorityReady: Bool

    public init(
        state: String,
        destinationName: String?,
        protocolName: String,
        protocolVersion: UInt64,
        receivedMessageCount: UInt64,
        invalidWordCount: UInt64,
        committedFrameCount: UInt64,
        ignoredMessageCount: UInt64,
        duplicateFrameCount: UInt64,
        lastDeckID: UInt64?,
        lastFrameSequence: UInt64?,
        precisePositionMessageCount: UInt64 = 0,
        authoritativePositionCount: UInt64 = 0,
        positionDiscontinuityCount: UInt64 = 0,
        positionAuthorityReady: Bool = false
    ) {
        self.state = state
        self.destinationName = destinationName
        self.protocolName = protocolName
        self.protocolVersion = protocolVersion
        self.receivedMessageCount = receivedMessageCount
        self.invalidWordCount = invalidWordCount
        self.committedFrameCount = committedFrameCount
        self.ignoredMessageCount = ignoredMessageCount
        self.duplicateFrameCount = duplicateFrameCount
        self.lastDeckID = lastDeckID
        self.lastFrameSequence = lastFrameSequence
        self.precisePositionMessageCount = precisePositionMessageCount
        self.authoritativePositionCount = authoritativePositionCount
        self.positionDiscontinuityCount = positionDiscontinuityCount
        self.positionAuthorityReady = positionAuthorityReady
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
    public let mode: String
    public let displayName: String
    public let status: String

    public init(
        providerKind: String,
        mode: String = "localPlayback",
        displayName: String = "Local Playback",
        status: String
    ) {
        self.providerKind = providerKind
        self.mode = mode
        self.displayName = displayName
        self.status = status
    }
}

public enum DeckPlanEligibility: String, Equatable, Sendable {
    case readyExact
    case readyTransient
    case autoHeld
}

public struct LocalPlaybackTrackSnapshot: Equatable, Sendable {
    public let audioURI: String
    public let durationMillis: UInt64

    public init(audioURI: String, durationMillis: UInt64) {
        self.audioURI = audioURI
        self.durationMillis = durationMillis
    }
}

/// Lightweight app-side clock used to render local and connected decks smoothly.
/// Authoritative transport, phrase and planning state still comes from the engine.
public struct DeckVisualClockSnapshot: Equatable, Sendable {
    public let trackLoadID: UInt64
    public let positionMillis: UInt64
    public let durationMillis: UInt64
    public let playing: Bool
    public let anchoredAtReferenceTime: TimeInterval
    public let playbackRate: Double
    public let discontinuityRevision: UInt64

    public init(
        trackLoadID: UInt64,
        positionMillis: UInt64,
        durationMillis: UInt64,
        playing: Bool,
        anchoredAtReferenceTime: TimeInterval,
        playbackRate: Double = 1,
        discontinuityRevision: UInt64 = 0
    ) {
        self.trackLoadID = trackLoadID
        self.positionMillis = positionMillis
        self.durationMillis = durationMillis
        self.playing = playing
        self.anchoredAtReferenceTime = anchoredAtReferenceTime
        self.playbackRate = max(0, playbackRate)
        self.discontinuityRevision = discontinuityRevision
    }

    public func positionMillis(at date: Date) -> Double {
        let elapsed = playing
            ? max(0, date.timeIntervalSinceReferenceDate - anchoredAtReferenceTime)
                * 1_000 * playbackRate
            : 0
        return min(Double(durationMillis), Double(positionMillis) + elapsed)
    }

    /// Returns whether this monotonic presentation clock can keep rendering
    /// without replacing it from the next authoritative deck poll.
    public func remainsValid(
        trackLoadID authoritativeTrackLoadID: UInt64,
        positionMillis authoritativePositionMillis: UInt64,
        durationMillis authoritativeDurationMillis: UInt64,
        playing authoritativePlaying: Bool,
        playbackRate authoritativePlaybackRate: Double,
        discontinuityRevision authoritativeDiscontinuityRevision: UInt64,
        at referenceTime: TimeInterval,
        maximumPlayingDriftMillis: Double = 250
    ) -> Bool {
        guard trackLoadID == authoritativeTrackLoadID,
              playing == authoritativePlaying,
              discontinuityRevision == authoritativeDiscontinuityRevision,
              durationMillis == authoritativeDurationMillis,
              abs(playbackRate - authoritativePlaybackRate) < 0.005 else {
            return false
        }
        if !playing {
            return positionMillis == authoritativePositionMillis
        }
        // An older network observation may never rewind the monotonic visual
        // clock. A newer authoritative position, however, must be allowed to
        // pull a clock forward after SwiftUI/app-mode work delayed its anchor.
        // Without this one-way drift bound the deck could remain seconds behind
        // forever because every later poll was treated as equivalent.
        let predicted = positionMillis(
            at: Date(timeIntervalSinceReferenceDate: referenceTime)
        )
        let forwardDrift = Double(authoritativePositionMillis) - predicted
        return forwardDrift <= maximumPlayingDriftMillis
    }
}

public struct DeckSnapshot: Equatable, Identifiable, Sendable {
    public let deckID: UInt64
    public let trackLoadID: UInt64
    public let title: String
    public let artist: String
    public let bpmMilli: UInt64
    public let colorRGB: UInt32?
    public let pitchClass: String
    public let keyMode: String
    public let keyKnown: Bool
    public let beat: UInt64
    public let playing: Bool
    public let playbackPositionMillis: UInt64?
    public let transportRevision: UInt64
    public let phraseIndex: UInt64?
    public let durationBeats: UInt64
    public let beatGrid: DeckBeatGridSnapshot?
    public let phrases: [DeckPhraseSnapshot]
    public let waveformPreview: DeckWaveformPreviewSnapshot?
    public let hotCues: [DeckHotCueSnapshot]
    public let planEligibility: DeckPlanEligibility
    public let planHoldReason: String?
    public let localPlayback: LocalPlaybackTrackSnapshot?

    public var id: UInt64 { deckID }

    public init(
        deckID: UInt64,
        trackLoadID: UInt64,
        title: String,
        artist: String,
        bpmMilli: UInt64,
        colorRGB: UInt32? = nil,
        pitchClass: String,
        keyMode: String,
        keyKnown: Bool = true,
        beat: UInt64,
        playing: Bool = false,
        playbackPositionMillis: UInt64? = nil,
        transportRevision: UInt64 = 0,
        phraseIndex: UInt64?,
        durationBeats: UInt64 = 0,
        beatGrid: DeckBeatGridSnapshot? = nil,
        phrases: [DeckPhraseSnapshot] = [],
        waveformPreview: DeckWaveformPreviewSnapshot? = nil,
        hotCues: [DeckHotCueSnapshot] = [],
        planEligibility: DeckPlanEligibility = .autoHeld,
        planHoldReason: String? = nil,
        localPlayback: LocalPlaybackTrackSnapshot? = nil
    ) {
        self.deckID = deckID
        self.trackLoadID = trackLoadID
        self.title = title
        self.artist = artist
        self.bpmMilli = bpmMilli
        self.colorRGB = colorRGB
        self.pitchClass = pitchClass
        self.keyMode = keyMode
        self.keyKnown = keyKnown
        self.beat = beat
        self.playing = playing
        self.playbackPositionMillis = playbackPositionMillis
        self.transportRevision = transportRevision
        self.phraseIndex = phraseIndex
        self.durationBeats = durationBeats
        self.beatGrid = beatGrid
        self.phrases = phrases
        self.waveformPreview = waveformPreview
        self.hotCues = hotCues
        self.planEligibility = planEligibility
        self.planHoldReason = planHoldReason
        self.localPlayback = localPlayback
    }
}

public struct DeckHotCueSnapshot: Equatable, Identifiable, Sendable {
    public var id: UInt8 { index }
    public let index: UInt8
    public let timeMillis: UInt64
    public let loopEndMillis: UInt64?
    public let name: String
    public let colorRGB: UInt32

    public init(
        index: UInt8,
        timeMillis: UInt64,
        loopEndMillis: UInt64? = nil,
        name: String,
        colorRGB: UInt32
    ) {
        self.index = index
        self.timeMillis = timeMillis
        self.loopEndMillis = loopEndMillis
        self.name = name
        self.colorRGB = colorRGB
    }

    public var letter: String {
        UnicodeScalar(64 + Int(index)).map(String.init) ?? "?"
    }
}

public struct DeckBeatGridSnapshot: Equatable, Sendable {
    public let beatsPerBar: UInt8
    public let durationMillis: UInt64
    public let timesMillis: [UInt64]

    public init(
        beatsPerBar: UInt8,
        durationMillis: UInt64,
        timesMillis: [UInt64]
    ) {
        self.beatsPerBar = beatsPerBar
        self.durationMillis = durationMillis
        self.timesMillis = timesMillis
    }
}

public struct DeckPhraseSnapshot: Equatable, Identifiable, Sendable {
    public let index: UInt64
    public let startBeat: UInt64
    public let endBeat: UInt64
    public let kind: String
    public let roleID: String?
    public let roleName: String?

    public var id: UInt64 { index }

    public init(
        index: UInt64,
        startBeat: UInt64,
        endBeat: UInt64,
        kind: String,
        roleID: String? = nil,
        roleName: String? = nil
    ) {
        self.index = index
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.kind = kind
        self.roleID = roleID
        self.roleName = roleName
    }
}

public struct DeckWaveformPreviewSnapshot: Equatable, Sendable {
    public let source: String
    public let style: String
    public let points: [DeckWaveformPointSnapshot]

    public init(source: String, style: String, points: [DeckWaveformPointSnapshot]) {
        self.source = source
        self.style = style
        self.points = points
    }
}

public struct LibraryWaveformDetailSnapshot: Equatable, Sendable {
    public let trackID: UInt64
    public let preview: DeckWaveformPreviewSnapshot

    public init(trackID: UInt64, preview: DeckWaveformPreviewSnapshot) {
        self.trackID = trackID
        self.preview = preview
    }
}

public struct DeckWaveformPointSnapshot: Equatable, Sendable {
    public let low: UInt8
    public let mid: UInt8
    public let high: UInt8

    public init(low: UInt8, mid: UInt8, high: UInt8) {
        self.low = low
        self.mid = mid
        self.high = high
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
    public let libraryTrack: PlanLibraryTrackSnapshot?
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
        libraryTrack: PlanLibraryTrackSnapshot? = nil,
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
        self.libraryTrack = libraryTrack
        self.cues = cues
    }
}

public struct PlanLibraryTrackSnapshot: Equatable, Sendable {
    public let providerKind: String
    public let sourceID: String
    public let sourceName: String
    public let sourceTrackID: String
    public let analysisRevision: String
    public let timelineRevision: UInt64

    public init(
        providerKind: String,
        sourceID: String,
        sourceName: String,
        sourceTrackID: String,
        analysisRevision: String,
        timelineRevision: UInt64
    ) {
        self.providerKind = providerKind
        self.sourceID = sourceID
        self.sourceName = sourceName
        self.sourceTrackID = sourceTrackID
        self.analysisRevision = analysisRevision
        self.timelineRevision = timelineRevision
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
    public let libraryResolution: PlanCueLibraryResolutionSnapshot?

    public var id: UInt64 { phraseIndex }

    public init(
        phraseIndex: UInt64,
        startBeat: UInt64,
        endBeat: UInt64,
        origin: String,
        locked: Bool,
        reason: PlanReasonSnapshot,
        action: PlanActionSnapshot,
        libraryResolution: PlanCueLibraryResolutionSnapshot? = nil
    ) {
        self.phraseIndex = phraseIndex
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.origin = origin
        self.locked = locked
        self.reason = reason
        self.action = action
        self.libraryResolution = libraryResolution
    }
}

public struct PlanCueLibraryResolutionSnapshot: Equatable, Sendable {
    public let roleID: String
    public let roleName: String
    public let strategy: String
    public let variantID: String
    public let catalogRevision: UInt64
    public let resolutionReason: String
    public let entryID: String
    public let entryName: String
    public let bankNumber: UInt64?
    public let autoloopNumber: UInt64?
    public let choices: [PlanAutoloopChoiceSnapshot]
    public let modifierChoices: [PlanModifierChoiceSnapshot]

    public init(
        roleID: String,
        roleName: String,
        strategy: String,
        variantID: String,
        catalogRevision: UInt64,
        resolutionReason: String,
        entryID: String,
        entryName: String,
        bankNumber: UInt64? = nil,
        autoloopNumber: UInt64? = nil,
        choices: [PlanAutoloopChoiceSnapshot] = [],
        modifierChoices: [PlanModifierChoiceSnapshot] = []
    ) {
        self.roleID = roleID
        self.roleName = roleName
        self.strategy = strategy
        self.variantID = variantID
        self.catalogRevision = catalogRevision
        self.resolutionReason = resolutionReason
        self.entryID = entryID
        self.entryName = entryName
        self.bankNumber = bankNumber
        self.autoloopNumber = autoloopNumber
        self.choices = choices
        self.modifierChoices = modifierChoices
    }
}

public struct PlanModifierChoiceSnapshot: Equatable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let kind: String
    public let scope: String
    public let midiChannel: UInt8
    public let midiNote: UInt8
}

public struct PlanAutoloopChoiceSnapshot: Equatable, Identifiable, Sendable {
    public let id: UInt64
    public let name: String
    public let variantID: String
    public let bankNumber: UInt64

    public init(id: UInt64, name: String, variantID: String, bankNumber: UInt64) {
        self.id = id
        self.name = name
        self.variantID = variantID
        self.bankNumber = bankNumber
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
