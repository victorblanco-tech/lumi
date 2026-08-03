import LumiDesignSystem

public enum LibraryWorkspaceFixtures {
    public static let ready = LibraryWorkspaceState(
        condition: .ready,
        providerKind: "demo",
        source: LibrarySource(
            id: "lumi-demo-library",
            name: "Lumi Demo Library",
            revision: "demo-library-v1",
            status: "current"
        ),
        capabilities: LibraryCapabilities(
            playlists: true,
            color: true,
            beatGrid: true,
            waveform: true,
            rawPhrases: true,
            localAudio: true
        ),
        playlists: [
            LibraryPlaylist(
                id: 1,
                sourcePlaylistID: "all-demo-tracks",
                name: "All Demo Tracks",
                trackCount: 3
            ),
            LibraryPlaylist(
                id: 2,
                sourcePlaylistID: "peak-time",
                name: "Peak Time",
                trackCount: 2
            )
        ],
        query: LibraryQuery(search: "", playlistID: nil, offset: 0, limit: 50),
        page: LibraryPage(total: 3, offset: 0, tracks: tracks)
    )

    public static let empty = state(.empty, diagnostic: "No tracks match this search.")
    public static let importing = LibraryWorkspaceState.importing()
    public static let stale = state(
        .stale,
        diagnostic: "Showing the last complete snapshot while the source refreshes."
    )
    public static let degraded = state(
        .degraded,
        tracks: [missingTrack] + Array(tracks.dropFirst()),
        diagnostic: "One track is missing waveform and phrase analysis."
    )
    public static let conflict = state(
        .degraded,
        tracks: [conflictTrack] + Array(tracks.dropFirst()),
        diagnostic: "One source change conflicts with a Lumi timeline."
    )
    public static let error = LibraryWorkspaceState.failed(
        "The demo library could not be opened. Existing show state was not changed."
    )

    private static let tracks = [
        track(
            id: 1,
            sourceID: "horizon-lines",
            title: "Horizon Lines",
            bpm: 124_000,
            key: MusicalKey(pitchClass: .a, mode: .minor),
            color: 0x4870CD
        ),
        track(
            id: 2,
            sourceID: "afterglow-drive",
            title: "Afterglow Drive",
            bpm: 128_000,
            key: MusicalKey(pitchClass: .fSharp, mode: .minor),
            color: 0xBB487E
        ),
        track(
            id: 3,
            sourceID: "northern-pulse",
            title: "Northern Pulse",
            bpm: 138_000,
            key: MusicalKey(pitchClass: .d, mode: .minor),
            color: 0x23A8BE
        )
    ]

    private static let missingTrack = LibraryTrack(
        id: 4,
        sourceTrackID: "partial-analysis",
        title: "Partial Analysis",
        artist: "Lumi Procedural Audio",
        bpmMilli: 126_000,
        musicalKey: MusicalKey(pitchClass: .e, mode: .minor),
        durationMillis: 180_000,
        colorRGB: 0xD49B3B,
        analysisRevision: "partial-v1",
        timelineRevision: nil,
        readiness: .missingAnalysis,
        missingCapabilities: ["Waveform", "Source phrases"],
        warnings: []
    )

    private static let conflictTrack = LibraryTrack(
        id: 5,
        sourceTrackID: "changed-grid",
        title: "Changed Grid",
        artist: "Lumi Procedural Audio",
        bpmMilli: 132_000,
        musicalKey: MusicalKey(pitchClass: .cSharp, mode: .minor),
        durationMillis: 190_000,
        colorRGB: 0xA658D1,
        analysisRevision: "source-v2",
        timelineRevision: 3,
        readiness: .conflict,
        missingCapabilities: [],
        warnings: ["Beatgrid changed after Lumi timeline revision 3"]
    )

    private static func state(
        _ condition: LibraryCondition,
        tracks: [LibraryTrack] = tracks,
        diagnostic: String
    ) -> LibraryWorkspaceState {
        LibraryWorkspaceState(
            condition: condition,
            providerKind: ready.providerKind,
            source: ready.source,
            capabilities: ready.capabilities,
            playlists: ready.playlists,
            query: ready.query,
            page: LibraryPage(
                total: condition == .empty ? 0 : UInt64(tracks.count),
                offset: 0,
                tracks: condition == .empty ? [] : tracks
            ),
            diagnostic: diagnostic
        )
    }

    private static func track(
        id: UInt64,
        sourceID: String,
        title: String,
        bpm: UInt64,
        key: MusicalKey,
        color: UInt32
    ) -> LibraryTrack {
        LibraryTrack(
            id: id,
            sourceTrackID: sourceID,
            title: title,
            artist: "Lumi Procedural Audio",
            bpmMilli: bpm,
            musicalKey: key,
            durationMillis: 240_000,
            colorRGB: color,
            analysisRevision: "\(sourceID)-v1",
            timelineRevision: nil,
            readiness: .ready,
            missingCapabilities: [],
            warnings: []
        )
    }
}
