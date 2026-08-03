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
        #expect(editor.phraseTimeRange(editor.phrases[1]) == 2_000..<4_000)
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

    @Test("Every viewport scale preserves complete bars and invertible beat positions")
    func trackEditorViewportAlignment() {
        for bars: UInt32 in [1, 2, 4, 8, 16, 32] {
            let viewport = TrackEditorViewport(
                startBar: 13,
                visibleBars: bars,
                totalBars: 64,
                beatsPerBar: 4
            )
            #expect(viewport.visibleBeats.isMultiple(of: 4))
            #expect(viewport.startBeat.isMultiple(of: 4))
            for beat in stride(
                from: Double(viewport.startBeat),
                through: Double(viewport.endBeat),
                by: 0.5
            ) {
                let x = viewport.x(forBeat: beat, width: 1_024)
                #expect(abs(viewport.beat(atX: x, width: 1_024) - beat) < 0.000_001)
            }
        }
    }

    @Test("Viewport movement and zoom clamp to whole track bars")
    func trackEditorViewportClamping() {
        let initial = TrackEditorViewport(startBar: 14, visibleBars: 8, totalBars: 16, beatsPerBar: 4)
        #expect(initial.startBar == 8)
        #expect(initial.moving(byBars: -100).startBar == 0)
        #expect(initial.moving(byBars: 100).startBar == 8)
        let zoomed = initial.zoomed(to: 4, aroundBar: 15)
        #expect(zoomed.visibleBars == 4)
        #expect(zoomed.startBar == 12)
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
            role: phrase.role,
            origin: "user"
        )
        #expect(preview.setLoop(acceptedEdit))
        #expect(preview.positionMillis == analysis.timeMillis(atBeat: acceptedEdit.startBeat))
    }

    @Test("Stale audio completion generations cannot overwrite newer transport")
    func staleAudioSchedulesAreRejected() {
        var generation = TrackAudioScheduleGeneration()
        let first = generation.invalidate()
        let replacement = generation.invalidate()

        #expect(!generation.isCurrent(first))
        #expect(generation.isCurrent(replacement))
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
        phrases: analysis.phrases
    )
}

private func envelope(trackValues: [JSONValue], editorValue: JSONValue = .null) -> MessageEnvelope {
    MessageEnvelope(
        protocolVersion: 1,
        messageType: .snapshot,
        messageId: "snapshot-library-test",
        sequence: 1,
        correlationId: "test",
        sentAt: "2026-08-03T00:00:00Z",
        payload: [
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
                "editor": editorValue
            ])
        ]
    )
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
                "role": .string("Intro"),
                "origin": .string("source")
            ]),
            .object([
                "id": .number(2),
                "startBeat": .number(4),
                "endBeat": .number(8),
                "role": .string("Build"),
                "origin": .string("source")
            ])
        ])
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
