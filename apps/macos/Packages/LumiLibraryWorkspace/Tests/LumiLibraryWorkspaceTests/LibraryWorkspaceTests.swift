import LumiLibraryWorkspace
import LumiProtocol
import Testing

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
    }
}

private func envelope(trackValues: [JSONValue]) -> MessageEnvelope {
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
                ])
            ])
        ]
    )
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
