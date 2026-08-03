import Foundation
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
            ),
            editor: try decodeEditor(library["editor"]),
            phraseRoleSettings: try decodePhraseRoleSettings(library["phraseRoleSettings"])
        )
    }

    private func decodePhraseRoleSettings(_ value: JSONValue?) throws -> PhraseRoleSettingsState? {
        guard let value, value != .null else { return nil }
        guard case let .object(settings) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let roleValues = try array(settings, "roles")
        let profileValues = try array(settings, "mappingProfiles")
        guard !roleValues.isEmpty, roleValues.count <= 1_000, profileValues.count <= 100 else {
            throw LibrarySnapshotError.unboundedPhraseRoleSettings
        }
        let roles = try roleValues.map { value -> PhraseRoleDefinition in
            guard case let .object(role) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            let usage = try object(role, "usage")
            let affectedTrackValues = try array(usage, "affectedTracks")
            guard affectedTrackValues.count <= 100 else {
                throw LibrarySnapshotError.unboundedPhraseRoleSettings
            }
            return PhraseRoleDefinition(
                id: try string(role, "id"),
                name: try string(role, "name"),
                sortOrder: try UInt16(exactly: unsigned(role, "sortOrder"))
                    .required(.invalidNumber("phraseRole.sortOrder")),
                archived: try boolean(role, "archived"),
                usage: PhraseRoleUsage(
                    phraseCount: try unsigned(usage, "phraseCount"),
                    trackCount: try unsigned(usage, "trackCount"),
                    catalogRowCount: try unsigned(usage, "catalogRowCount"),
                    affectedTracks: try affectedTrackValues.map { trackValue in
                        guard case let .object(track) = trackValue else {
                            throw LibrarySnapshotError.invalidObject
                        }
                        return PhraseRoleAffectedTrack(
                            trackID: try unsigned(track, "trackId"),
                            title: try string(track, "title"),
                            phraseCount: try unsigned(track, "phraseCount")
                        )
                    },
                    hasMoreAffectedTracks: try boolean(usage, "hasMoreAffectedTracks")
                )
            )
        }
        guard Set(roles.map(\.id)).count == roles.count,
              Set(roles.map(\.sortOrder)).count == roles.count,
              roles.sorted(by: { $0.sortOrder < $1.sortOrder }) == roles,
              roles.enumerated().allSatisfy({ index, role in
                  role.sortOrder == UInt16(index + 1)
              }),
              roles.contains(where: { !$0.archived }) else {
            throw LibrarySnapshotError.invalidPhraseRoleSettings
        }
        let roleIDs = Set(roles.map(\.id))
        let profiles = try profileValues.map { value -> SourcePhraseMappingProfile in
            guard case let .object(profile) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            let mappingValues = try array(profile, "mappings")
            guard mappingValues.count <= 1_000 else {
                throw LibrarySnapshotError.unboundedPhraseRoleSettings
            }
            let mappings = try mappingValues.map { value -> SourcePhraseMapping in
                guard case let .object(mapping) = value else {
                    throw LibrarySnapshotError.invalidObject
                }
                return SourcePhraseMapping(
                    rawLabel: try string(mapping, "rawLabel"),
                    roleID: try string(mapping, "roleId")
                )
            }
            guard Set(mappings.map { $0.rawLabel.lowercased() }).count == mappings.count,
                  mappings.allSatisfy({ roleIDs.contains($0.roleID) }) else {
                throw LibrarySnapshotError.invalidPhraseRoleSettings
            }
            return SourcePhraseMappingProfile(
                providerKind: try string(profile, "providerKind"),
                providerName: try string(profile, "providerName"),
                mappings: mappings
            )
        }
        guard Set(profiles.map(\.providerKind)).count == profiles.count else {
            throw LibrarySnapshotError.invalidPhraseRoleSettings
        }
        let revision = try unsigned(settings, "revision")
        guard revision > 0 else { throw LibrarySnapshotError.invalidPhraseRoleSettings }
        return PhraseRoleSettingsState(
            revision: revision,
            defaultsVersion: try UInt16(exactly: unsigned(settings, "defaultsVersion"))
                .required(.invalidNumber("phraseRole.defaultsVersion")),
            roles: roles,
            mappingProfiles: profiles,
            mappingPolicy: try string(settings, "mappingPolicy")
        )
    }

    private func decodeEditor(_ value: JSONValue?) throws -> TrackEditorAnalysis? {
        guard let value, value != .null else { return nil }
        guard case let .object(editor) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        guard case let .object(trackValue) = editor["track"] else {
            throw LibrarySnapshotError.missingField("editor.track")
        }
        let beatGrid = try object(editor, "beatGrid")
        let markers = try array(beatGrid, "markers")
        let waveform = try array(editor, "waveform")
        let phrases = try array(editor, "phrases")
        let roles = try array(editor, "roles")
        let sourcePhraseValues: [JSONValue]
        if case let .array(values)? = editor["sourcePhrases"] {
            sourcePhraseValues = values
        } else {
            sourcePhraseValues = []
        }
        let timeline = try object(editor, "timeline")
        let revisions = try array(timeline, "revisions")
        guard markers.count <= 1_000_000,
              waveform.count <= 100_000,
              phrases.count <= 10_000,
              roles.count <= 1_000,
              revisions.count <= 200,
              sourcePhraseValues.count <= 10_000 else {
            throw LibrarySnapshotError.unboundedEditor
        }
        let beatsPerBar = try UInt8(exactly: unsigned(beatGrid, "beatsPerBar"))
            .required(.invalidNumber("beatsPerBar"))
        guard (1...16).contains(beatsPerBar), !markers.isEmpty else {
            throw LibrarySnapshotError.invalidBeatGrid
        }
        let beats = try markers.map { marker -> TrackEditorBeat in
            guard case let .object(marker) = marker else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackEditorBeat(
                beatIndex: try UInt32(exactly: unsigned(marker, "beatIndex"))
                    .required(.invalidNumber("beatIndex")),
                timeMillis: try unsigned(marker, "timeMillis"),
                barIndex: try UInt32(exactly: unsigned(marker, "barIndex"))
                    .required(.invalidNumber("barIndex")),
                beatInBar: try UInt8(exactly: unsigned(marker, "beatInBar"))
                    .required(.invalidNumber("beatInBar"))
            )
        }
        guard beats.count.isMultiple(of: Int(beatsPerBar)) else {
            throw LibrarySnapshotError.invalidBeatGrid
        }
        for (index, beat) in beats.enumerated() {
            guard beat.beatIndex == UInt32(index),
                  beat.barIndex == UInt32(index / Int(beatsPerBar) + 1),
                  beat.beatInBar == UInt8(index % Int(beatsPerBar) + 1),
                  index == 0 || beat.timeMillis > beats[index - 1].timeMillis else {
                throw LibrarySnapshotError.invalidBeatGrid
            }
        }
        guard !waveform.isEmpty else { throw LibrarySnapshotError.invalidWaveform }
        let decodedPhrases = try phrases.map { value in
            guard case let .object(phrase) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackEditorPhrase(
                id: try unsigned(phrase, "id"),
                startBeat: try UInt32(exactly: unsigned(phrase, "startBeat"))
                    .required(.invalidNumber("phrase.startBeat")),
                endBeat: try UInt32(exactly: unsigned(phrase, "endBeat"))
                    .required(.invalidNumber("phrase.endBeat")),
                roleID: try string(phrase, "roleId"),
                role: try string(phrase, "role"),
                origin: try string(phrase, "origin"),
                loopStrategy: try string(phrase, "loopStrategy")
            )
        }
        var phraseIDs = Set<UInt64>()
        var previousEnd: UInt32 = 0
        for (index, phrase) in decodedPhrases.enumerated() {
            guard phraseIDs.insert(phrase.id).inserted,
                  phrase.startBeat < phrase.endBeat,
                  phrase.endBeat <= UInt32(beats.count),
                  phrase.startBeat.isMultiple(of: UInt32(beatsPerBar)),
                  phrase.endBeat.isMultiple(of: UInt32(beatsPerBar)),
                  (index == 0 ? phrase.startBeat == 0 : phrase.startBeat == previousEnd) else {
                throw LibrarySnapshotError.invalidPhraseTimeline
            }
            previousEnd = phrase.endBeat
        }
        guard previousEnd == UInt32(beats.count) else {
            throw LibrarySnapshotError.invalidPhraseTimeline
        }
        let decodedRoles = try roles.map { value -> TrackEditorRole in
            guard case let .object(role) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackEditorRole(
                id: try string(role, "id"),
                name: try string(role, "name"),
                archived: optionalBoolean(role, "archived") ?? false
            )
        }
        guard Set(decodedRoles.map(\.id)).count == decodedRoles.count,
              !decodedRoles.isEmpty,
              decodedPhrases.allSatisfy({ phrase in
                  decodedRoles.contains(where: { $0.id == phrase.roleID })
              }) else {
            throw LibrarySnapshotError.invalidPhraseTimeline
        }
        let decodedRevisions = try revisions.map { value -> TrackEditorRevision in
            guard case let .object(revision) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackEditorRevision(
                revision: try unsigned(revision, "revision"),
                origin: try string(revision, "origin"),
                reason: try string(revision, "reason"),
                phraseCount: try UInt32(exactly: unsigned(revision, "phraseCount"))
                    .required(.invalidNumber("timeline.phraseCount")),
                restoredFrom: optionalUnsigned(revision, "restoredFrom")
            )
        }
        let timelineRevision = try unsigned(timeline, "revision")
        guard timelineRevision > 0,
              decodedRevisions.first?.revision == timelineRevision,
              zip(decodedRevisions, decodedRevisions.dropFirst()).allSatisfy({ left, right in
                  left.revision > right.revision
              }) else {
            throw LibrarySnapshotError.invalidPhraseTimeline
        }
        let audioURI = try string(editor, "audioUri")
        guard !audioURI.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw LibrarySnapshotError.invalidAudioURI
        }
        return TrackEditorAnalysis(
            track: try decodeTrack(.object(trackValue)),
            audioURI: audioURI,
            beatsPerBar: beatsPerBar,
            beats: beats,
            waveform: try waveform.map { value in
                guard case let .object(point) = value else {
                    throw LibrarySnapshotError.invalidObject
                }
                return TrackEditorWaveformPoint(
                    low: try UInt8(exactly: unsigned(point, "low"))
                        .required(.invalidNumber("waveform.low")),
                    mid: try UInt8(exactly: unsigned(point, "mid"))
                        .required(.invalidNumber("waveform.mid")),
                    high: try UInt8(exactly: unsigned(point, "high"))
                        .required(.invalidNumber("waveform.high"))
                )
            },
            phrases: decodedPhrases,
            roles: decodedRoles,
            sourcePhrases: try sourcePhraseValues.map { value in
                guard case let .object(phrase) = value else {
                    throw LibrarySnapshotError.invalidObject
                }
                let sourcePhrase = TrackEditorSourcePhrase(
                    startBeat: try UInt32(exactly: unsigned(phrase, "startBeat"))
                        .required(.invalidNumber("sourcePhrase.startBeat")),
                    endBeat: try UInt32(exactly: unsigned(phrase, "endBeat"))
                        .required(.invalidNumber("sourcePhrase.endBeat")),
                    rawLabel: try string(phrase, "rawLabel"),
                    providerKind: try string(phrase, "providerKind")
                )
                guard sourcePhrase.startBeat < sourcePhrase.endBeat,
                      sourcePhrase.endBeat <= UInt32(beats.count) else {
                    throw LibrarySnapshotError.invalidPhraseTimeline
                }
                return sourcePhrase
            },
            timeline: TrackEditorTimeline(
                revision: timelineRevision,
                baselineRevision: try string(timeline, "baselineRevision"),
                origin: try string(timeline, "origin"),
                reason: try string(timeline, "reason"),
                canUndo: try boolean(timeline, "canUndo"),
                canRedo: try boolean(timeline, "canRedo"),
                revisions: decodedRevisions
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

    private func optionalBoolean(_ values: [String: JSONValue], _ key: String) -> Bool? {
        guard case let .boolean(value)? = values[key] else { return nil }
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
    case unboundedEditor
    case invalidBeatGrid
    case invalidWaveform
    case invalidPhraseTimeline
    case invalidAudioURI
    case unboundedPhraseRoleSettings
    case invalidPhraseRoleSettings
}

private extension Optional {
    func required(_ error: @autoclosure () -> LibrarySnapshotError) throws -> Wrapped {
        guard let self else { throw error() }
        return self
    }
}
