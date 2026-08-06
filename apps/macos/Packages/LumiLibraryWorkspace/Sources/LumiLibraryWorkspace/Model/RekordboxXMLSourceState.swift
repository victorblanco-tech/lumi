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

private extension RekordboxPlaylistNode {
    var playlistCount: Int {
        (kind == .playlist ? 1 : 0) + children.reduce(0) { $0 + $1.playlistCount }
    }

    var folderCount: Int {
        (kind == .folder ? 1 : 0) + children.reduce(0) { $0 + $1.folderCount }
    }
}
