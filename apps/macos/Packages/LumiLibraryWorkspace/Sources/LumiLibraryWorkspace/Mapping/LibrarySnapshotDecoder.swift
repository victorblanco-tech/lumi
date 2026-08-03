import LumiDesignSystem
import LumiProtocol

public struct LibrarySnapshotDecoder: Sendable {
    public init() {}

    public func decode(_ envelope: MessageEnvelope) throws -> LibraryWorkspaceState {
        guard envelope.messageType == .snapshot,
              case let .object(library)? = envelope.payload["library"] else {
            throw LibrarySnapshotError.missingLibrary
        }
        let condition = try LibraryCondition(rawValue: string(library, "condition"))
            .required(.invalidCondition)
        let sourceObject = try object(library, "source")
        let capabilitiesObject = try object(library, "capabilities")
        let queryObject = try object(library, "query")
        let pageObject = try object(library, "page")
        let playlistValues = try array(library, "playlists")
        let trackValues = try array(pageObject, "tracks")
        guard playlistValues.count <= 200, trackValues.count <= 200 else {
            throw LibrarySnapshotError.unboundedPage
        }
        let query = LibraryQuery(
            search: try string(queryObject, "search"),
            playlistID: optionalUnsigned(queryObject, "playlistId"),
            offset: try UInt32(exactly: unsigned(queryObject, "offset"))
                .required(.invalidNumber("offset")),
            limit: try UInt16(exactly: unsigned(queryObject, "limit"))
                .required(.invalidNumber("limit"))
        )
        guard (1...200).contains(query.limit) else {
            throw LibrarySnapshotError.unboundedPage
        }
        return LibraryWorkspaceState(
            condition: condition,
            providerKind: try string(library, "providerKind"),
            source: LibrarySource(
                id: try string(sourceObject, "id"),
                name: try string(sourceObject, "name"),
                revision: try string(sourceObject, "revision"),
                status: try string(sourceObject, "status")
            ),
            capabilities: LibraryCapabilities(
                playlists: try boolean(capabilitiesObject, "playlists"),
                color: try boolean(capabilitiesObject, "color"),
                beatGrid: try boolean(capabilitiesObject, "beatGrid"),
                waveform: try boolean(capabilitiesObject, "waveform"),
                rawPhrases: try boolean(capabilitiesObject, "rawPhrases"),
                localAudio: try boolean(capabilitiesObject, "localAudio")
            ),
            collectionTotal: try unsigned(library, "collectionTotal"),
            playlists: try playlistValues.map(decodePlaylist),
            query: query,
            page: LibraryPage(
                total: try unsigned(pageObject, "total"),
                offset: try UInt32(exactly: unsigned(pageObject, "offset"))
                    .required(.invalidNumber("page.offset")),
                tracks: try trackValues.map(decodeTrack)
            )
        )
    }

    private func decodePlaylist(_ value: JSONValue) throws -> LibraryPlaylist {
        guard case let .object(object) = value else { throw LibrarySnapshotError.invalidObject }
        return LibraryPlaylist(
            id: try unsigned(object, "id"),
            sourcePlaylistID: try string(object, "sourcePlaylistId"),
            name: try string(object, "name"),
            trackCount: try unsigned(object, "trackCount")
        )
    }

    private func decodeTrack(_ value: JSONValue) throws -> LibraryTrack {
        guard case let .object(object) = value else { throw LibrarySnapshotError.invalidObject }
        let key = try self.object(object, "key")
        let readiness = try self.object(object, "readiness")
        return LibraryTrack(
            id: try unsigned(object, "id"),
            sourceTrackID: try string(object, "sourceTrackId"),
            title: try string(object, "title"),
            artist: try string(object, "artist"),
            bpmMilli: try unsigned(object, "bpmMilli"),
            musicalKey: MusicalKey(
                pitchClass: try PitchClass(rawValue: pitchIndex(try string(key, "pitchClass")))
                    .required(.invalidMusicalKey),
                mode: try KeyMode(rawValue: string(key, "mode"))
                    .required(.invalidMusicalKey)
            ),
            durationMillis: try unsigned(object, "durationMillis"),
            colorRGB: optionalUnsigned(object, "colorRgb").flatMap(UInt32.init(exactly:)),
            analysisRevision: try string(object, "analysisRevision"),
            timelineRevision: optionalUnsigned(object, "timelineRevision"),
            readiness: try LibraryReadiness(rawValue: string(readiness, "status"))
                .required(.invalidReadiness),
            missingCapabilities: try strings(readiness, "missingCapabilities"),
            warnings: try strings(readiness, "warnings")
        )
    }

    private func pitchIndex(_ value: String) throws -> Int {
        let values = ["c", "cSharp", "d", "dSharp", "e", "f", "fSharp", "g", "gSharp", "a", "aSharp", "b"]
        guard let index = values.firstIndex(of: value) else {
            throw LibrarySnapshotError.invalidMusicalKey
        }
        return index
    }

    private func object(_ values: [String: JSONValue], _ key: String) throws -> [String: JSONValue] {
        guard case let .object(value)? = values[key] else {
            throw LibrarySnapshotError.missingField(key)
        }
        return value
    }

    private func array(_ values: [String: JSONValue], _ key: String) throws -> [JSONValue] {
        guard case let .array(value)? = values[key] else {
            throw LibrarySnapshotError.missingField(key)
        }
        return value
    }

    private func string(_ values: [String: JSONValue], _ key: String) throws -> String {
        guard case let .string(value)? = values[key] else {
            throw LibrarySnapshotError.missingField(key)
        }
        return value
    }

    private func unsigned(_ values: [String: JSONValue], _ key: String) throws -> UInt64 {
        guard case let .number(value)? = values[key], value >= 0,
              value.rounded(.towardZero) == value,
              let result = UInt64(exactly: value) else {
            throw LibrarySnapshotError.invalidNumber(key)
        }
        return result
    }

    private func optionalUnsigned(_ values: [String: JSONValue], _ key: String) -> UInt64? {
        guard case let .number(value)? = values[key], value >= 0,
              value.rounded(.towardZero) == value else { return nil }
        return UInt64(exactly: value)
    }

    private func boolean(_ values: [String: JSONValue], _ key: String) throws -> Bool {
        guard case let .boolean(value)? = values[key] else {
            throw LibrarySnapshotError.missingField(key)
        }
        return value
    }

    private func strings(_ values: [String: JSONValue], _ key: String) throws -> [String] {
        try array(values, key).map { value in
            guard case let .string(string) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return string
        }
    }
}

public enum LibrarySnapshotError: Error, Equatable {
    case missingLibrary
    case missingField(String)
    case invalidObject
    case invalidCondition
    case invalidMusicalKey
    case invalidReadiness
    case invalidNumber(String)
    case unboundedPage
}

private extension Optional {
    func required(_ error: @autoclosure () -> LibrarySnapshotError) throws -> Wrapped {
        guard let self else { throw error() }
        return self
    }
}
