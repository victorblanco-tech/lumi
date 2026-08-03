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
    case create(startBar: UInt32, endBar: UInt32, roleID: String)
    case split(phraseIndex: UInt16, atBar: UInt32)
    case mergePrevious(phraseIndex: UInt16)
    case mergeNext(phraseIndex: UInt16)
    case moveBoundary(afterPhraseIndex: UInt16, toBar: UInt32)
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

public struct TrackEditorAnalysis: Identifiable, Equatable, Sendable {
    public var id: UInt64 { track.id }

    public let track: LibraryTrack
    public let audioURI: String
    public let beatsPerBar: UInt8
    public let beats: [TrackEditorBeat]
    public let waveform: [TrackEditorWaveformPoint]
    public let phrases: [TrackEditorPhrase]
    public let roles: [TrackEditorRole]
    public let sourcePhrases: [TrackEditorSourcePhrase]
    public let timeline: TrackEditorTimeline

    public init(
        track: LibraryTrack,
        audioURI: String,
        beatsPerBar: UInt8,
        beats: [TrackEditorBeat],
        waveform: [TrackEditorWaveformPoint],
        phrases: [TrackEditorPhrase],
        roles: [TrackEditorRole],
        sourcePhrases: [TrackEditorSourcePhrase] = [],
        timeline: TrackEditorTimeline
    ) {
        self.track = track
        self.audioURI = audioURI
        self.beatsPerBar = beatsPerBar
        self.beats = beats
        self.waveform = waveform
        self.phrases = phrases
        self.roles = roles
        self.sourcePhrases = sourcePhrases
        self.timeline = timeline
    }

    public var totalBars: UInt32 {
        guard beatsPerBar > 0 else { return 0 }
        return UInt32(beats.count) / UInt32(beatsPerBar)
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
