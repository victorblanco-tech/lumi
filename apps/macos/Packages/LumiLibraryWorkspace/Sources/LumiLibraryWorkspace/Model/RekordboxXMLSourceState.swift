import Foundation

public enum RekordboxPlaylistNodeKind: String, Equatable, Sendable {
    case folder
    case playlist
}

public struct RekordboxPlaylistNode: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let path: String
    public let kind: RekordboxPlaylistNodeKind
    public let trackCount: UInt64
    public let children: [RekordboxPlaylistNode]

    public init(
        id: String,
        name: String,
        path: String,
        kind: RekordboxPlaylistNodeKind,
        trackCount: UInt64,
        children: [RekordboxPlaylistNode]
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.kind = kind
        self.trackCount = trackCount
        self.children = children
    }

    public var descendantTrackCount: UInt64 {
        kind == .playlist
            ? trackCount
            : children.reduce(0) { $0 + $1.descendantTrackCount }
    }
}

public struct RekordboxXMLExport: Identifiable, Equatable, Sendable {
    public var id: String { path }

    public let path: String
    public let fileName: String
    public let modifiedAt: Date
    public let sizeBytes: UInt64

    public init(path: String, fileName: String, modifiedAt: Date, sizeBytes: UInt64) {
        self.path = path
        self.fileName = fileName
        self.modifiedAt = modifiedAt
        self.sizeBytes = sizeBytes
    }
}

public struct RekordboxXMLDiscoveryState: Equatable, Sendable {
    public let export: RekordboxXMLExport
    public let xmlVersion: String
    public let productName: String
    public let productVersion: String
    public let collectionEntries: UInt64
    public let roots: [RekordboxPlaylistNode]

    public init(
        export: RekordboxXMLExport,
        xmlVersion: String,
        productName: String,
        productVersion: String,
        collectionEntries: UInt64,
        roots: [RekordboxPlaylistNode]
    ) {
        self.export = export
        self.xmlVersion = xmlVersion
        self.productName = productName
        self.productVersion = productVersion
        self.collectionEntries = collectionEntries
        self.roots = roots
    }

    public var playlistCount: Int { roots.reduce(0) { $0 + $1.playlistCount } }
    public var folderCount: Int { roots.reduce(0) { $0 + $1.folderCount } }
}

public struct RekordboxXMLSyncPreviewRequest: Equatable, Sendable {
    public let folderPath: String
    public let followedPaths: [String]
    public let includeFutureChildPlaylists: Bool

    public init(
        folderPath: String,
        followedPaths: [String],
        includeFutureChildPlaylists: Bool
    ) {
        self.folderPath = folderPath
        self.followedPaths = followedPaths
        self.includeFutureChildPlaylists = includeFutureChildPlaylists
    }
}

public struct RekordboxXMLSyncPreview: Equatable, Sendable {
    public let exportFileName: String
    public let contentSHA256: String
    public let productVersion: String
    public let collectionTrackCount: UInt64
    public let followedPlaylistCount: UInt64
    public let uniqueTrackCount: UInt64
    public let selectionPaths: [String]
    public let includeFutureChildPlaylists: Bool
    public let playlists: [RekordboxXMLSyncPlaylist]
    public let diagnostics: RekordboxXMLSyncDiagnostics
    public let applyState: String

    public init(
        exportFileName: String,
        contentSHA256: String,
        productVersion: String,
        collectionTrackCount: UInt64,
        followedPlaylistCount: UInt64,
        uniqueTrackCount: UInt64,
        selectionPaths: [String],
        includeFutureChildPlaylists: Bool,
        playlists: [RekordboxXMLSyncPlaylist],
        diagnostics: RekordboxXMLSyncDiagnostics,
        applyState: String
    ) {
        self.exportFileName = exportFileName
        self.contentSHA256 = contentSHA256
        self.productVersion = productVersion
        self.collectionTrackCount = collectionTrackCount
        self.followedPlaylistCount = followedPlaylistCount
        self.uniqueTrackCount = uniqueTrackCount
        self.selectionPaths = selectionPaths
        self.includeFutureChildPlaylists = includeFutureChildPlaylists
        self.playlists = playlists
        self.diagnostics = diagnostics
        self.applyState = applyState
    }
}

public struct RekordboxXMLSyncPlaylist: Identifiable, Equatable, Sendable {
    public var id: String { path }

    public let path: String
    public let name: String
    public let trackCount: UInt64

    public init(path: String, name: String, trackCount: UInt64) {
        self.path = path
        self.name = name
        self.trackCount = trackCount
    }
}

public struct RekordboxXMLSyncDiagnostics: Equatable, Sendable {
    public let duplicatePlaylistReferences: UInt64
    public let missingArtist: UInt64
    public let missingBPM: UInt64
    public let missingKey: UInt64
    public let missingDuration: UInt64
    public let missingBeatGrid: UInt64
    public let missingColour: UInt64
    public let missingWaveform: UInt64
    public let missingPhrases: UInt64

    public init(
        duplicatePlaylistReferences: UInt64,
        missingArtist: UInt64,
        missingBPM: UInt64,
        missingKey: UInt64,
        missingDuration: UInt64,
        missingBeatGrid: UInt64,
        missingColour: UInt64,
        missingWaveform: UInt64,
        missingPhrases: UInt64
    ) {
        self.duplicatePlaylistReferences = duplicatePlaylistReferences
        self.missingArtist = missingArtist
        self.missingBPM = missingBPM
        self.missingKey = missingKey
        self.missingDuration = missingDuration
        self.missingBeatGrid = missingBeatGrid
        self.missingColour = missingColour
        self.missingWaveform = missingWaveform
        self.missingPhrases = missingPhrases
    }
}

private extension RekordboxPlaylistNode {
    var playlistCount: Int {
        (kind == .playlist ? 1 : 0) + children.reduce(0) { $0 + $1.playlistCount }
    }

    var folderCount: Int {
        (kind == .folder ? 1 : 0) + children.reduce(0) { $0 + $1.folderCount }
    }
}
