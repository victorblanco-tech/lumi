import Foundation

public struct RekordboxXMLDiscoveryService: Sendable {
    public static let maximumFileSize: UInt64 = 256 * 1_024 * 1_024
    public static let maximumNodeCount = 20_000
    public static let maximumDepth = 32
    public static let maximumNameLength = 512
    public static let maximumTrackReferences: UInt64 = 1_000_000

    public init() {}

    public func exports(in folderURL: URL) throws -> [RekordboxXMLExport] {
        let values: Set<URLResourceKey> = [
            .contentModificationDateKey,
            .fileSizeKey,
            .isRegularFileKey,
            .isHiddenKey
        ]
        return try FileManager.default
            .contentsOfDirectory(
                at: folderURL,
                includingPropertiesForKeys: Array(values),
                options: [.skipsHiddenFiles, .skipsPackageDescendants]
            )
            .compactMap { url -> RekordboxXMLExport? in
                guard url.pathExtension.lowercased() == "xml" else { return nil }
                let resource = try url.resourceValues(forKeys: values)
                guard resource.isRegularFile == true, resource.isHidden != true else { return nil }
                let rawSize = resource.fileSize ?? 0
                guard rawSize > 0 else { return nil }
                let size = UInt64(rawSize)
                return RekordboxXMLExport(
                    path: url.path,
                    fileName: url.lastPathComponent,
                    modifiedAt: resource.contentModificationDate ?? .distantPast,
                    sizeBytes: size
                )
            }
            .sorted {
                if $0.modifiedAt != $1.modifiedAt { return $0.modifiedAt > $1.modifiedAt }
                return $0.fileName.localizedStandardCompare($1.fileName) == .orderedAscending
            }
    }

    public func scan(_ export: RekordboxXMLExport) throws -> RekordboxXMLDiscoveryState {
        guard export.sizeBytes <= Self.maximumFileSize else {
            throw RekordboxXMLDiscoveryError.fileTooLarge(export.sizeBytes)
        }
        let url = URL(fileURLWithPath: export.path)
        guard let parser = XMLParser(contentsOf: url) else {
            throw RekordboxXMLDiscoveryError.unreadable
        }
        let delegate = RekordboxXMLParserDelegate()
        parser.delegate = delegate
        parser.shouldProcessNamespaces = false
        parser.shouldReportNamespacePrefixes = false
        parser.shouldResolveExternalEntities = false
        guard parser.parse() else {
            throw delegate.failure
                ?? parser.parserError.map {
                    RekordboxXMLDiscoveryError.malformed($0.localizedDescription)
                }
                ?? .malformed("Unknown XML parser failure")
        }
        return try delegate.result(export: export)
    }
}

public enum RekordboxXMLDiscoveryError: Error, Equatable, LocalizedError, Sendable {
    case unreadable
    case fileTooLarge(UInt64)
    case unsupportedRoot
    case unsupportedVersion(String)
    case missingProduct
    case missingPlaylists
    case invalidNode
    case tooManyNodes
    case tooDeep
    case nameTooLong
    case tooManyTrackReferences
    case malformed(String)

    public var errorDescription: String? {
        switch self {
        case .unreadable: "The XML export could not be opened read-only."
        case let .fileTooLarge(bytes): "The XML export is too large (\(bytes) bytes; 256 MB maximum)."
        case .unsupportedRoot: "This is not a supported rekordbox DJ_PLAYLISTS export."
        case let .unsupportedVersion(version): "Rekordbox XML version \(version) is not supported."
        case .missingProduct: "The XML export has no Rekordbox product identity."
        case .missingPlaylists: "The XML export contains no playlist tree."
        case .invalidNode: "The XML export contains an invalid playlist node."
        case .tooManyNodes: "The XML export contains too many playlist nodes."
        case .tooDeep: "The XML playlist tree is nested too deeply."
        case .nameTooLong: "A playlist or folder name exceeds the safe length limit."
        case .tooManyTrackReferences: "The selected XML contains too many playlist track references."
        case let .malformed(detail): "The XML export is malformed: \(detail)"
        }
    }
}

private final class RekordboxXMLParserDelegate: NSObject, XMLParserDelegate {
    private struct NodeBuilder {
        let name: String
        let kind: RekordboxPlaylistNodeKind
        let declaredEntries: UInt64
        var observedEntries: UInt64 = 0
        var children: [RekordboxPlaylistNode] = []
    }

    private var sawRoot = false
    private var sawPlaylists = false
    private var insidePlaylists = false
    private var xmlVersion = ""
    private var productName = ""
    private var productVersion = ""
    private var collectionEntries: UInt64 = 0
    private var nodeCount = 0
    private var trackReferenceCount: UInt64 = 0
    private var stack: [NodeBuilder] = []
    private var roots: [RekordboxPlaylistNode] = []
    private(set) var failure: RekordboxXMLDiscoveryError?

    func parser(
        _ parser: XMLParser,
        didStartElement elementName: String,
        namespaceURI: String?,
        qualifiedName qName: String?,
        attributes attributeDict: [String: String] = [:]
    ) {
        guard failure == nil else { parser.abortParsing(); return }
        switch elementName {
        case "DJ_PLAYLISTS":
            sawRoot = true
            xmlVersion = attributeDict["Version"] ?? ""
        case "PRODUCT":
            productName = attributeDict["Name"] ?? ""
            productVersion = attributeDict["Version"] ?? ""
        case "COLLECTION":
            collectionEntries = unsigned(attributeDict["Entries"]) ?? 0
        case "PLAYLISTS":
            sawPlaylists = true
            insidePlaylists = true
        case "NODE" where insidePlaylists:
            startNode(attributeDict, parser: parser)
        case "TRACK" where insidePlaylists && !stack.isEmpty:
            guard stack.last?.kind == .playlist else {
                fail(.invalidNode, parser: parser)
                return
            }
            trackReferenceCount += 1
            stack[stack.count - 1].observedEntries += 1
            if trackReferenceCount > RekordboxXMLDiscoveryService.maximumTrackReferences {
                fail(.tooManyTrackReferences, parser: parser)
            }
        default:
            break
        }
    }

    func parser(
        _ parser: XMLParser,
        didEndElement elementName: String,
        namespaceURI: String?,
        qualifiedName qName: String?
    ) {
        guard failure == nil else { return }
        if elementName == "NODE", insidePlaylists {
            finishNode(parser: parser)
        } else if elementName == "PLAYLISTS" {
            insidePlaylists = false
        }
    }

    func parser(_ parser: XMLParser, parseErrorOccurred parseError: Error) {
        if failure == nil {
            failure = .malformed(parseError.localizedDescription)
        }
    }

    func result(export: RekordboxXMLExport) throws -> RekordboxXMLDiscoveryState {
        guard sawRoot else { throw RekordboxXMLDiscoveryError.unsupportedRoot }
        guard ["1.0.0", "1,0,0"].contains(xmlVersion) else {
            throw RekordboxXMLDiscoveryError.unsupportedVersion(xmlVersion)
        }
        guard productName.lowercased().contains("rekordbox") else {
            throw RekordboxXMLDiscoveryError.missingProduct
        }
        guard sawPlaylists, !roots.isEmpty else {
            throw RekordboxXMLDiscoveryError.missingPlaylists
        }
        return RekordboxXMLDiscoveryState(
            export: export,
            xmlVersion: xmlVersion,
            productName: productName,
            productVersion: productVersion,
            collectionEntries: collectionEntries,
            roots: normalizedRoots()
        )
    }

    private func normalizedRoots() -> [RekordboxPlaylistNode] {
        let visibleRoots: [RekordboxPlaylistNode]
        if roots.count == 1,
           roots[0].kind == .folder,
           roots[0].name.caseInsensitiveCompare("ROOT") == .orderedSame {
            visibleRoots = roots[0].children
        } else {
            visibleRoots = roots
        }
        return visibleRoots.map { rebased($0, parentPath: "") }
    }

    private func rebased(
        _ node: RekordboxPlaylistNode,
        parentPath: String
    ) -> RekordboxPlaylistNode {
        let path = parentPath.isEmpty ? node.name : "\(parentPath)/\(node.name)"
        return RekordboxPlaylistNode(
            id: path,
            name: node.name,
            path: path,
            kind: node.kind,
            trackCount: node.trackCount,
            children: node.children.map { rebased($0, parentPath: path) }
        )
    }

    private func startNode(_ attributes: [String: String], parser: XMLParser) {
        guard stack.last?.kind != .playlist else {
            fail(.invalidNode, parser: parser); return
        }
        nodeCount += 1
        guard nodeCount <= RekordboxXMLDiscoveryService.maximumNodeCount else {
            fail(.tooManyNodes, parser: parser); return
        }
        guard stack.count < RekordboxXMLDiscoveryService.maximumDepth else {
            fail(.tooDeep, parser: parser); return
        }
        guard let name = attributes["Name"]?.trimmingCharacters(in: .whitespacesAndNewlines),
              !name.isEmpty,
              let type = attributes["Type"],
              let kind = type == "0" ? RekordboxPlaylistNodeKind.folder : type == "1" ? .playlist : nil
        else {
            fail(.invalidNode, parser: parser); return
        }
        guard name.count <= RekordboxXMLDiscoveryService.maximumNameLength else {
            fail(.nameTooLong, parser: parser); return
        }
        stack.append(
            NodeBuilder(
                name: name,
                kind: kind,
                declaredEntries: unsigned(attributes["Entries"]) ?? 0
            )
        )
    }

    private func finishNode(parser: XMLParser) {
        guard let builder = stack.popLast() else {
            fail(.invalidNode, parser: parser); return
        }
        if builder.kind == .playlist,
           builder.declaredEntries != builder.observedEntries {
            fail(.invalidNode, parser: parser); return
        }
        let parentPath = stack.map(\.name).joined(separator: "/")
        let path = parentPath.isEmpty ? builder.name : "\(parentPath)/\(builder.name)"
        let node = RekordboxPlaylistNode(
            id: path,
            name: builder.name,
            path: path,
            kind: builder.kind,
            trackCount: builder.kind == .playlist ? builder.observedEntries : 0,
            children: builder.children
        )
        if stack.isEmpty {
            roots.append(node)
        } else {
            stack[stack.count - 1].children.append(node)
        }
    }

    private func fail(_ error: RekordboxXMLDiscoveryError, parser: XMLParser) {
        failure = error
        parser.abortParsing()
    }

    private func unsigned(_ value: String?) -> UInt64? {
        value.flatMap(UInt64.init)
    }
}
