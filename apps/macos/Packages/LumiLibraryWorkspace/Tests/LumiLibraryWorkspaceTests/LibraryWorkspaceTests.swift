import Foundation
import LumiProtocol
import Testing
@testable import LumiLibraryWorkspace

@Suite("Library workspace")
struct LibraryWorkspaceTests {
    @Test("Authoritative engine library metadata decodes into a bounded page")
    func decodesLibrarySnapshot() throws {
        let state = try LibrarySnapshotDecoder().decode(envelope(trackValues: [trackValue()]))
        #expect(state.condition == .ready)
        #expect(state.source?.id == "lumi-demo-library")
        #expect(state.collectionTotal == 10_000)
        #expect(state.playlists.first?.name == "All Demo Tracks")
        #expect(state.page.total == 10_000)
        #expect(state.page.tracks.first?.title == "Horizon Lines")
        #expect(state.page.tracks.first?.readiness == .ready)
        #expect(state.page.tracks.first?.missingCapabilities == [])
    }

    @Test("MIDI integration state decodes independently from the library catalog")
    func decodesMidiIntegrationState() throws {
        let midi: JSONValue = .object([
            "state": .string("ready"),
            "sourceName": .string("Lumi Virtual MIDI"),
            "protocol": .string("MIDI 1.0 UMP"),
            "sentPulseCount": .number(1),
            "lastEvent": .string("Learn pulse sent")
        ])
        let state = try LibrarySnapshotDecoder().decode(
            envelope(trackValues: [trackValue()], midiIntegration: midi)
        )

        #expect(state.midiIntegration?.isReady == true)
        #expect(state.midiIntegration?.sourceName == "Lumi Virtual MIDI")
        #expect(state.midiIntegration?.sentPulseCount == 1)
    }

    @Test("Wire pages over 200 tracks are rejected before presentation")
    func rejectsUnboundedPage() {
        let values = Array(repeating: trackValue(), count: 201)
        #expect(throws: LibrarySnapshotError.unboundedPage) {
            try LibrarySnapshotDecoder().decode(envelope(trackValues: values))
        }
    }

    @Test("Readiness filters use explicit provider state")
    func filtersExplicitReadiness() {
        let degraded = LibraryWorkspaceFixtures.degraded
        #expect(
            LibraryWorkspacePresenter.visibleTracks(in: degraded, filter: .missingAnalysis)
                .map(\.title) == ["Partial Analysis"]
        )
        #expect(
            LibraryWorkspacePresenter.visibleTracks(in: degraded, filter: .ready).count == 2
        )
    }

    @Test("A 10,000-track result remains a 50-row native page")
    func largeLibraryRemainsBounded() throws {
        let pageTracks = (0..<50).map { index in
            var value = trackValue()
            guard case var .object(object) = value else { return value }
            object["id"] = .number(Double(index + 1))
            object["title"] = .string("Track \(index + 1)")
            value = .object(object)
            return value
        }
        let clock = ContinuousClock()
        let started = clock.now
        let state = try LibrarySnapshotDecoder().decode(envelope(trackValues: pageTracks))
        let duration = started.duration(to: clock.now)
        #expect(state.page.total == 10_000)
        #expect(state.page.tracks.count == 50)
        #expect(LibraryWorkspacePresenter.pageCount(in: state) == 200)
        #expect(duration < .milliseconds(100))
    }

    @Test("English localization resources are complete for primary controls")
    func localizesPrimaryControls() {
        #expect(LibraryWorkspaceLocalization.value("library.title") == "Library")
        #expect(LibraryWorkspaceLocalization.value("library.search").contains("Search"))
        #expect(LibraryWorkspaceLocalization.value("library.openEditor").contains("Editor"))
        #expect(LibraryWorkspaceLocalization.value("editor.playPause") == "Play or pause")
        #expect(LibraryWorkspaceLocalization.value("editor.loopPhrase") == "Loop selected phrase")
        #expect(LibraryWorkspaceLocalization.value("editor.createSelection").contains("selection"))
        #expect(LibraryWorkspaceLocalization.value("editor.planIsolation").contains("Live plan"))
        #expect(LibraryWorkspaceLocalization.value("settings.phraseRoles") == "Phrase Roles")
        #expect(LibraryWorkspaceLocalization.value("settings.mappingPolicy").contains("never silently rewritten"))
    }

    @Test("Phrase-role settings decode stable IDs, usage, archive state, and provider mappings")
    func decodesPhraseRoleSettings() throws {
        let state = try LibrarySnapshotDecoder().decode(
            envelope(trackValues: [trackValue()], phraseRoleSettings: phraseRoleSettingsValue())
        )
        let settings = try #require(state.phraseRoleSettings)
        #expect(settings.revision == 4)
        #expect(settings.roles.map(\.id) == ["intro-outro", "synth"])
        #expect(settings.roles[1].usage.trackCount == 1)
        #expect(settings.roles[1].usage.affectedTracks.first?.title == "Northern Pulse")
        #expect(settings.mappingProfiles.first?.providerKind == "rekordbox7")
        #expect(settings.mappingProfiles.first?.mappings.first?.rawLabel == "Intro")
        #expect(settings.mappingProfiles.first?.mappings.first?.roleID == "intro-outro")
    }

    @Test("BLT MIDI input diagnostics decode independently from SoundSwitch output")
    func decodesDeckInputIntegration() throws {
        let state = try LibrarySnapshotDecoder().decode(
            envelope(
                trackValues: [trackValue()],
                deckInputIntegration: .object([
                    "state": .string("ready"),
                    "destinationName": .string("Lumi Deck Input"),
                    "protocol": .string("BLT MIDI Deck Frame"),
                    "protocolVersion": .number(1),
                    "receivedMessageCount": .number(48),
                    "invalidWordCount": .number(0),
                    "committedFrameCount": .number(3),
                    "ignoredMessageCount": .number(1),
                    "duplicateFrameCount": .number(0),
                    "lastDeckId": .number(2),
                    "lastFrameSequence": .number(7)
                ])
            )
        )
        let input = try #require(state.deckInputIntegration)
        #expect(input.destinationName == "Lumi Deck Input")
        #expect(input.isReceiving)
        #expect(input.lastDeckID == 2)
        #expect(state.midiIntegration == nil)
    }

    @Test("Duplicate stable role IDs are rejected before Settings renders")
    func rejectsDuplicatePhraseRoleIDs() {
        var settings = phraseRoleSettingsValue()
        guard case var .object(object) = settings,
              case let .array(roles) = object["roles"] else {
            Issue.record("Phrase-role fixture must contain roles")
            return
        }
        object["roles"] = .array([roles[0], roles[0]])
        settings = .object(object)
        #expect(throws: LibrarySnapshotError.invalidPhraseRoleSettings) {
            try LibrarySnapshotDecoder().decode(
                envelope(trackValues: [trackValue()], phraseRoleSettings: settings)
            )
        }
    }

    @Test("Non-contiguous phrase-role ordering is rejected before Settings renders")
    func rejectsNonContiguousPhraseRoleOrdering() {
        var settings = phraseRoleSettingsValue()
        guard case var .object(object) = settings,
              case var .array(roles) = object["roles"],
              case var .object(secondRole) = roles[1] else {
            Issue.record("Phrase-role fixture must contain two roles")
            return
        }
        secondRole["sortOrder"] = .number(3)
        roles[1] = .object(secondRole)
        object["roles"] = .array(roles)
        settings = .object(object)

        #expect(throws: LibrarySnapshotError.invalidPhraseRoleSettings) {
            try LibrarySnapshotDecoder().decode(
                envelope(trackValues: [trackValue()], phraseRoleSettings: settings)
            )
        }
    }

    @Test("Autoloop catalog decodes four Theme columns and flexible role variants")
    func decodesAutoloopCatalog() throws {
        let state = try LibrarySnapshotDecoder().decode(
            envelope(trackValues: [trackValue()], autoloopCatalog: autoloopCatalogValue())
        )
        let catalog = try #require(state.autoloopCatalog)
        #expect(catalog.revision == 7)
        #expect(catalog.themes.map(\.name) == ["Electric Bloom", "Deep Ocean", "Solar Flare", "Ultraviolet"])
        #expect(catalog.roles.first?.id == "synth")
        #expect(catalog.roles.first?.variants.count == 2)
        #expect(catalog.roles.first?.variants[1].cells.last?.isMissing == true)
        #expect(catalog.preflight.missingCellCount == 1)
        #expect(catalog.preflight.missingRoleCount == 1)
        #expect(catalog.preflight.missingRoleIDs == ["vocal"])
        #expect(!catalog.hardCodedPhysicalCapacity)
    }

    @Test("Built-in SoundSwitch profile projects four banks with 32 explicit button mappings")
    func projectsSoundSwitchAutoloopBanks() {
        let catalog = AutoloopCatalogFixtures.incomplete
        #expect(SoundSwitchOutputProfileState.builtIn.bankCount == 4)
        #expect(SoundSwitchOutputProfileState.builtIn.slotsPerBank == 32)
        let projectedBanks = SoundSwitchOutputProfileProjection.banks(catalog: catalog)
        #expect(projectedBanks.map(\.number) == [1, 2, 3, 4])
        #expect(projectedBanks.allSatisfy { $0.organization == .theme })
        #expect(projectedBanks.first?.groupName == "Blue Pink")
        let banks = catalog.themes.map { bank in
            SoundSwitchOutputProfileProjection.slots(for: bank.id, catalog: catalog)
        }
        #expect(banks.count == 4)
        #expect(banks.allSatisfy { $0.count == 32 })
        #expect(banks.allSatisfy { $0.map(\.number) == Array(1...32) })
        #expect(banks[0][0].roleID == "intro-outro")
        #expect(banks[3][0].roleID == "intro-outro")
        #expect(banks.flatMap { $0 }.allSatisfy { $0.status == .mapped })
        #expect(banks.flatMap { $0 }.count == 128)
        #expect(
            SoundSwitchOutputProfileProjection.controllerGridSlots(
                for: 1,
                catalog: catalog
            ).map(\.number)
                == [
                    1, 9, 17, 25,
                    2, 10, 18, 26,
                    3, 11, 19, 27,
                    4, 12, 20, 28,
                    5, 13, 21, 29,
                    6, 14, 22, 30,
                    7, 15, 23, 31,
                    8, 16, 24, 32
                ]
        )
    }

    @Test("The same SoundSwitch button may use a different Phrase Type in each bank")
    func projectsIndependentSoundSwitchButtons() throws {
        let catalog = AutoloopCatalogFixtures.incomplete
        let bankOne = SoundSwitchOutputProfileProjection.slots(for: 1, catalog: catalog)
        let bankTwo = SoundSwitchOutputProfileProjection.slots(for: 2, catalog: catalog)
        let bankOneButtonSix = try #require(bankOne.first { $0.number == 6 })
        let bankTwoButtonSix = try #require(bankTwo.first { $0.number == 6 })
        #expect(bankOneButtonSix.roleID == "breakdown-2")
        #expect(bankOneButtonSix.entryName == "BREAKDOWN 2 BLUE PINK")
        #expect(bankTwoButtonSix.roleID == "bridge")
        #expect(bankTwoButtonSix.entryName == "BRIDGE GREEN PINK")
    }

    @Test("Autoloop catalog rejects any projection that conflates Theme targets with a variable count")
    func rejectsNonFourThemeCatalog() {
        var catalog = autoloopCatalogValue()
        guard case var .object(object) = catalog,
              case var .array(themes) = object["themes"] else {
            Issue.record("Autoloop fixture must contain Themes")
            return
        }
        themes.removeLast()
        object["themes"] = .array(themes)
        catalog = .object(object)
        #expect(throws: LibrarySnapshotError.invalidAutoloopCatalog) {
            try LibrarySnapshotDecoder().decode(
                envelope(trackValues: [trackValue()], autoloopCatalog: catalog)
            )
        }
    }

    @Test("Autoloop preflight cannot claim ready while coverage is incomplete")
    func rejectsContradictoryAutoloopPreflight() {
        var catalog = autoloopCatalogValue()
        guard case var .object(object) = catalog,
              case var .object(preflight) = object["preflight"] else {
            Issue.record("Autoloop fixture must contain preflight")
            return
        }
        preflight["status"] = .string("ready")
        object["preflight"] = .object(preflight)
        catalog = .object(object)

        #expect(throws: LibrarySnapshotError.invalidAutoloopCatalog) {
            try LibrarySnapshotDecoder().decode(
                envelope(trackValues: [trackValue()], autoloopCatalog: catalog)
            )
        }
    }

    @Test("Track editor analysis uses one bounded beat coordinate system")
    func decodesTrackEditorAnalysis() throws {
        let state = try LibrarySnapshotDecoder().decode(
            envelope(trackValues: [trackValue()], editorValue: editorValue())
        )

        let editor = try #require(state.editor)
        #expect(editor.track.title == "Horizon Lines")
        #expect(editor.beatsPerBar == 4)
        #expect(editor.totalBars == 2)
        #expect(editor.waveform.count == 3)
        #expect(editor.phrases.map(\.role) == ["Intro", "Build"])
        #expect(editor.phrases.map(\.roleID) == ["intro-outro", "buildup-1"])
        #expect(editor.timeline.revision == 1)
        #expect(editor.timeline.revisions.count == 1)
        #expect(editor.sourcePhrases.first?.rawLabel == "Intro")
        #expect(editor.phrases.first?.loopStrategy.kind == "auto")
        #expect(editor.phrases.first?.loopStrategy.rowRoleID == "intro-outro")
        #expect(editor.phrases.first?.loopStrategy.locked == false)
        #expect(!editor.timeline.canUndo)
        #expect(editor.phraseTimeRange(editor.phrases[1]) == 2_000..<4_000)
    }

    @Test("Source reconciliation exposes classified changes and explicit phrase conflicts")
    func decodesSourceReconciliation() throws {
        var editor = editorValue()
        guard case var .object(object) = editor else {
            Issue.record("Editor fixture must be an object")
            return
        }
        object["sourceReconciliation"] = .object([
            "fromRevision": .string("track-v1"),
            "toRevision": .string("track-v2"),
            "sourceLibraryRevision": .string("library-v2"),
            "metadataOnly": .boolean(false),
            "requiresTimelineDecision": .boolean(true),
            "changes": .array([.string("beatGrid"), .string("rawPhrases")]),
            "sourceTotalBeats": .number(8),
            "rebaseAmbiguities": .array([.number(0)]),
            "conflicts": .array([
                .object([
                    "phraseIndex": .number(0),
                    "lumi": .object([
                        "startBeat": .number(0),
                        "endBeat": .number(4),
                        "roleId": .string("intro-outro")
                    ]),
                    "source": .object([
                        "startBeat": .number(0),
                        "endBeat": .number(8),
                        "roleId": .string("intro-outro")
                    ])
                ])
            ])
        ])
        editor = .object(object)

        let state = try LibrarySnapshotDecoder().decode(
            envelope(trackValues: [trackValue()], editorValue: editor)
        )
        let reconciliation = try #require(state.editor?.sourceReconciliation)
        #expect(reconciliation.changes == ["beatGrid", "rawPhrases"])
        #expect(reconciliation.rebaseAmbiguities == [0])
        #expect(reconciliation.conflicts.first?.source?.endBeat == 8)
    }

    @Test("Loop strategy rows must remain compatible with their phrase role")
    func rejectsLoopStrategyRoleMismatch() {
        var editor = editorValue()
        guard case var .object(editorObject) = editor,
              case var .array(phrases) = editorObject["phrases"],
              case var .object(firstPhrase) = phrases.first,
              case var .object(strategy) = firstPhrase["loopStrategy"] else {
            Issue.record("Editor fixture must contain a loop strategy")
            return
        }
        strategy["rowRoleId"] = .string("drop")
        firstPhrase["loopStrategy"] = .object(strategy)
        phrases[0] = .object(firstPhrase)
        editorObject["phrases"] = .array(phrases)
        editor = .object(editorObject)

        #expect(throws: LibrarySnapshotError.invalidPhraseTimeline) {
            try LibrarySnapshotDecoder().decode(
                envelope(trackValues: [trackValue()], editorValue: editor)
            )
        }
    }

    @Test("Incomplete bars are rejected before the editor can render")
    func rejectsIncompleteBeatGrid() {
        var editor = editorValue()
        guard case var .object(editorObject) = editor,
              case var .object(beatGrid) = editorObject["beatGrid"],
              case var .array(markers) = beatGrid["markers"] else {
            Issue.record("Editor fixture must contain a beat grid")
            return
        }
        markers.removeLast()
        beatGrid["markers"] = .array(markers)
        editorObject["beatGrid"] = .object(beatGrid)
        editor = .object(editorObject)

        #expect(throws: LibrarySnapshotError.invalidBeatGrid) {
            try LibrarySnapshotDecoder().decode(
                envelope(trackValues: [trackValue()], editorValue: editor)
            )
        }
    }

    @Test("Viewport permits fractional pan and zoom while preserving invertible beat positions")
    func trackEditorViewportAlignment() {
        for visibleBeats: Double in [1, 3.5, 8, 17.25, 64, 128] {
            let viewport = TrackEditorViewport(
                startBeat: 13.25,
                visibleBeats: visibleBeats,
                totalBeats: 256,
                beatsPerBar: 4
            )
            for beat in stride(
                from: viewport.startBeat,
                through: viewport.endBeat,
                by: 0.5
            ) {
                let x = viewport.x(forBeat: beat, width: 1_024)
                #expect(abs(viewport.beat(atX: x, width: 1_024) - beat) < 0.000_001)
            }
        }

        let viewport = TrackEditorViewport(
            startBeat: 13.25,
            visibleBeats: 16,
            totalBeats: 256,
            beatsPerBar: 4
        )
        let trackpadPan = viewport.panned(byPixels: 125, width: 1_000)
        #expect(abs(trackpadPan.startBeat - 15.25) < 0.000_001)
    }

    @Test("Phrase mutations quantize fractional positions to whole beats")
    func phraseSelectionSnapsToBeats() {
        #expect(TrackEditorEditGeometry.quantizedBeat(0.49, totalBeats: 64) == 0)
        #expect(TrackEditorEditGeometry.quantizedBeat(0.5, totalBeats: 64) == 1)
        #expect(TrackEditorEditGeometry.quantizedBeat(3.99, totalBeats: 64) == 4)
        #expect(TrackEditorEditGeometry.quantizedBeat(63.99, totalBeats: 64) == 64)
        #expect(
            TrackEditorEditGeometry.beatSelection(anchorBeat: 6, currentBeat: 2, totalBeats: 16)
                == 2..<7
        )
    }

    @Test("Persisted phrase points may land on any whole beat")
    func decodesNonBarAlignedPhrasePoint() throws {
        var editor = editorValue()
        guard case var .object(editorObject) = editor,
              case var .array(phrases) = editorObject["phrases"],
              case var .object(firstPhrase) = phrases[0],
              case var .object(secondPhrase) = phrases[1] else {
            Issue.record("Editor fixture must contain adjacent phrases")
            return
        }
        firstPhrase["endBeat"] = .number(3)
        secondPhrase["startBeat"] = .number(3)
        phrases[0] = .object(firstPhrase)
        phrases[1] = .object(secondPhrase)
        editorObject["phrases"] = .array(phrases)
        editor = .object(editorObject)

        let state = try LibrarySnapshotDecoder().decode(
            envelope(trackValues: [trackValue()], editorValue: editor)
        )
        #expect(state.editor?.phrases.map(\.startBeat) == [0, 3])
        #expect(state.editor?.phrases.map(\.endBeat) == [3, 8])
    }

    @Test("Viewport movement and zoom clamp to the track without bar snapping")
    func trackEditorViewportClamping() {
        let initial = TrackEditorViewport(startBeat: 56.5, visibleBeats: 32, totalBeats: 64, beatsPerBar: 4)
        #expect(initial.startBeat == 32)
        #expect(initial.moving(byBeats: -100).startBeat == 0)
        #expect(initial.moving(byBeats: 100).startBeat == 32)
        let zoomed = initial.zoomed(to: 15.5, aroundBeat: 60.25)
        #expect(zoomed.visibleBeats == 15.5)
        #expect(abs(zoomed.startBeat - 46.566_406_25) < 0.000_001)
    }

    @Test("Preview resolver accepts demo and readable local sources without source mutation")
    func previewSourceResolution() throws {
        #expect(
            TrackAudioPreviewResolver.resolve("lumi-demo://fixture")
                == .syntheticDemo("lumi-demo://fixture")
        )
        #expect(
            TrackAudioPreviewResolver.resolve("https://example.com/audio.mp3")
                == .unavailable("Preview is unavailable for this audio source.")
        )
        let missingPath = "/private/tmp/lumi-missing-preview-file.wav"
        #expect(
            TrackAudioPreviewResolver.resolve(missingPath)
                == .unavailable("The original audio file is missing or unreadable.")
        )
    }

    @MainActor
    @Test("Preview transport seeks by exact bar and loops exact phrase boundaries")
    func previewTransportUsesBeatGrid() throws {
        let analysis = TrackEditorFixtures.ready
        let preview = TrackAudioPreviewController(analysis: analysis)
        defer { preview.shutdown() }

        preview.seek(toMillis: 7_500)
        preview.moveByBar(1)
        #expect(preview.positionMillis == 8_000)
        preview.moveByBar(-1)
        #expect(preview.positionMillis == 6_000)
        let phrase = try #require(analysis.phrases.dropFirst().first)
        preview.seek(toMillis: 10_000)
        #expect(preview.setLoop(phrase))
        #expect(preview.positionMillis == analysis.phraseTimeRange(phrase).lowerBound)

        let invalidEdit = TrackEditorPhrase(
            id: phrase.id,
            startBeat: phrase.startBeat + 1,
            endBeat: phrase.endBeat,
            roleID: phrase.roleID,
            role: phrase.role,
            origin: "user"
        )
        let acceptedPosition = preview.positionMillis
        #expect(!preview.setLoop(invalidEdit))
        #expect(preview.positionMillis == acceptedPosition)

        let acceptedEdit = TrackEditorPhrase(
            id: phrase.id,
            startBeat: 20,
            endBeat: 36,
            roleID: phrase.roleID,
            role: phrase.role,
            origin: "user"
        )
        #expect(preview.setLoop(acceptedEdit))
        #expect(preview.positionMillis == analysis.timeMillis(atBeat: acceptedEdit.startBeat))

        preview.seek(toMillis: 11_000)
        #expect(preview.adoptEditedLoop(acceptedEdit))
        #expect(preview.positionMillis == 11_000)
        #expect(analysis.phraseTimeRange(acceptedEdit).contains(preview.positionMillis))
    }

    @Test("Stale audio completion generations cannot overwrite newer transport")
    func staleAudioSchedulesAreRejected() {
        var generation = TrackAudioScheduleGeneration()
        let first = generation.invalidate()
        let replacement = generation.invalidate()

        #expect(!generation.isCurrent(first))
        #expect(generation.isCurrent(replacement))
    }

    @Test("Edit-during-playback loop adoption has a stable safe transcript")
    func editDuringPlaybackLoopTranscript() {
        let editedLoop: Range<UInt64> = 10_000..<18_000
        let transcript = [
            "inside:\(TrackAudioLoopTransition.position(current: 11_000, loop: editedLoop, preservingPosition: true))",
            "outside:\(TrackAudioLoopTransition.position(current: 19_000, loop: editedLoop, preservingPosition: true))",
            "fresh:\(TrackAudioLoopTransition.position(current: 11_000, loop: editedLoop, preservingPosition: false))"
        ]
        #expect(transcript == ["inside:11000", "outside:10000", "fresh:10000"])
    }

    @MainActor
    @Test("Missing and corrupt audio fail closed while analysis remains available")
    func unavailableAudioDoesNotInvalidateEditorAnalysis() throws {
        let missing = TrackAudioPreviewController(
            analysis: replacingAudioURI(
                in: TrackEditorFixtures.ready,
                with: "/private/tmp/lumi-editor-missing-audio.wav"
            )
        )
        #expect(missing.unavailableReason?.contains("missing or unreadable") == true)
        #expect(missing.positionMillis == 0)

        let corruptURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("lumi-corrupt-\(UUID().uuidString).wav")
        try Data("not audio".utf8).write(to: corruptURL, options: .atomic)
        defer { try? FileManager.default.removeItem(at: corruptURL) }
        let corrupt = TrackAudioPreviewController(
            analysis: replacingAudioURI(in: TrackEditorFixtures.ready, with: corruptURL.path)
        )
        #expect(corrupt.unavailableReason != nil)
        #expect(corrupt.positionMillis == 0)
    }
}

private func replacingAudioURI(
    in analysis: TrackEditorAnalysis,
    with audioURI: String
) -> TrackEditorAnalysis {
    TrackEditorAnalysis(
        track: analysis.track,
        audioURI: audioURI,
        beatsPerBar: analysis.beatsPerBar,
        beats: analysis.beats,
        waveform: analysis.waveform,
        phrases: analysis.phrases,
        roles: analysis.roles,
        sourcePhrases: analysis.sourcePhrases,
        timeline: analysis.timeline
    )
}

private func envelope(
    trackValues: [JSONValue],
    editorValue: JSONValue = .null,
    phraseRoleSettings: JSONValue = .null,
    autoloopCatalog: JSONValue = .null,
    midiIntegration: JSONValue = .null,
    deckInputIntegration: JSONValue = .null
) -> MessageEnvelope {
    MessageEnvelope(
        protocolVersion: 1,
        messageType: .snapshot,
        messageId: "snapshot-library-test",
        sequence: 1,
        correlationId: "test",
        sentAt: "2026-08-03T00:00:00Z",
        payload: [
            "midiIntegration": midiIntegration,
            "deckInputIntegration": deckInputIntegration,
            "library": .object([
                "condition": .string("ready"),
                "providerKind": .string("demo"),
                "source": .object([
                    "id": .string("lumi-demo-library"),
                    "name": .string("Lumi Demo Library"),
                    "revision": .string("demo-library-v1"),
                    "status": .string("current")
                ]),
                "capabilities": .object([
                    "playlists": .boolean(true),
                    "color": .boolean(true),
                    "beatGrid": .boolean(true),
                    "waveform": .boolean(true),
                    "rawPhrases": .boolean(true),
                    "localAudio": .boolean(true)
                ]),
                "collectionTotal": .number(10_000),
                "query": .object([
                    "search": .string(""),
                    "playlistId": .null,
                    "offset": .number(0),
                    "limit": .number(50)
                ]),
                "playlists": .array([
                    .object([
                        "id": .number(1),
                        "sourcePlaylistId": .string("all-demo-tracks"),
                        "name": .string("All Demo Tracks"),
                        "trackCount": .number(10_000)
                    ])
                ]),
                "page": .object([
                    "total": .number(10_000),
                    "offset": .number(0),
                    "tracks": .array(trackValues)
                ]),
                "editor": editorValue,
                "phraseRoleSettings": phraseRoleSettings,
                "autoloopCatalog": autoloopCatalog
            ])
        ]
    )
}

private func autoloopCatalogValue() -> JSONValue {
    let themes: [JSONValue] = ["Electric Bloom", "Deep Ocean", "Solar Flare", "Ultraviolet"]
        .enumerated()
        .map { index, name in
            .object([
                "id": .number(Double(index + 1)),
                "name": .string(name),
                "sortOrder": .number(Double(index + 1))
            ])
        }
    func cells(_ variant: String, missingLast: Bool) -> [JSONValue] {
        (1...4).map { theme in
            let missing = missingLast && theme == 4
            return .object([
                "themeId": .number(Double(theme)),
                "entryId": missing ? .null : .string("theme-\(theme)--synth--\(variant)"),
                "name": missing ? .null : .string("Theme \(theme) Synth \(variant)"),
                "status": .string(missing ? "missing" : "ready")
            ])
        }
    }
    return .object([
        "revision": .number(7),
        "defaultsVersion": .number(1),
        "themes": .array(themes),
        "roles": .array([
            .object([
                "id": .string("synth"),
                "name": .string("Synth"),
                "archived": .boolean(false),
                "variants": .array([
                    .object([
                        "id": .string("variant-1"),
                        "name": .string("Variant 1"),
                        "sortOrder": .number(1),
                        "archived": .boolean(false),
                        "cells": .array(cells("variant-1", missingLast: false))
                    ]),
                    .object([
                        "id": .string("variant-2"),
                        "name": .string("Variant 2"),
                        "sortOrder": .number(2),
                        "archived": .boolean(false),
                        "cells": .array(cells("variant-2", missingLast: true))
                    ])
                ])
            ]),
            .object([
                "id": .string("vocal"),
                "name": .string("Vocal"),
                "archived": .boolean(false),
                "variants": .array([])
            ])
        ]),
        "preflight": .object([
            "status": .string("incomplete"),
            "missingCellCount": .number(1),
            "missingCells": .array([
                .object([
                    "themeId": .number(4),
                    "roleId": .string("synth"),
                    "variantId": .string("variant-2")
                ])
            ]),
            "hasMoreMissingCells": .boolean(false),
            "missingRoleCount": .number(1),
            "missingRoleIds": .array([.string("vocal")]),
            "hasMoreMissingRoles": .boolean(false)
        ]),
        "targetCapabilities": .object([
            "validationOwner": .string("targetAdapter"),
            "hardCodedPhysicalCapacity": .boolean(false)
        ])
    ])
}

private func editorValue() -> JSONValue {
    let markers: [JSONValue] = (0..<8).map { index in
        let marker: [String: JSONValue] = [
            "beatIndex": .number(Double(index)),
            "timeMillis": .number(Double(index * 500)),
            "barIndex": .number(Double(index / 4 + 1)),
            "beatInBar": .number(Double(index % 4 + 1))
        ]
        return .object(marker)
    }
    var editorTrack = trackValue()
    if case var .object(trackObject) = editorTrack {
        trackObject["durationMillis"] = .number(4_000)
        editorTrack = .object(trackObject)
    }
    return .object([
        "track": editorTrack,
        "audioUri": .string("lumi-demo://horizon-lines"),
        "beatGrid": .object([
            "beatsPerBar": .number(4),
            "markers": .array(markers)
        ]),
        "waveform": .array([
            .object(["low": .number(30), "mid": .number(60), "high": .number(90)]),
            .object(["low": .number(90), "mid": .number(60), "high": .number(30)]),
            .object(["low": .number(45), "mid": .number(80), "high": .number(120)])
        ]),
        "phrases": .array([
            .object([
                "id": .number(1),
                "startBeat": .number(0),
                "endBeat": .number(4),
                "roleId": .string("intro-outro"),
                "role": .string("Intro"),
                "origin": .string("sourceImport"),
                "loopStrategy": loopStrategyValue(roleID: "intro-outro")
            ]),
            .object([
                "id": .number(2),
                "startBeat": .number(4),
                "endBeat": .number(8),
                "roleId": .string("buildup-1"),
                "role": .string("Build"),
                "origin": .string("sourceImport"),
                "loopStrategy": loopStrategyValue(roleID: "buildup-1")
            ])
        ]),
        "roles": .array([
            .object(["id": .string("intro-outro"), "name": .string("Intro"), "archived": .boolean(false)]),
            .object(["id": .string("buildup-1"), "name": .string("Build"), "archived": .boolean(false)])
        ]),
        "sourcePhrases": .array([
            .object([
                "startBeat": .number(0),
                "endBeat": .number(4),
                "rawLabel": .string("Intro"),
                "providerKind": .string("demo")
            ]),
            .object([
                "startBeat": .number(4),
                "endBeat": .number(8),
                "rawLabel": .string("Up"),
                "providerKind": .string("demo")
            ])
        ]),
        "timeline": .object([
            "revision": .number(1),
            "baselineRevision": .string("horizon-lines-v1"),
            "origin": .string("sourceImport"),
            "reason": .string("initialSourceMapping"),
            "canUndo": .boolean(false),
            "canRedo": .boolean(false),
            "revisions": .array([
                .object([
                    "revision": .number(1),
                    "origin": .string("sourceImport"),
                    "reason": .string("initialSourceMapping"),
                    "phraseCount": .number(2),
                    "restoredFrom": .null
                ])
            ])
        ])
    ])
}

private func loopStrategyValue(roleID: String) -> JSONValue {
    .object([
        "kind": .string("auto"),
        "locked": .boolean(false),
        "provenance": .string("automaticDefault"),
        "rowRoleId": .string(roleID),
        "fixedVariantId": .null,
        "themeOverrides": .array([]),
        "validatedCatalogRevision": .number(1),
        "status": .string("ready"),
        "issues": .array([])
    ])
}

private func phraseRoleSettingsValue() -> JSONValue {
    .object([
        "revision": .number(4),
        "defaultsVersion": .number(1),
        "roles": .array([
            .object([
                "id": .string("intro-outro"),
                "name": .string("Intro / Outro"),
                "sortOrder": .number(1),
                "archived": .boolean(false),
                "usage": .object([
                    "phraseCount": .number(3),
                    "trackCount": .number(2),
                    "catalogRowCount": .number(0),
                    "affectedTracks": .array([]),
                    "hasMoreAffectedTracks": .boolean(false)
                ])
            ]),
            .object([
                "id": .string("synth"),
                "name": .string("Synth"),
                "sortOrder": .number(2),
                "archived": .boolean(false),
                "usage": .object([
                    "phraseCount": .number(1),
                    "trackCount": .number(1),
                    "catalogRowCount": .number(0),
                    "affectedTracks": .array([
                        .object([
                            "trackId": .number(3),
                            "title": .string("Northern Pulse"),
                            "phraseCount": .number(1)
                        ])
                    ]),
                    "hasMoreAffectedTracks": .boolean(false)
                ])
            ])
        ]),
        "mappingProfiles": .array([
            .object([
                "providerKind": .string("rekordbox7"),
                "providerName": .string("Rekordbox 7"),
                "mappings": .array([
                    .object([
                        "rawLabel": .string("Intro"),
                        "roleId": .string("intro-outro")
                    ]),
                    .object([
                        "rawLabel": .string("Synth"),
                        "roleId": .string("synth")
                    ])
                ])
            ])
        ]),
        "mappingPolicy": .string("futureInitialTimelinesOnly")
    ])
}

private func trackValue() -> JSONValue {
    .object([
        "id": .number(1),
        "sourceTrackId": .string("horizon-lines"),
        "title": .string("Horizon Lines"),
        "artist": .string("Lumi Procedural Audio"),
        "bpmMilli": .number(124_000),
        "key": .object([
            "pitchClass": .string("a"),
            "mode": .string("minor")
        ]),
        "durationMillis": .number(240_000),
        "colorRgb": .number(0x4870CD),
        "analysisRevision": .string("horizon-lines-v1"),
        "timelineRevision": .null,
        "readiness": .object([
            "status": .string("ready"),
            "missingCapabilities": .array([]),
            "warnings": .array([])
        ])
    ])
}
