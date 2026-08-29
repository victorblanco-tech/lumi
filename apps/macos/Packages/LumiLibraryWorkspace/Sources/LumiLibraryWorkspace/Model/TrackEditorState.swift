import Foundation

public struct TrackEditorBeat: Equatable, Sendable {
    public let beatIndex: UInt32
    public let timeMillis: UInt64
    public let barIndex: UInt32
    public let beatInBar: UInt8

    public init(beatIndex: UInt32, timeMillis: UInt64, barIndex: UInt32, beatInBar: UInt8) {
        self.beatIndex = beatIndex
        self.timeMillis = timeMillis
        self.barIndex = barIndex
        self.beatInBar = beatInBar
    }
}

public struct TrackEditorWaveformPoint: Equatable, Sendable {
    public let low: UInt8
    public let mid: UInt8
    public let high: UInt8

    public init(low: UInt8, mid: UInt8, high: UInt8) {
        self.low = low
        self.mid = mid
        self.high = high
    }
}

public struct TrackEditorHotCue: Identifiable, Equatable, Sendable {
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

public struct TrackEditorThemeVariantOverride: Equatable, Sendable {
    public let themeID: UInt64
    public let variantID: String

    public init(themeID: UInt64, variantID: String) {
        self.themeID = themeID
        self.variantID = variantID
    }
}

public struct TrackEditorLoopStrategyIssue: Equatable, Sendable {
    public let reason: String
    public let themeID: UInt64
    public let variantID: String?

    public init(reason: String, themeID: UInt64, variantID: String?) {
        self.reason = reason
        self.themeID = themeID
        self.variantID = variantID
    }
}

public struct TrackEditorLoopStrategy: Equatable, Sendable {
    public static let automatic = Self(
        kind: "auto",
        locked: false,
        provenance: "automaticDefault",
        rowRoleID: "",
        fixedVariantID: nil,
        themeOverrides: [],
        validatedCatalogRevision: 1,
        status: "ready",
        issues: []
    )

    public let kind: String
    public let locked: Bool
    public let provenance: String
    public let rowRoleID: String
    public let fixedVariantID: String?
    public let themeOverrides: [TrackEditorThemeVariantOverride]
    public let validatedCatalogRevision: UInt64
    public let status: String
    public let issues: [TrackEditorLoopStrategyIssue]

    public init(
        kind: String,
        locked: Bool,
        provenance: String,
        rowRoleID: String,
        fixedVariantID: String?,
        themeOverrides: [TrackEditorThemeVariantOverride],
        validatedCatalogRevision: UInt64,
        status: String,
        issues: [TrackEditorLoopStrategyIssue]
    ) {
        self.kind = kind
        self.locked = locked
        self.provenance = provenance
        self.rowRoleID = rowRoleID
        self.fixedVariantID = fixedVariantID
        self.themeOverrides = themeOverrides
        self.validatedCatalogRevision = validatedCatalogRevision
        self.status = status
        self.issues = issues
    }
}

public struct TrackEditorPhrase: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let startBeat: UInt32
    public let endBeat: UInt32
    public let roleID: String
    public let role: String
    public let origin: String
    public let loopStrategy: TrackEditorLoopStrategy

    public init(
        id: UInt64,
        startBeat: UInt32,
        endBeat: UInt32,
        roleID: String,
        role: String,
        origin: String,
        loopStrategy: TrackEditorLoopStrategy = .automatic
    ) {
        self.id = id
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.roleID = roleID
        self.role = role
        self.origin = origin
        self.loopStrategy = loopStrategy
    }
}

public struct TrackEditorRole: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let archived: Bool

    public init(id: String, name: String, archived: Bool = false) {
        self.id = id
        self.name = name
        self.archived = archived
    }
}

public struct TrackEditorSourcePhrase: Equatable, Sendable {
    public let startBeat: UInt32
    public let endBeat: UInt32
    public let rawLabel: String
    public let providerKind: String

    public init(startBeat: UInt32, endBeat: UInt32, rawLabel: String, providerKind: String) {
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.rawLabel = rawLabel
        self.providerKind = providerKind
    }
}

public struct TrackEditorRevision: Identifiable, Equatable, Sendable {
    public var id: UInt64 { revision }

    public let revision: UInt64
    public let origin: String
    public let reason: String
    public let phraseCount: UInt32
    public let restoredFrom: UInt64?

    public init(
        revision: UInt64,
        origin: String,
        reason: String,
        phraseCount: UInt32,
        restoredFrom: UInt64?
    ) {
        self.revision = revision
        self.origin = origin
        self.reason = reason
        self.phraseCount = phraseCount
        self.restoredFrom = restoredFrom
    }
}

public struct TrackEditorTimeline: Equatable, Sendable {
    public let revision: UInt64
    public let baselineRevision: String
    public let origin: String
    public let reason: String
    public let canUndo: Bool
    public let canRedo: Bool
    public let revisions: [TrackEditorRevision]

    public init(
        revision: UInt64,
        baselineRevision: String,
        origin: String,
        reason: String,
        canUndo: Bool,
        canRedo: Bool,
        revisions: [TrackEditorRevision]
    ) {
        self.revision = revision
        self.baselineRevision = baselineRevision
        self.origin = origin
        self.reason = reason
        self.canUndo = canUndo
        self.canRedo = canRedo
        self.revisions = revisions
    }
}

public enum TrackTimelineEditRequest: Equatable, Sendable {
    case create(startBeat: UInt32, endBeat: UInt32, roleID: String)
    case split(phraseIndex: UInt16, atBeat: UInt32)
    case mergePrevious(phraseIndex: UInt16)
    case mergeNext(phraseIndex: UInt16)
    case moveBoundary(afterPhraseIndex: UInt16, toBeat: UInt32)
    case deleteAbsorbPrevious(phraseIndex: UInt16)
    case deleteAbsorbNext(phraseIndex: UInt16)
    case changeRole(phraseIndex: UInt16, roleID: String)
    case setLoopStrategy(phraseIndex: UInt16, strategy: TrackLoopStrategyRequest)
}

public enum TrackLoopStrategyRequest: Equatable, Sendable {
    case automatic
    case fixedVariant(String)
    case themeSpecificExact([TrackEditorThemeVariantOverride])
}

public enum TrackTimelineHistoryRequest: Equatable, Sendable {
    case undo
    case redo
    case restore(revision: UInt64)
}

public struct TrackSourcePhraseVersion: Equatable, Sendable {
    public let startBeat: UInt32
    public let endBeat: UInt32
    public let roleID: String

    public init(startBeat: UInt32, endBeat: UInt32, roleID: String) {
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.roleID = roleID
    }
}

public struct TrackSourceConflict: Identifiable, Equatable, Sendable {
    public var id: UInt16 { phraseIndex }
    public let phraseIndex: UInt16
    public let lumi: TrackSourcePhraseVersion?
    public let source: TrackSourcePhraseVersion?

    public init(
        phraseIndex: UInt16,
        lumi: TrackSourcePhraseVersion?,
        source: TrackSourcePhraseVersion?
    ) {
        self.phraseIndex = phraseIndex
        self.lumi = lumi
        self.source = source
    }
}

public struct TrackSourceReconciliation: Equatable, Sendable {
    public let fromRevision: String
    public let toRevision: String
    public let sourceLibraryRevision: String
    public let changes: [String]
    public let metadataOnly: Bool
    public let requiresTimelineDecision: Bool
    public let sourceTotalBeats: UInt32
    public let rebaseAmbiguities: [UInt16]
    public let conflicts: [TrackSourceConflict]

    public init(
        fromRevision: String,
        toRevision: String,
        sourceLibraryRevision: String,
        changes: [String],
        metadataOnly: Bool,
        requiresTimelineDecision: Bool,
        sourceTotalBeats: UInt32,
        rebaseAmbiguities: [UInt16],
        conflicts: [TrackSourceConflict]
    ) {
        self.fromRevision = fromRevision
        self.toRevision = toRevision
        self.sourceLibraryRevision = sourceLibraryRevision
        self.changes = changes
        self.metadataOnly = metadataOnly
        self.requiresTimelineDecision = requiresTimelineDecision
        self.sourceTotalBeats = sourceTotalBeats
        self.rebaseAmbiguities = rebaseAmbiguities
        self.conflicts = conflicts
    }
}

public enum TrackSourceConflictSide: String, Equatable, Sendable {
    case lumi
    case source
}

public struct TrackSourceConflictChoice: Equatable, Sendable {
    public let phraseIndex: UInt16
    public let side: TrackSourceConflictSide

    public init(phraseIndex: UInt16, side: TrackSourceConflictSide) {
        self.phraseIndex = phraseIndex
        self.side = side
    }
}

public enum TrackSourceReconcileRequest: Equatable, Sendable {
    case previewDemoChanges
    case keepLumi
    case rebase
    case merge([TrackSourceConflictChoice])
    case replaceWithSource
}

public struct TrackEditorAnalysis: Identifiable, Equatable, Sendable {
    public var id: UInt64 { track.id }

    public let track: LibraryTrack
    public let audioURI: String
    public let beatsPerBar: UInt8
    public let beats: [TrackEditorBeat]
    public let waveform: [TrackEditorWaveformPoint]
    public let hotCues: [TrackEditorHotCue]
    public let phrases: [TrackEditorPhrase]
    public let roles: [TrackEditorRole]
    public let sourcePhrases: [TrackEditorSourcePhrase]
    public let timeline: TrackEditorTimeline
    public let sourceReconciliation: TrackSourceReconciliation?
    public let creativeReuseCandidates: [CreativeTimelineCandidate]

    public init(
        track: LibraryTrack,
        audioURI: String,
        beatsPerBar: UInt8,
        beats: [TrackEditorBeat],
        waveform: [TrackEditorWaveformPoint],
        hotCues: [TrackEditorHotCue] = [],
        phrases: [TrackEditorPhrase],
        roles: [TrackEditorRole],
        sourcePhrases: [TrackEditorSourcePhrase] = [],
        timeline: TrackEditorTimeline,
        sourceReconciliation: TrackSourceReconciliation? = nil,
        creativeReuseCandidates: [CreativeTimelineCandidate] = []
    ) {
        self.track = track
        self.audioURI = audioURI
        self.beatsPerBar = beatsPerBar
        self.beats = beats
        self.waveform = waveform
        self.hotCues = hotCues
        self.phrases = phrases
        self.roles = roles
        self.sourcePhrases = sourcePhrases
        self.timeline = timeline
        self.sourceReconciliation = sourceReconciliation
        self.creativeReuseCandidates = creativeReuseCandidates
    }

    public var totalBars: UInt32 {
        guard beatsPerBar > 0 else { return 0 }
        return UInt32(beats.count) / UInt32(beatsPerBar)
    }

    public var totalBeats: UInt32 {
        UInt32(beats.count)
    }

    public func timeMillis(atBeat beat: UInt32) -> UInt64 {
        if let marker = beats.first(where: { $0.beatIndex == beat }) {
            return marker.timeMillis
        }
        return track.durationMillis
    }

    public func phraseTimeRange(_ phrase: TrackEditorPhrase) -> Range<UInt64> {
        timeMillis(atBeat: phrase.startBeat)..<timeMillis(atBeat: phrase.endBeat)
    }
}

public struct CreativeTimelineCandidate: Identifiable, Equatable, Sendable {
    public var id: UInt64 { trackID }
    public let trackID: UInt64
    public let title: String
    public let artist: String
    public let phraseCount: UInt64
    public let totalBeats: UInt32
    public let exactBeatCompatibility: Bool
    public let likelyVersion: Bool
    public let timelineRevision: UInt64
    public let bpmMilli: UInt32
    public let durationMillis: UInt64
    public let bpmDeltaMilli: Int64
    public let durationDeltaMillis: Int64
}

public struct CreativeTimelineReuseRequest: Equatable, Sendable {
    public let sourceTrackID: UInt64
    public let targetTrackID: UInt64
    public let expectedTargetRevision: UInt64

    public init(sourceTrackID: UInt64, targetTrackID: UInt64, expectedTargetRevision: UInt64) {
        self.sourceTrackID = sourceTrackID
        self.targetTrackID = targetTrackID
        self.expectedTargetRevision = expectedTargetRevision
    }
}
