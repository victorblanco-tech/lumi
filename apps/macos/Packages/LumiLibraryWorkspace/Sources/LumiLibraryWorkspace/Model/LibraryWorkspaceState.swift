import Foundation
import LumiDesignSystem

public enum LibraryCondition: String, CaseIterable, Equatable, Sendable {
    case empty
    case importing
    case ready
    case stale
    case degraded
    case error

    public var componentState: LumiComponentState {
        switch self {
        case .empty: .empty
        case .importing: .loading
        case .ready: .ready
        case .stale: .stale
        case .degraded: .degraded
        case .error: .error
        }
    }
}

public enum LibraryReadiness: String, CaseIterable, Equatable, Sendable {
    case ready
    case missingAnalysis
    case staleSource
    case conflict
}

public enum LibraryReadinessFilter: String, CaseIterable, Identifiable, Sendable {
    case all
    case ready
    case missingAnalysis
    case staleSource
    case conflict

    public var id: String { rawValue }
}

public enum TrackPreparationStatus: String, CaseIterable, Identifiable, Equatable, Sendable {
    case notStarted = "not-started"
    case inProgress = "in-progress"
    case readyForShow = "ready-for-show"

    public var id: String { rawValue }
}

public enum TrackWorkflowFilter: String, CaseIterable, Identifiable, Equatable, Sendable {
    case changedAfterUSBSync
    case versionCandidates
    case notStarted
    case inProgress
    case readyForShow

    public var id: String { rawValue }
}

public enum TrackAttentionReason: String, CaseIterable, Equatable, Sendable {
    case metadataChanged
    case waveformChanged
    case beatGridChanged
    case hotCuesChanged
    case sourcePhrasesChanged
}

public struct TrackWorkflowAttention: Equatable, Sendable {
    public let revision: UInt64
    public let sourceID: String
    public let sourceRevision: String
    public let detectedAt: String
    public let reasons: [TrackAttentionReason]
}

public struct TrackWorkflowState: Equatable, Sendable {
    public let preparationStatus: TrackPreparationStatus
    public let stepID: String
    public let statusRevision: UInt64
    public let effectiveReady: Bool
    public let attention: TrackWorkflowAttention?

    public static let notStarted = Self(
        preparationStatus: .notStarted,
        stepID: "not-started",
        statusRevision: 0,
        effectiveReady: false,
        attention: nil
    )
}

public struct TrackPhraseProtectionState: Equatable, Sendable {
    public let locked: Bool
    public let revision: UInt64

    public static let unlocked = Self(locked: false, revision: 0)
}

public struct TrackWorkflowSummary: Equatable, Sendable {
    public let changedAfterUSBSync: UInt64
    public let versionCandidates: UInt64
    public let notStarted: UInt64
    public let inProgress: UInt64
    public let readyForShow: UInt64
    public let catalogRevision: UInt64
    public let stepCounts: [String: UInt64]

    public static let empty = Self(
        changedAfterUSBSync: 0,
        versionCandidates: 0,
        notStarted: 0,
        inProgress: 0,
        readyForShow: 0,
        catalogRevision: 0,
        stepCounts: [:]
    )
}

public enum WorkflowRuleField: String, CaseIterable, Identifiable, Equatable, Sendable {
    case preparationStatus, technicalReady, unresolvedUsbChange, authoredTimeline
    case audioAvailable, versionCandidate
    public var id: String { rawValue }
}

public enum WorkflowRuleOperator: String, CaseIterable, Identifiable, Equatable, Sendable {
    case isEqual = "is"
    case isNot
    public var id: String { rawValue }
}

public struct WorkflowRule: Identifiable, Equatable, Sendable {
    public var id: String { "\(field.rawValue):\(self.operator.rawValue):\(value)" }
    public let field: WorkflowRuleField
    public let `operator`: WorkflowRuleOperator
    public let value: String
}

public struct WorkflowStepDefinition: Identifiable, Equatable, Sendable {
    public let id: String
    public let displayName: String
    public let icon: String
    public let colorRGB: UInt32
    public let sortOrder: UInt16
    public let archived: Bool
    public let rules: [WorkflowRule]
}

public struct TrackWorkflowCatalog: Equatable, Sendable {
    public let revision: UInt64
    public let steps: [WorkflowStepDefinition]
    public static let defaults = Self(revision: 0, steps: [])
}

public struct LibrarySource: Equatable, Sendable {
    public let id: String
    public let name: String
    public let revision: String
    public let status: String

    public init(id: String, name: String, revision: String, status: String) {
        self.id = id
        self.name = name
        self.revision = revision
        self.status = status
    }
}

public struct LibraryCapabilities: Equatable, Sendable {
    public let playlists: Bool
    public let color: Bool
    public let beatGrid: Bool
    public let waveform: Bool
    public let rawPhrases: Bool
    public let localAudio: Bool

    public init(
        playlists: Bool,
        color: Bool,
        beatGrid: Bool,
        waveform: Bool,
        rawPhrases: Bool,
        localAudio: Bool
    ) {
        self.playlists = playlists
        self.color = color
        self.beatGrid = beatGrid
        self.waveform = waveform
        self.rawPhrases = rawPhrases
        self.localAudio = localAudio
    }

    public var missingNames: [String] {
        [
            playlists ? nil : "Playlists",
            color ? nil : "Color",
            beatGrid ? nil : "Beatgrid",
            waveform ? nil : "Waveform",
            rawPhrases ? nil : "Source phrases",
            localAudio ? nil : "Local audio"
        ].compactMap(\.self)
    }
}

public struct LibraryPlaylist: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let sourcePlaylistID: String
    public let name: String
    public let trackCount: UInt64

    public init(id: UInt64, sourcePlaylistID: String, name: String, trackCount: UInt64) {
        self.id = id
        self.sourcePlaylistID = sourcePlaylistID
        self.name = name
        self.trackCount = trackCount
    }
}

public struct LibraryTrack: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let sourceTrackID: String
    public let title: String
    public let artist: String
    public let bpmMilli: UInt64
    public let musicalKey: MusicalKey
    public let durationMillis: UInt64
    public let colorRGB: UInt32?
    public let analysisRevision: String
    public let timelineRevision: UInt64?
    public let readiness: LibraryReadiness
    public let missingCapabilities: [String]
    public let warnings: [String]
    public let usbSources: [LibraryTrackUSBSource]
    public let workflow: TrackWorkflowState
    public let phraseProtection: TrackPhraseProtectionState

    public init(
        id: UInt64,
        sourceTrackID: String,
        title: String,
        artist: String,
        bpmMilli: UInt64,
        musicalKey: MusicalKey,
        durationMillis: UInt64,
        colorRGB: UInt32?,
        analysisRevision: String,
        timelineRevision: UInt64?,
        readiness: LibraryReadiness,
        missingCapabilities: [String],
        warnings: [String],
        usbSources: [LibraryTrackUSBSource] = [],
        workflow: TrackWorkflowState = .notStarted,
        phraseProtection: TrackPhraseProtectionState = .unlocked
    ) {
        self.id = id
        self.sourceTrackID = sourceTrackID
        self.title = title
        self.artist = artist
        self.bpmMilli = bpmMilli
        self.musicalKey = musicalKey
        self.durationMillis = durationMillis
        self.colorRGB = colorRGB
        self.analysisRevision = analysisRevision
        self.timelineRevision = timelineRevision
        self.readiness = readiness
        self.missingCapabilities = missingCapabilities
        self.warnings = warnings
        self.usbSources = usbSources
        self.workflow = workflow
        self.phraseProtection = phraseProtection
    }
}

public extension LibraryTrack {
    var sortKey: UInt16 {
        let mode: Int = musicalKey.mode == .major ? 0 : 1
        return UInt16(musicalKey.pitchClass.rawValue * 2 + mode)
    }
    var sortUSBSources: String { usbSources.map(\.displayName).joined(separator: "\u{001F}") }
    var sortTimelineRevision: UInt64 { timelineRevision ?? 0 }
    var timelineRevisionLabel: String {
        guard let timelineRevision else { return "—" }
        return "R\(timelineRevision)"
    }
    var sortReadiness: String { readiness.rawValue }
    var sortPreparationStatus: String { workflow.preparationStatus.rawValue }
    var sortAttention: String { workflow.attention?.reasons.first?.rawValue ?? "" }
}

public struct LibraryTrackUSBSource: Identifiable, Equatable, Sendable {
    public var id: String { sourceID }
    public let sourceID: String
    public let displayName: String
    public let syncDisposition: String

    public init(sourceID: String, displayName: String, syncDisposition: String) {
        self.sourceID = sourceID
        self.displayName = displayName
        self.syncDisposition = syncDisposition
    }
}

public struct LibraryQuery: Equatable, Sendable {
    public let search: String
    public let playlistID: UInt64?
    public let offset: UInt32
    public let limit: UInt16
    public let sortBy: LibraryTrackSortField
    public let sortDirection: LibraryTrackSortDirection
    public let workflowFilter: TrackWorkflowFilter?
    public let workflowStepID: String?

    public init(
        search: String,
        playlistID: UInt64?,
        offset: UInt32,
        limit: UInt16,
        sortBy: LibraryTrackSortField = .playlist,
        sortDirection: LibraryTrackSortDirection = .ascending,
        workflowFilter: TrackWorkflowFilter? = nil,
        workflowStepID: String? = nil
    ) {
        self.search = search
        self.playlistID = playlistID
        self.offset = offset
        self.limit = limit
        self.sortBy = sortBy
        self.sortDirection = sortDirection
        self.workflowFilter = workflowFilter
        self.workflowStepID = workflowStepID
    }
}

public enum LibraryTrackSortField: String, CaseIterable, Equatable, Sendable {
    case playlist
    case title
    case artist
    case bpm
    case key
    case duration
    case usbSources
    case timelineRevision
    case readiness
    case preparationStatus
    case attention
    case sourceTrackID
    case analysisRevision
}

public enum LibraryTrackSortDirection: String, Equatable, Sendable {
    case ascending
    case descending
}

public struct LibraryPage: Equatable, Sendable {
    public let total: UInt64
    public let offset: UInt32
    public let tracks: [LibraryTrack]

    public init(total: UInt64, offset: UInt32, tracks: [LibraryTrack]) {
        self.total = total
        self.offset = offset
        self.tracks = tracks
    }
}

public struct MidiIntegrationState: Equatable, Sendable {
    public let state: String
    public let sourceName: String
    public let midiProtocol: String
    public let sentPulseCount: UInt64
    public let lastEvent: String?
    public let realtimeLane: RealtimeMidiLaneState?

    public init(
        state: String,
        sourceName: String,
        midiProtocol: String,
        sentPulseCount: UInt64,
        lastEvent: String?,
        realtimeLane: RealtimeMidiLaneState? = nil
    ) {
        self.state = state
        self.sourceName = sourceName
        self.midiProtocol = midiProtocol
        self.sentPulseCount = sentPulseCount
        self.lastEvent = lastEvent
        self.realtimeLane = realtimeLane
    }

    public var isReady: Bool { state == "ready" }
}

public struct RealtimeMidiLaneState: Equatable, Sendable {
    public let queueCapacity: UInt64
    public let queueDepth: UInt64
    public let queueHighWater: UInt64
    public let scheduledCount: UInt64
    public let emittedCount: UInt64
    public let cancelledCount: UInt64
    public let saturationCount: UInt64
    public let latencySampleCount: UInt64
    public let latencyP50Micros: UInt64
    public let latencyP95Micros: UInt64
    public let latencyP99Micros: UInt64
    public let latencyMaxMicros: UInt64
    public let lastDispatchLatenessMicros: UInt64
    public let lateDispatchCount: UInt64

    public var isHealthy: Bool {
        saturationCount == 0 && (latencySampleCount == 0 || latencyP95Micros <= 20_000)
    }
}

public struct MidiClockIntegrationState: Equatable, Sendable {
    public let state: String
    public let sourceName: String
    public let midiProtocol: String
    public let bpmMilli: UInt64?
    public let sentTickCount: UInt64
    public let sentTransportCount: UInt64
    public let lastEvent: String?
    public let lastError: String?

    public var isPublished: Bool { state != "stopped" }
    public var isRunning: Bool { state == "running" }
    public var bpmDescription: String {
        guard let bpmMilli else { return "Waiting for Local Playback" }
        return String(format: "%.3f BPM", Double(bpmMilli) / 1_000)
    }
}

public struct AbletonLinkIntegrationState: Equatable, Sendable {
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
    public let receivedAnchorCount: UInt64
    public let appliedAnchorCount: UInt64
    public let coalescedAnchorCount: UInt64
    public let hardReanchorCount: UInt64
    public let softCorrectionCount: UInt64
    public let failClosedCount: UInt64
    public let failureCount: UInt64
    public let maxAbsPhaseErrorMicros: UInt64
    public let enginePumpCount: UInt64
    public let enginePumpStarvationCount: UInt64
    public let enginePumpMaxLatenessMicros: UInt64
    public let lastReanchor: String?
    public let lastEvent: String?
    public let lastError: String?

    public var isAvailable: Bool { enabled && ["ready", "running"].contains(state) }
    public var bpmDescription: String {
        guard let bpmMilli else { return "Waiting for timing authority" }
        return String(format: "%.3f BPM", Double(bpmMilli) / 1_000)
    }
    public var sourceDescription: String {
        switch source {
        case "localPlayback": "Local Playback"
        case "proDjLink": "Pro DJ Link"
        default: "Waiting for source"
        }
    }
}

public struct DataManagementState: Equatable, Sendable {
    public let trackCount: UInt64
    public let playlistCount: UInt64
    public let userEditedTrackCount: UInt64
    public let creativeArchiveCount: UInt64
    public let pendingArchiveCount: UInt64
    public let resetCandidates: [ResetCandidateTrack]
    public let creativeArchives: [CreativeTrackArchive]
    public let resetPreview: LibraryResetPreview?

    public static let empty = Self(
        trackCount: 0,
        playlistCount: 0,
        userEditedTrackCount: 0,
        creativeArchiveCount: 0,
        pendingArchiveCount: 0,
        resetCandidates: [],
        creativeArchives: [],
        resetPreview: nil
    )
}

public struct ResetCandidateTrack: Identifiable, Equatable, Sendable {
    public let trackID: UInt64
    public let title: String
    public let artist: String
    public let timelineRevision: UInt64
    public var id: UInt64 { trackID }
}

public struct CreativeTrackArchive: Identifiable, Equatable, Sendable {
    public let archiveID: UInt64
    public let title: String
    public let artist: String
    public let phraseCount: UInt64
    public let totalBeats: UInt64
    public let state: String
    public let restoredTrackID: UInt64?
    public var id: UInt64 { archiveID }
}

public struct LibraryResetPreview: Equatable, Sendable {
    public let token: String
    public let trackCount: UInt64
    public let playlistCount: UInt64
    public let preservedTrackCount: UInt64
    public let removedTrackCount: UInt64
    public let archivedCreativeTrackCount: UInt64
    public let preserveTrackIDs: [UInt64]
}

public struct LibraryBackupRecord: Identifiable, Equatable, Sendable {
    public let path: String
    public let name: String
    public let createdAt: Date
    public let sizeBytes: UInt64
    public var id: String { path }

    public init(path: String, name: String, createdAt: Date, sizeBytes: UInt64) {
        self.path = path
        self.name = name
        self.createdAt = createdAt
        self.sizeBytes = sizeBytes
    }
}

public struct DataManagementOperationState: Equatable, Sendable {
    public enum Phase: String, Equatable, Sendable {
        case idle
        case backingUp
        case preparingReset
        case resetting
        case restoring
        case completed
        case failed
    }

    public let phase: Phase
    public let title: String
    public let detail: String

    public init(phase: Phase, title: String, detail: String) {
        self.phase = phase
        self.title = title
        self.detail = detail
    }

    public static let idle = Self(phase: .idle, title: "", detail: "")
    public var isBusy: Bool {
        [.backingUp, .preparingReset, .resetting, .restoring].contains(phase)
    }
}

public struct LibraryWorkspaceState: Equatable, Sendable {
    public let condition: LibraryCondition
    public let providerKind: String
    public let source: LibrarySource?
    public let capabilities: LibraryCapabilities?
    public let collectionTotal: UInt64
    public let playlists: [LibraryPlaylist]
    public let query: LibraryQuery
    public let page: LibraryPage
    public let workflow: TrackWorkflowSummary
    public let workflowCatalog: TrackWorkflowCatalog
    public let editor: TrackEditorAnalysis?
    public let phraseRoleSettings: PhraseRoleSettingsState?
    public let autoloopCatalog: AutoloopCatalogState?
    public let midiIntegration: MidiIntegrationState?
    public let midiClockIntegration: MidiClockIntegrationState?
    public let abletonLinkIntegration: AbletonLinkIntegrationState?
    public let deckInputIntegration: DeckInputIntegrationState?
    public let rekordboxSyncPreview: RekordboxXMLSyncPreview?
    public let rekordboxMirror: RekordboxMirrorState?
    public let rekordboxDevices: [RekordboxDeviceState]
    public let rekordboxDeviceInspection: RekordboxDeviceInspectionState?
    public let dataManagement: DataManagementState
    public let diagnostic: String?

    public init(
        condition: LibraryCondition,
        providerKind: String,
        source: LibrarySource?,
        capabilities: LibraryCapabilities?,
        collectionTotal: UInt64,
        playlists: [LibraryPlaylist],
        query: LibraryQuery,
        page: LibraryPage,
        workflow: TrackWorkflowSummary = .empty,
        workflowCatalog: TrackWorkflowCatalog = .defaults,
        editor: TrackEditorAnalysis? = nil,
        phraseRoleSettings: PhraseRoleSettingsState? = nil,
        autoloopCatalog: AutoloopCatalogState? = nil,
        midiIntegration: MidiIntegrationState? = nil,
        midiClockIntegration: MidiClockIntegrationState? = nil,
        abletonLinkIntegration: AbletonLinkIntegrationState? = nil,
        deckInputIntegration: DeckInputIntegrationState? = nil,
        rekordboxSyncPreview: RekordboxXMLSyncPreview? = nil,
        rekordboxMirror: RekordboxMirrorState? = nil,
        rekordboxDevices: [RekordboxDeviceState] = [],
        rekordboxDeviceInspection: RekordboxDeviceInspectionState? = nil,
        dataManagement: DataManagementState = .empty,
        diagnostic: String? = nil
    ) {
        self.condition = condition
        self.providerKind = providerKind
        self.source = source
        self.capabilities = capabilities
        self.collectionTotal = collectionTotal
        self.playlists = playlists
        self.query = query
        self.page = page
        self.workflow = workflow
        self.workflowCatalog = workflowCatalog
        self.editor = editor
        self.phraseRoleSettings = phraseRoleSettings
        self.autoloopCatalog = autoloopCatalog
        self.midiIntegration = midiIntegration
        self.midiClockIntegration = midiClockIntegration
        self.abletonLinkIntegration = abletonLinkIntegration
        self.deckInputIntegration = deckInputIntegration
        self.rekordboxSyncPreview = rekordboxSyncPreview
        self.rekordboxMirror = rekordboxMirror
        self.rekordboxDevices = rekordboxDevices
        self.rekordboxDeviceInspection = rekordboxDeviceInspection
        self.dataManagement = dataManagement
        self.diagnostic = diagnostic
    }

    public static func importing() -> Self {
        placeholder(.importing, diagnostic: "Importing the local music library…")
    }

    public func preservingDeviceInspection(
        _ inspection: RekordboxDeviceInspectionState?
    ) -> Self {
        guard rekordboxDeviceInspection == nil, let inspection else { return self }
        return Self(
            condition: condition,
            providerKind: providerKind,
            source: source,
            capabilities: capabilities,
            collectionTotal: collectionTotal,
            playlists: playlists,
            query: query,
            page: page,
            workflow: workflow,
            workflowCatalog: workflowCatalog,
            editor: editor,
            phraseRoleSettings: phraseRoleSettings,
            autoloopCatalog: autoloopCatalog,
            midiIntegration: midiIntegration,
            midiClockIntegration: midiClockIntegration,
            abletonLinkIntegration: abletonLinkIntegration,
            deckInputIntegration: deckInputIntegration,
            rekordboxSyncPreview: rekordboxSyncPreview,
            rekordboxMirror: rekordboxMirror,
            rekordboxDevices: rekordboxDevices,
            rekordboxDeviceInspection: inspection,
            dataManagement: dataManagement,
            diagnostic: diagnostic
        )
    }

    public static func failed(_ message: String) -> Self {
        placeholder(.error, diagnostic: message)
    }

    public static func placeholder(
        _ condition: LibraryCondition,
        diagnostic: String? = nil
    ) -> Self {
        Self(
            condition: condition,
            providerKind: "unavailable",
            source: nil,
            capabilities: nil,
            collectionTotal: 0,
            playlists: [],
            query: LibraryQuery(search: "", playlistID: nil, offset: 0, limit: 50),
            page: LibraryPage(total: 0, offset: 0, tracks: []),
            diagnostic: diagnostic
        )
    }
}

public struct RekordboxDevicePlaylistState: Equatable, Sendable, Identifiable {
    public let id: UInt32
    public let path: String
    public let folderNames: [String]
    public let name: String
    public let trackCount: UInt64
    public let statusCounts: RekordboxDeviceStatusCounts
    public let tracks: [RekordboxDeviceTrackState]
}

public struct RekordboxDeviceStatusCounts: Equatable, Sendable {
    public let current: UInt64
    public let usbNewer: UInt64
    public let usbOutdated: UInt64
    public let notInLumi: UInt64
    public let conflict: UInt64
}

public struct RekordboxDeviceTrackState: Equatable, Sendable, Identifiable {
    public let id: UInt32
    public let title: String
    public let artist: String
    public let bpmMilli: UInt64
    public let durationMillis: UInt64
    public let status: String
    public let detail: String
}

public struct RekordboxDeviceInspectionState: Equatable, Sendable {
    public let sourceID: String
    public let displayName: String
    public let databaseRevision: String
    public let libraryFormat: String
    public let databaseVersion: String
    public let exportedAt: String
    public let trackCount: UInt64
    public let playlistCount: UInt64
    public let selectedPlaylistIDs: [UInt32]
    public let playlists: [RekordboxDevicePlaylistState]
}

public struct RekordboxDeviceState: Equatable, Sendable, Identifiable {
    public let sourceID: String
    public let displayName: String
    public let databaseRevision: String
    public let activeTracks: UInt64
    public let matchedTracks: UInt64
    public let unmatchedTracks: UInt64
    public let syncedAt: String
    public let trustState: String
    public let currentTracks: UInt64
    public let promotedTracks: UInt64
    public let protectedTracks: UInt64
    public let conflictTracks: UInt64
    public let beatGridRefresh: Bool
    public let cueRevisionTracked: Bool
    public let reviewTracks: [RekordboxDeviceReviewTrackState]
    public let playlists: [RekordboxDeviceSyncedPlaylistState]

    public var id: String { sourceID }
}

public struct RekordboxDeviceReviewTrackState: Equatable, Sendable, Identifiable {
    public let deviceTrackID: UInt32
    public let canonicalTrackID: UInt64?
    public let title: String
    public let artist: String
    public let bpmMilli: UInt64
    public let durationMillis: UInt64
    public let incomingAnalyzedAt: String
    public let activeAnalyzedAt: String?
    public let activeSourceName: String?
    public let incomingAnalysisRevision: String
    public let activeAnalysisRevision: String?
    public let incomingMetadataRevision: String
    public let incomingFileSize: UInt64
    public let reason: String
    public let components: RekordboxDeviceReviewComponentsState?

    public var id: UInt32 { deviceTrackID }
}

public struct RekordboxDeviceReviewComponentState: Equatable, Sendable {
    public let status: String
    public let detail: String

    public var changed: Bool { status == "changed" }
}

public struct RekordboxDeviceReviewComponentsState: Equatable, Sendable {
    public let beatGrid: RekordboxDeviceReviewComponentState
    public let cuePoints: RekordboxDeviceReviewComponentState
    public let fileData: RekordboxDeviceReviewComponentState
    public let rekordboxPhrases: RekordboxDeviceReviewComponentState
    public let waveform: RekordboxDeviceReviewComponentState
}

public enum USBConflictResolutionChoice: String, Sendable {
    case keepLumi = "keep-lumi"
    case useUSB = "use-usb"
}

public struct USBConflictResolutionRequest: Sendable {
    public let root: String
    public let sourceID: String
    public let deviceTrackID: UInt32
    public let expectedIncomingRevision: String
    public let expectedActiveRevision: String
    public let choice: USBConflictResolutionChoice

    public init(
        root: String,
        sourceID: String,
        deviceTrackID: UInt32,
        expectedIncomingRevision: String,
        expectedActiveRevision: String,
        choice: USBConflictResolutionChoice
    ) {
        self.root = root
        self.sourceID = sourceID
        self.deviceTrackID = deviceTrackID
        self.expectedIncomingRevision = expectedIncomingRevision
        self.expectedActiveRevision = expectedActiveRevision
        self.choice = choice
    }
}

public struct RekordboxDeviceSyncedPlaylistState: Equatable, Sendable, Identifiable {
    public let id: UInt32
    public let libraryPlaylistID: UInt64
    public let name: String
    public let trackCount: UInt64
}

public struct USBSourceOperationState: Equatable, Sendable {
    public enum Phase: String, Equatable, Sendable {
        case idle
        case reading
        case synchronizing
        case completed
        case failed
    }

    public let phase: Phase
    public let title: String
    public let detail: String

    public init(phase: Phase, title: String, detail: String) {
        self.phase = phase
        self.title = title
        self.detail = detail
    }

    public static let idle = Self(phase: .idle, title: "", detail: "")
    public var isActive: Bool { phase == .reading || phase == .synchronizing }
}

public struct ProDJLinkDeviceState: Equatable, Sendable, Identifiable {
    public let playerNumber: UInt64
    public let name: String
    public let address: String?

    public var id: UInt64 { playerNumber }
}

public struct DeckInputIntegrationState: Equatable, Sendable {
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
    public let sourceState: String?
    public let bridgeVersion: String?
    public let beatLinkVersion: String?
    public let discoveredPlayers: [ProDJLinkDeviceState]
    public let recoveryPending: Bool
    public let restartCount: UInt64
    public let ingressQueueCapacity: UInt64
    public let ingressQueueDepth: UInt64
    public let ingressQueueHighWater: UInt64
    public let ingressCoalescedMessageCount: UInt64
    public let ingressCriticalSaturationCount: UInt64
    public let ingressSourceAgeSampleCount: UInt64
    public let ingressSourceAgeP50Micros: UInt64
    public let ingressSourceAgeP95Micros: UInt64
    public let ingressSourceAgeP99Micros: UInt64
    public let ingressSourceAgeMaxMicros: UInt64
    public let precisePositionMessageCount: UInt64
    public let authoritativePositionCount: UInt64
    public let positionDiscontinuityCount: UInt64
    public let positionAuthorityReady: Bool
    public let lastError: String?

    public var isReady: Bool { state == "ready" }
    public var isReceiving: Bool { committedFrameCount > 0 }
    public var isProDJLink: Bool { protocolName == "lumi-prolink-bridge" }
}

public enum LibraryWorkspacePresenter {
    public static func visibleTracks(
        in state: LibraryWorkspaceState,
        filter: LibraryReadinessFilter
    ) -> [LibraryTrack] {
        guard filter != .all else { return state.page.tracks }
        return state.page.tracks.filter { $0.readiness.rawValue == filter.rawValue }
    }

    public static func pageNumber(in state: LibraryWorkspaceState) -> UInt64 {
        UInt64(state.query.offset) / UInt64(state.query.limit) + 1
    }

    public static func pageCount(in state: LibraryWorkspaceState) -> UInt64 {
        max(1, (state.page.total + UInt64(state.query.limit) - 1) / UInt64(state.query.limit))
    }
}
