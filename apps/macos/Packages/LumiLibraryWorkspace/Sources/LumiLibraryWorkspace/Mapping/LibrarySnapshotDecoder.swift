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
            phraseRoleSettings: try decodePhraseRoleSettings(library["phraseRoleSettings"]),
            autoloopCatalog: try decodeAutoloopCatalog(library["autoloopCatalog"]),
            midiIntegration: try decodeMidiIntegration(envelope.payload["midiIntegration"]),
            midiClockIntegration: try decodeMidiClockIntegration(
                envelope.payload["midiClockIntegration"]
            ),
            abletonLinkIntegration: try decodeAbletonLinkIntegration(
                envelope.payload["abletonLinkIntegration"]
            ),
            deckInputIntegration: try decodeDeckInputIntegration(
                envelope.payload["deckInputIntegration"]
            ),
            rekordboxSyncPreview: try decodeRekordboxSyncPreview(
                library["rekordboxSyncPreview"]
            ),
            rekordboxMirror: try decodeRekordboxMirror(library["rekordboxMirror"]),
            rekordboxDevices: try decodeRekordboxDevices(library["rekordboxDevices"]),
            rekordboxDeviceInspection: try decodeRekordboxDeviceInspection(
                library["rekordboxDeviceInspection"]
            ),
            dataManagement: try decodeDataManagement(library["dataManagement"])
        )
    }

    private func decodeDataManagement(_ value: JSONValue?) throws -> DataManagementState {
        guard let value, value != .null else { return .empty }
        guard case let .object(data) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let candidateValues = try optionalArray(data, "resetCandidates")
        let archiveValues = try optionalArray(data, "creativeArchives")
        guard candidateValues.count <= 20_000, archiveValues.count <= 20_000 else {
            throw LibrarySnapshotError.unboundedPage
        }
        let candidates = try candidateValues.map { value -> ResetCandidateTrack in
            guard case let .object(track) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return ResetCandidateTrack(
                trackID: try unsigned(track, "trackId"),
                title: try string(track, "title"),
                artist: try string(track, "artist"),
                timelineRevision: try unsigned(track, "timelineRevision")
            )
        }
        let archives = try archiveValues.map { value -> CreativeTrackArchive in
            guard case let .object(archive) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return CreativeTrackArchive(
                archiveID: try unsigned(archive, "archiveId"),
                title: try string(archive, "title"),
                artist: try string(archive, "artist"),
                phraseCount: try unsigned(archive, "phraseCount"),
                totalBeats: try unsigned(archive, "totalBeats"),
                state: try string(archive, "state"),
                restoredTrackID: optionalUnsigned(archive, "restoredTrackId")
            )
        }
        let preview: LibraryResetPreview?
        if case let .object(reset)? = data["resetPreview"] {
            preview = LibraryResetPreview(
                token: try string(reset, "token"),
                trackCount: try unsigned(reset, "trackCount"),
                playlistCount: try unsigned(reset, "playlistCount"),
                preservedTrackCount: try unsigned(reset, "preservedTrackCount"),
                removedTrackCount: try unsigned(reset, "removedTrackCount"),
                archivedCreativeTrackCount: try unsigned(reset, "archivedCreativeTrackCount"),
                preserveTrackIDs: try unsignedArray(reset, "preserveTrackIds")
            )
        } else {
            preview = nil
        }
        return DataManagementState(
            trackCount: try unsigned(data, "trackCount"),
            playlistCount: try unsigned(data, "playlistCount"),
            userEditedTrackCount: try unsigned(data, "userEditedTrackCount"),
            creativeArchiveCount: try unsigned(data, "creativeArchiveCount"),
            pendingArchiveCount: try unsigned(data, "pendingArchiveCount"),
            resetCandidates: candidates,
            creativeArchives: archives,
            resetPreview: preview
        )
    }

    private func decodeRekordboxDeviceInspection(
        _ value: JSONValue?
    ) throws -> RekordboxDeviceInspectionState? {
        guard let value, value != .null else { return nil }
        guard case let .object(inspection) = value,
              case let .array(playlistValues)? = inspection["playlists"],
              playlistValues.count <= 20_000 else {
            throw LibrarySnapshotError.invalidObject
        }
        let playlists = try playlistValues.map { value -> RekordboxDevicePlaylistState in
            guard case let .object(playlist) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            guard case let .object(counts)? = playlist["statusCounts"],
                  case let .array(trackValues)? = playlist["tracks"],
                  trackValues.count <= 20_000 else {
                throw LibrarySnapshotError.invalidObject
            }
            let tracks = try trackValues.map { trackValue -> RekordboxDeviceTrackState in
                guard case let .object(track) = trackValue else {
                    throw LibrarySnapshotError.invalidObject
                }
                return RekordboxDeviceTrackState(
                    id: try UInt32(exactly: unsigned(track, "id"))
                        .required(.invalidNumber("track.id")),
                    title: try string(track, "title"),
                    artist: try string(track, "artist"),
                    bpmMilli: try unsigned(track, "bpmMilli"),
                    durationMillis: try unsigned(track, "durationMillis"),
                    status: try string(track, "status"),
                    detail: try string(track, "detail")
                )
            }
            return RekordboxDevicePlaylistState(
                id: try UInt32(exactly: unsigned(playlist, "id"))
                    .required(.invalidNumber("playlist.id")),
                path: try string(playlist, "path"),
                name: try string(playlist, "name"),
                trackCount: try unsigned(playlist, "trackCount"),
                statusCounts: RekordboxDeviceStatusCounts(
                    current: try unsigned(counts, "current"),
                    usbNewer: try unsigned(counts, "usbNewer"),
                    usbOutdated: try unsigned(counts, "usbOutdated"),
                    notInLumi: try unsigned(counts, "notInLumi"),
                    conflict: try unsigned(counts, "conflict")
                ),
                tracks: tracks
            )
        }
        let playlistCount = try unsigned(inspection, "playlistCount")
        guard playlistCount == UInt64(playlists.count),
              Set(playlists.map(\.id)).count == playlists.count else {
            throw LibrarySnapshotError.invalidObject
        }
        return RekordboxDeviceInspectionState(
            sourceID: try string(inspection, "sourceId"),
            displayName: try string(inspection, "displayName"),
            databaseRevision: try string(inspection, "databaseRevision"),
            libraryFormat: try string(inspection, "libraryFormat"),
            databaseVersion: try string(inspection, "databaseVersion"),
            exportedAt: try string(inspection, "exportedAt"),
            trackCount: try unsigned(inspection, "trackCount"),
            playlistCount: playlistCount,
            selectedPlaylistIDs: try unsignedArray(inspection, "selectedPlaylistIds").map {
                try UInt32(exactly: $0).required(.invalidNumber("selectedPlaylistIds"))
            },
            playlists: playlists
        )
    }

    private func decodeRekordboxDevices(_ value: JSONValue?) throws -> [RekordboxDeviceState] {
        guard let value, value != .null else { return [] }
        guard case let .array(values) = value, values.count <= 32 else {
            throw LibrarySnapshotError.invalidObject
        }
        return try values.map { value in
            guard case let .object(device) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            let playlistValues = try optionalArray(device, "playlists")
            guard playlistValues.count <= 20_000 else {
                throw LibrarySnapshotError.invalidObject
            }
            return RekordboxDeviceState(
                sourceID: try string(device, "sourceId"),
                displayName: try string(device, "displayName"),
                databaseRevision: try string(device, "databaseRevision"),
                activeTracks: try unsigned(device, "activeTracks"),
                matchedTracks: try unsigned(device, "matchedTracks"),
                unmatchedTracks: try unsigned(device, "unmatchedTracks"),
                syncedAt: optionalString(device, "syncedAt") ?? "Unknown",
                trustState: optionalString(device, "trustState") ?? "trusted",
                currentTracks: optionalUnsigned(device, "currentTracks") ?? 0,
                promotedTracks: optionalUnsigned(device, "promotedTracks") ?? 0,
                protectedTracks: optionalUnsigned(device, "protectedTracks") ?? 0,
                conflictTracks: optionalUnsigned(device, "conflictTracks") ?? 0,
                beatGridRefresh: try boolean(device, "beatGridRefresh"),
                cueRevisionTracked: try boolean(device, "cueRevisionTracked"),
                playlists: try playlistValues.map { value in
                    guard case let .object(playlist) = value else {
                        throw LibrarySnapshotError.invalidObject
                    }
                    return RekordboxDeviceSyncedPlaylistState(
                        id: try UInt32(exactly: unsigned(playlist, "id"))
                            .required(.invalidNumber("device playlist id")),
                        libraryPlaylistID: try unsigned(playlist, "libraryPlaylistId"),
                        name: try string(playlist, "name"),
                        trackCount: try unsigned(playlist, "trackCount")
                    )
                }
            )
        }
    }

    private func decodeRekordboxSyncPreview(
        _ value: JSONValue?
    ) throws -> RekordboxXMLSyncPreview? {
        guard let value, value != .null else { return nil }
        guard case let .object(preview) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let playlistValues = try array(preview, "playlists")
        guard !playlistValues.isEmpty, playlistValues.count <= 20_000 else {
            throw LibrarySnapshotError.unboundedRekordboxSyncPreview
        }
        let playlists = try playlistValues.map { value -> RekordboxXMLSyncPlaylist in
            guard case let .object(playlist) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return RekordboxXMLSyncPlaylist(
                path: try string(playlist, "path"),
                name: try string(playlist, "name"),
                trackCount: try unsigned(playlist, "trackCount")
            )
        }
        let diagnostics = try object(preview, "diagnostics")
        let diff = try object(preview, "diff")
        let followedPlaylistCount = try unsigned(preview, "followedPlaylistCount")
        let contentSHA256 = try string(preview, "contentSha256")
        let selectionPaths = try strings(preview, "selectionPaths")
        let applyState = try string(preview, "applyState")
        guard followedPlaylistCount == UInt64(playlists.count),
              Set(playlists.map(\.path)).count == playlists.count,
              contentSHA256.count == 64,
              contentSHA256.allSatisfy({ $0.isHexDigit }),
              !selectionPaths.isEmpty,
              selectionPaths.count <= 20_000,
              Set(selectionPaths).count == selectionPaths.count,
              ["ready", "applied"].contains(applyState) else {
            throw LibrarySnapshotError.invalidRekordboxSyncPreview
        }
        return RekordboxXMLSyncPreview(
            exportFileName: try string(preview, "exportFileName"),
            contentSHA256: contentSHA256,
            productVersion: try string(preview, "productVersion"),
            collectionTrackCount: try unsigned(preview, "collectionTrackCount"),
            followedPlaylistCount: followedPlaylistCount,
            uniqueTrackCount: try unsigned(preview, "uniqueTrackCount"),
            selectionPaths: selectionPaths,
            includeFutureChildPlaylists: try boolean(
                preview,
                "includeFutureChildPlaylists"
            ),
            playlists: playlists,
            diagnostics: RekordboxXMLSyncDiagnostics(
                duplicatePlaylistReferences: try unsigned(
                    diagnostics,
                    "duplicatePlaylistReferences"
                ),
                missingArtist: try unsigned(diagnostics, "missingArtist"),
                missingBPM: try unsigned(diagnostics, "missingBpm"),
                missingKey: try unsigned(diagnostics, "missingKey"),
                missingDuration: try unsigned(diagnostics, "missingDuration"),
                missingBeatGrid: try unsigned(diagnostics, "missingBeatGrid"),
                missingColour: try unsigned(diagnostics, "missingColour"),
                missingWaveform: try unsigned(diagnostics, "missingWaveform"),
                missingPhrases: try unsigned(diagnostics, "missingPhrases")
            ),
            diff: RekordboxXMLSyncDiff(
                inserted: try unsigned(diff, "inserted"),
                updated: try unsigned(diff, "updated"),
                unchanged: try unsigned(diff, "unchanged"),
                archived: try unsigned(diff, "archived"),
                restored: try unsigned(diff, "restored")
            ),
            applyState: applyState
        )
    }

    private func decodeRekordboxMirror(_ value: JSONValue?) throws -> RekordboxMirrorState? {
        guard let value, value != .null else { return nil }
        guard case let .object(mirror) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let analysisState = try string(mirror, "analysisState")
        guard analysisState == "pending" else {
            throw LibrarySnapshotError.invalidObject
        }
        return RekordboxMirrorState(
            revision: try string(mirror, "revision"),
            activeTracks: try unsigned(mirror, "activeTracks"),
            archivedTracks: try unsigned(mirror, "archivedTracks"),
            playlists: try unsigned(mirror, "playlists"),
            analysisState: analysisState
        )
    }

    private func decodeDeckInputIntegration(
        _ value: JSONValue?
    ) throws -> DeckInputIntegrationState? {
        guard let value, value != .null else { return nil }
        guard case let .object(input) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let state = try string(input, "state")
        guard state == "stopped" || state == "ready" else {
            throw LibrarySnapshotError.invalidObject
        }
        let lastDeckID = try strictOptionalUnsigned(input, "lastDeckId")
        let lastFrameSequence = try strictOptionalUnsigned(input, "lastFrameSequence")
        let protocolName = try string(input, "protocol")
        let isBLTMIDI = protocolName == "BLT MIDI Deck Frame"
        guard lastDeckID.map({ (1...4).contains($0) }) ?? true,
              lastFrameSequence.map({ !isBLTMIDI || $0 <= 127 }) ?? true else {
            throw LibrarySnapshotError.invalidObject
        }
        let discoveredPlayers: [ProDJLinkDeviceState]
        if case let .array(values)? = input["discoveredPlayers"] {
            guard values.count <= 16 else { throw LibrarySnapshotError.invalidObject }
            discoveredPlayers = try values.map { value in
                guard case let .object(player) = value else {
                    throw LibrarySnapshotError.invalidObject
                }
                return ProDJLinkDeviceState(
                    playerNumber: try unsigned(player, "playerNumber"),
                    name: try string(player, "name"),
                    address: optionalString(player, "address")
                )
            }
        } else {
            discoveredPlayers = []
        }
        return DeckInputIntegrationState(
            state: state,
            destinationName: try strictOptionalString(input, "destinationName"),
            protocolName: protocolName,
            protocolVersion: try unsigned(input, "protocolVersion"),
            receivedMessageCount: try unsigned(input, "receivedMessageCount"),
            invalidWordCount: try unsigned(input, "invalidWordCount"),
            committedFrameCount: try unsigned(input, "committedFrameCount"),
            ignoredMessageCount: try unsigned(input, "ignoredMessageCount"),
            duplicateFrameCount: try unsigned(input, "duplicateFrameCount"),
            lastDeckID: lastDeckID,
            lastFrameSequence: lastFrameSequence,
            sourceState: optionalString(input, "sourceState"),
            bridgeVersion: optionalString(input, "bridgeVersion"),
            beatLinkVersion: optionalString(input, "beatLinkVersion"),
            discoveredPlayers: discoveredPlayers,
            lastError: optionalString(input, "lastError")
        )
    }

    private func decodeMidiIntegration(_ value: JSONValue?) throws -> MidiIntegrationState? {
        guard let value, value != .null else { return nil }
        guard case let .object(midi) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let state = try string(midi, "state")
        guard state == "stopped" || state == "ready" else {
            throw LibrarySnapshotError.invalidObject
        }
        return MidiIntegrationState(
            state: state,
            sourceName: try string(midi, "sourceName"),
            midiProtocol: try string(midi, "protocol"),
            sentPulseCount: try unsigned(midi, "sentPulseCount"),
            lastEvent: optionalString(midi, "lastEvent")
        )
    }

    private func decodeMidiClockIntegration(
        _ value: JSONValue?
    ) throws -> MidiClockIntegrationState? {
        guard let value, value != .null else { return nil }
        guard case let .object(clock) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let state = try string(clock, "state")
        guard ["stopped", "ready", "running"].contains(state) else {
            throw LibrarySnapshotError.invalidObject
        }
        return MidiClockIntegrationState(
            state: state,
            sourceName: try string(clock, "sourceName"),
            midiProtocol: try string(clock, "protocol"),
            bpmMilli: optionalUnsigned(clock, "bpmMilli"),
            sentTickCount: try unsigned(clock, "sentTickCount"),
            sentTransportCount: try unsigned(clock, "sentTransportCount"),
            lastEvent: optionalString(clock, "lastEvent"),
            lastError: optionalString(clock, "lastError")
        )
    }

    private func decodeAbletonLinkIntegration(
        _ value: JSONValue?
    ) throws -> AbletonLinkIntegrationState? {
        guard let value, value != .null else { return nil }
        guard case let .object(link) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let state = try string(link, "state")
        guard ["stopped", "starting", "ready", "running", "degraded"].contains(state) else {
            throw LibrarySnapshotError.invalidObject
        }
        return AbletonLinkIntegrationState(
            enabled: try boolean(link, "enabled"),
            state: state,
            provider: try string(link, "provider"),
            helperVersion: optionalString(link, "helperVersion"),
            peers: try unsigned(link, "peers"),
            source: optionalString(link, "source"),
            deckNumber: optionalUnsigned(link, "deckNumber"),
            bpmMilli: optionalUnsigned(link, "bpmMilli"),
            beatWithinBar: optionalUnsigned(link, "beatWithinBar"),
            playing: try boolean(link, "playing"),
            generation: optionalUnsigned(link, "generation"),
            lastBeatAgeMillis: optionalUnsigned(link, "lastBeatAgeMillis"),
            phaseErrorMicros: try strictOptionalSigned(link, "phaseErrorMicros"),
            lastReanchor: optionalString(link, "lastReanchor"),
            lastEvent: optionalString(link, "lastEvent"),
            lastError: optionalString(link, "lastError")
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

    private func decodeAutoloopCatalog(_ value: JSONValue?) throws -> AutoloopCatalogState? {
        guard let value, value != .null else { return nil }
        guard case let .object(catalog) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let themeValues = try array(catalog, "themes")
        let roleValues = try array(catalog, "roles")
        guard themeValues.count == 4, roleValues.count <= 1_000 else {
            throw LibrarySnapshotError.invalidAutoloopCatalog
        }
        let themes = try themeValues.map { value -> AutoloopThemeState in
            guard case let .object(theme) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return AutoloopThemeState(
                id: try unsigned(theme, "id"),
                name: try string(theme, "name"),
                sortOrder: try UInt16(exactly: unsigned(theme, "sortOrder"))
                    .required(.invalidNumber("autoloop.theme.sortOrder"))
            )
        }
        guard Set(themes.map(\.id)).count == themes.count,
              Set(themes.map { $0.name.lowercased() }).count == themes.count,
              themes.enumerated().allSatisfy({ index, theme in
                  theme.id > 0 && theme.sortOrder == UInt16(index + 1)
              }) else {
            throw LibrarySnapshotError.invalidAutoloopCatalog
        }
        let themeIDs = Set(themes.map(\.id))
        var totalVariantCount = 0
        let roles = try roleValues.map { value -> AutoloopRoleMatrixState in
            guard case let .object(role) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            let variantValues = try array(role, "variants")
            totalVariantCount += variantValues.count
            guard totalVariantCount <= 10_000 else {
                throw LibrarySnapshotError.unboundedAutoloopCatalog
            }
            let variants = try variantValues.map { value -> AutoloopVariantState in
                guard case let .object(variant) = value else {
                    throw LibrarySnapshotError.invalidObject
                }
                let cellValues = try array(variant, "cells")
                guard cellValues.count == themes.count else {
                    throw LibrarySnapshotError.invalidAutoloopCatalog
                }
                let cells = try cellValues.map { value -> AutoloopCellState in
                    guard case let .object(cell) = value else {
                        throw LibrarySnapshotError.invalidObject
                    }
                    let status = try string(cell, "status")
                    guard status == "ready" || status == "missing" else {
                        throw LibrarySnapshotError.invalidAutoloopCatalog
                    }
                    let entryID = optionalString(cell, "entryId")
                    let name = optionalString(cell, "name")
                    guard (status == "ready" && entryID != nil && name != nil)
                        || (status == "missing" && entryID == nil && name == nil) else {
                        throw LibrarySnapshotError.invalidAutoloopCatalog
                    }
                    return AutoloopCellState(
                        themeID: try unsigned(cell, "themeId"),
                        buttonNumber: optionalUnsigned(cell, "buttonNumber").flatMap(UInt16.init(exactly:)),
                        entryID: entryID,
                        name: name,
                        status: status
                    )
                }
                guard Set(cells.map(\.themeID)) == themeIDs else {
                    throw LibrarySnapshotError.invalidAutoloopCatalog
                }
                return AutoloopVariantState(
                    id: try string(variant, "id"),
                    name: try string(variant, "name"),
                    sortOrder: try UInt16(exactly: unsigned(variant, "sortOrder"))
                        .required(.invalidNumber("autoloop.variant.sortOrder")),
                    archived: try boolean(variant, "archived"),
                    cells: cells
                )
            }
            guard Set(variants.map(\.id)).count == variants.count,
                  variants.enumerated().allSatisfy({ index, variant in
                      variant.sortOrder == UInt16(index + 1)
                  }) else {
                throw LibrarySnapshotError.invalidAutoloopCatalog
            }
            return AutoloopRoleMatrixState(
                id: try string(role, "id"),
                name: try string(role, "name"),
                archived: try boolean(role, "archived"),
                variants: variants
            )
        }
        guard Set(roles.map(\.id)).count == roles.count else {
            throw LibrarySnapshotError.invalidAutoloopCatalog
        }
        let preflightValue = try object(catalog, "preflight")
        let missingValues = try array(preflightValue, "missingCells")
        guard missingValues.count <= 200 else {
            throw LibrarySnapshotError.unboundedAutoloopCatalog
        }
        let missing = try missingValues.map { value -> MissingAutoloopCellState in
            guard case let .object(cell) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return MissingAutoloopCellState(
                themeID: try unsigned(cell, "themeId"),
                roleID: try string(cell, "roleId"),
                variantID: try string(cell, "variantId")
            )
        }
        let missingCount = try unsigned(preflightValue, "missingCellCount")
        let missingRoleValues = try array(preflightValue, "missingRoleIds")
        guard missingRoleValues.count <= 200 else {
            throw LibrarySnapshotError.unboundedAutoloopCatalog
        }
        let missingRoleIDs = try missingRoleValues.map { value -> String in
            guard case let .string(roleID) = value else {
                throw LibrarySnapshotError.invalidAutoloopCatalog
            }
            return roleID
        }
        let missingRoleCount = try unsigned(preflightValue, "missingRoleCount")
        let hasMoreMissingCells = try boolean(preflightValue, "hasMoreMissingCells")
        let hasMoreMissingRoles = try boolean(preflightValue, "hasMoreMissingRoles")
        let preflightStatus = try string(preflightValue, "status")
        guard missingCount >= UInt64(missing.count),
              hasMoreMissingCells || missingCount == UInt64(missing.count),
              missingRoleCount >= UInt64(missingRoleIDs.count),
              hasMoreMissingRoles || missingRoleCount == UInt64(missingRoleIDs.count),
              Set(missingRoleIDs).count == missingRoleIDs.count,
              missingRoleIDs.allSatisfy({ roleID in
                  roles.contains(where: { $0.id == roleID && !$0.archived && $0.variants.allSatisfy(\.archived) })
              }),
              missing.allSatisfy({ cell in
                  themeIDs.contains(cell.themeID)
                      && roles.contains(where: { role in
                          role.id == cell.roleID
                              && role.variants.contains(where: { variant in
                                  variant.id == cell.variantID
                                      && !variant.archived
                                      && variant.cells.contains(where: {
                                          $0.themeID == cell.themeID && $0.isMissing
                                      })
                              })
                      })
              }),
              ["ready", "incomplete"].contains(preflightStatus),
              (preflightStatus == "ready") == (missingCount == 0 && missingRoleCount == 0) else {
            throw LibrarySnapshotError.invalidAutoloopCatalog
        }
        let target = try object(catalog, "targetCapabilities")
        let revision = try unsigned(catalog, "revision")
        let defaultsVersion = try unsigned(catalog, "defaultsVersion")
        let targetValidationOwner = try string(target, "validationOwner")
        let hardCodedPhysicalCapacity = try boolean(target, "hardCodedPhysicalCapacity")
        guard revision > 0,
              defaultsVersion > 0,
              targetValidationOwner == "targetAdapter",
              !hardCodedPhysicalCapacity else {
            throw LibrarySnapshotError.invalidAutoloopCatalog
        }
        return AutoloopCatalogState(
            revision: revision,
            defaultsVersion: try UInt16(exactly: defaultsVersion)
                .required(.invalidNumber("autoloop.defaultsVersion")),
            themes: themes,
            roles: roles,
            preflight: AutoloopPreflightState(
                status: preflightStatus,
                missingCellCount: missingCount,
                missingCells: missing,
                hasMoreMissingCells: hasMoreMissingCells,
                missingRoleCount: missingRoleCount,
                missingRoleIDs: missingRoleIDs,
                hasMoreMissingRoles: hasMoreMissingRoles
            ),
            targetValidationOwner: targetValidationOwner,
            hardCodedPhysicalCapacity: hardCodedPhysicalCapacity
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
            let roleID = try string(phrase, "roleId")
            return TrackEditorPhrase(
                id: try unsigned(phrase, "id"),
                startBeat: try UInt32(exactly: unsigned(phrase, "startBeat"))
                    .required(.invalidNumber("phrase.startBeat")),
                endBeat: try UInt32(exactly: unsigned(phrase, "endBeat"))
                    .required(.invalidNumber("phrase.endBeat")),
                roleID: roleID,
                role: try string(phrase, "role"),
                origin: try string(phrase, "origin"),
                loopStrategy: try decodeLoopStrategy(phrase, roleID: roleID)
            )
        }
        var phraseIDs = Set<UInt64>()
        var previousEnd: UInt32 = 0
        for (index, phrase) in decodedPhrases.enumerated() {
            guard phraseIDs.insert(phrase.id).inserted,
                  phrase.startBeat < phrase.endBeat,
                  phrase.endBeat <= UInt32(beats.count),
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
            ),
            sourceReconciliation: try decodeSourceReconciliation(editor["sourceReconciliation"])
        )
    }

    private func decodeSourceReconciliation(
        _ value: JSONValue?
    ) throws -> TrackSourceReconciliation? {
        guard let value, value != .null else { return nil }
        guard case let .object(object) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let changes = try array(object, "changes").map { value -> String in
            guard case let .string(change) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return change
        }
        let ambiguityValues = try array(object, "rebaseAmbiguities")
        let ambiguities = try ambiguityValues.map { value -> UInt16 in
            guard case let .number(number) = value,
                  number.rounded() == number,
                  let unsigned = UInt64(exactly: number),
                  let result = UInt16(exactly: unsigned) else {
                throw LibrarySnapshotError.invalidNumber("rebaseAmbiguities")
            }
            return result
        }
        let conflicts = try array(object, "conflicts").map { value -> TrackSourceConflict in
            guard case let .object(conflict) = value else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackSourceConflict(
                phraseIndex: try UInt16(exactly: unsigned(conflict, "phraseIndex"))
                    .required(.invalidNumber("phraseIndex")),
                lumi: try decodeSourcePhraseVersion(conflict["lumi"]),
                source: try decodeSourcePhraseVersion(conflict["source"])
            )
        }
        guard changes.count <= 4,
              conflicts.count <= 10_000,
              ambiguities.count <= 10_000,
              Set(conflicts.map(\.phraseIndex)).count == conflicts.count else {
            throw LibrarySnapshotError.unboundedEditor
        }
        return TrackSourceReconciliation(
            fromRevision: try string(object, "fromRevision"),
            toRevision: try string(object, "toRevision"),
            sourceLibraryRevision: try string(object, "sourceLibraryRevision"),
            changes: changes,
            metadataOnly: try boolean(object, "metadataOnly"),
            requiresTimelineDecision: try boolean(object, "requiresTimelineDecision"),
            sourceTotalBeats: try UInt32(exactly: unsigned(object, "sourceTotalBeats"))
                .required(.invalidNumber("sourceTotalBeats")),
            rebaseAmbiguities: ambiguities,
            conflicts: conflicts
        )
    }

    private func decodeSourcePhraseVersion(
        _ value: JSONValue?
    ) throws -> TrackSourcePhraseVersion? {
        guard let value, value != .null else { return nil }
        guard case let .object(object) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        let start = try UInt32(exactly: unsigned(object, "startBeat"))
            .required(.invalidNumber("startBeat"))
        let end = try UInt32(exactly: unsigned(object, "endBeat"))
            .required(.invalidNumber("endBeat"))
        guard start < end else { throw LibrarySnapshotError.invalidPhraseTimeline }
        return TrackSourcePhraseVersion(
            startBeat: start,
            endBeat: end,
            roleID: try string(object, "roleId")
        )
    }

    private func decodeLoopStrategy(
        _ phrase: [String: JSONValue],
        roleID: String
    ) throws -> TrackEditorLoopStrategy {
        let value = try object(phrase, "loopStrategy")
        let kind = try string(value, "kind")
        let locked = try boolean(value, "locked")
        let provenance = try string(value, "provenance")
        let rowRoleID = try string(value, "rowRoleId")
        let fixedVariantID = optionalString(value, "fixedVariantId")
        let overrideValues = try array(value, "themeOverrides")
        guard overrideValues.count <= 4 else {
            throw LibrarySnapshotError.invalidPhraseTimeline
        }
        let overrides = try overrideValues.map { item -> TrackEditorThemeVariantOverride in
            guard case let .object(overrideValue) = item else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackEditorThemeVariantOverride(
                themeID: try unsigned(overrideValue, "themeId"),
                variantID: try string(overrideValue, "variantId")
            )
        }
        let issueValues = try array(value, "issues")
        guard issueValues.count <= 4 else {
            throw LibrarySnapshotError.invalidPhraseTimeline
        }
        let issues = try issueValues.map { item -> TrackEditorLoopStrategyIssue in
            guard case let .object(issue) = item else {
                throw LibrarySnapshotError.invalidObject
            }
            return TrackEditorLoopStrategyIssue(
                reason: try string(issue, "reason"),
                themeID: try unsigned(issue, "themeId"),
                variantID: optionalString(issue, "variantId")
            )
        }
        let validatedCatalogRevision = try unsigned(value, "validatedCatalogRevision")
        let status = try string(value, "status")
        let overridesAreOrdered = overrides.enumerated().allSatisfy { index, item in
            item.themeID > 0 && (index == 0 || overrides[index - 1].themeID < item.themeID)
        }
        let validShape = switch kind {
        case "auto":
            !locked && provenance == "automaticDefault" && fixedVariantID == nil && overrides.isEmpty
        case "fixedVariant":
            locked && provenance == "userSelection" && fixedVariantID != nil && overrides.isEmpty
        case "themeSpecificExact":
            locked && provenance == "userSelection" && fixedVariantID == nil && !overrides.isEmpty
        default:
            false
        }
        guard rowRoleID == roleID,
              validatedCatalogRevision > 0,
              overridesAreOrdered,
              validShape,
              ["ready", "incomplete", "stale"].contains(status),
              (status == "ready") == issues.isEmpty else {
            throw LibrarySnapshotError.invalidPhraseTimeline
        }
        return TrackEditorLoopStrategy(
            kind: kind,
            locked: locked,
            provenance: provenance,
            rowRoleID: rowRoleID,
            fixedVariantID: fixedVariantID,
            themeOverrides: overrides,
            validatedCatalogRevision: validatedCatalogRevision,
            status: status,
            issues: issues
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
            warnings: try strings(readiness, "warnings"),
            usbSources: try optionalArray(object, "usbSources").map { value in
                guard case let .object(source) = value else {
                    throw LibrarySnapshotError.invalidObject
                }
                return LibraryTrackUSBSource(
                    sourceID: try string(source, "sourceId"),
                    displayName: try string(source, "displayName"),
                    syncDisposition: try string(source, "syncDisposition")
                )
            }
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

    private func optionalArray(_ values: [String: JSONValue], _ key: String) throws -> [JSONValue] {
        guard let value = values[key] else { return [] }
        guard case let .array(array) = value else {
            throw LibrarySnapshotError.invalidObject
        }
        return array
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

    private func unsignedArray(
        _ values: [String: JSONValue],
        _ key: String
    ) throws -> [UInt64] {
        try array(values, key).map { value in
            guard case let .number(number) = value, number >= 0,
                  number.rounded(.towardZero) == number,
                  let result = UInt64(exactly: number) else {
                throw LibrarySnapshotError.invalidNumber(key)
            }
            return result
        }
    }

    private func optionalUnsigned(_ values: [String: JSONValue], _ key: String) -> UInt64? {
        guard case let .number(value)? = values[key], value >= 0,
              value.rounded(.towardZero) == value else { return nil }
        return UInt64(exactly: value)
    }

    private func strictOptionalUnsigned(
        _ values: [String: JSONValue],
        _ key: String
    ) throws -> UInt64? {
        guard let value = values[key], value != .null else { return nil }
        guard let decoded = optionalUnsigned(values, key) else {
            throw LibrarySnapshotError.invalidNumber(key)
        }
        return decoded
    }

    private func strictOptionalSigned(
        _ values: [String: JSONValue],
        _ key: String
    ) throws -> Int? {
        guard let value = values[key], value != .null else { return nil }
        guard case let .number(number) = value,
              number.rounded(.towardZero) == number,
              let decoded = Int(exactly: number) else {
            throw LibrarySnapshotError.invalidNumber(key)
        }
        return decoded
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

    private func optionalString(_ values: [String: JSONValue], _ key: String) -> String? {
        guard case let .string(value)? = values[key], !value.isEmpty else { return nil }
        return value
    }

    private func strictOptionalString(
        _ values: [String: JSONValue],
        _ key: String
    ) throws -> String? {
        guard let value = values[key], value != .null else { return nil }
        guard let decoded = optionalString(values, key) else {
            throw LibrarySnapshotError.invalidObject
        }
        return decoded
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
    case unboundedAutoloopCatalog
    case invalidAutoloopCatalog
    case unboundedRekordboxSyncPreview
    case invalidRekordboxSyncPreview
}

private extension Optional {
    func required(_ error: @autoclosure () -> LibrarySnapshotError) throws -> Wrapped {
        guard let self else { throw error() }
        return self
    }
}
