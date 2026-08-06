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
        warnings: [String]
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
    }
}

public struct LibraryQuery: Equatable, Sendable {
    public let search: String
    public let playlistID: UInt64?
    public let offset: UInt32
    public let limit: UInt16

    public init(search: String, playlistID: UInt64?, offset: UInt32, limit: UInt16) {
        self.search = search
        self.playlistID = playlistID
        self.offset = offset
        self.limit = limit
    }
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

    public init(
        state: String,
        sourceName: String,
        midiProtocol: String,
        sentPulseCount: UInt64,
        lastEvent: String?
    ) {
        self.state = state
        self.sourceName = sourceName
        self.midiProtocol = midiProtocol
        self.sentPulseCount = sentPulseCount
        self.lastEvent = lastEvent
    }

    public var isReady: Bool { state == "ready" }
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
    public let editor: TrackEditorAnalysis?
    public let phraseRoleSettings: PhraseRoleSettingsState?
    public let autoloopCatalog: AutoloopCatalogState?
    public let midiIntegration: MidiIntegrationState?
    public let deckInputIntegration: DeckInputIntegrationState?
    public let rekordboxSyncPreview: RekordboxXMLSyncPreview?
    public let rekordboxMirror: RekordboxMirrorState?
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
        editor: TrackEditorAnalysis? = nil,
        phraseRoleSettings: PhraseRoleSettingsState? = nil,
        autoloopCatalog: AutoloopCatalogState? = nil,
        midiIntegration: MidiIntegrationState? = nil,
        deckInputIntegration: DeckInputIntegrationState? = nil,
        rekordboxSyncPreview: RekordboxXMLSyncPreview? = nil,
        rekordboxMirror: RekordboxMirrorState? = nil,
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
        self.editor = editor
        self.phraseRoleSettings = phraseRoleSettings
        self.autoloopCatalog = autoloopCatalog
        self.midiIntegration = midiIntegration
        self.deckInputIntegration = deckInputIntegration
        self.rekordboxSyncPreview = rekordboxSyncPreview
        self.rekordboxMirror = rekordboxMirror
        self.diagnostic = diagnostic
    }

    public static func importing() -> Self {
        placeholder(.importing, diagnostic: "Importing the local music library…")
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

    public var isReady: Bool { state == "ready" }
    public var isReceiving: Bool { committedFrameCount > 0 }
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
