import Testing

@testable import LumiRemoteClient
@testable import LumiRemoteFeature

@Test
func portraitOrderingMovesTheMasterFirstWithoutRenamingPlayers() {
    let projection = RemoteLiveProjection(
        projectionRevision: 1,
        stateRevision: 1,
        engineVersion: "0.6.0-dev-4",
        operationState: .armed,
        leaderPlayerNumber: 2,
        integrations: RemoteIntegrationStatus(
            proDJLink: .ready,
            lightOutput: .ready,
            abletonLink: .ready,
            abletonLinkEnabled: true,
            abletonLinkBPMMilli: 140_000,
            timingOffsetMillis: -20,
            pendingTimingOffsetMillis: nil
        ),
        players: [player(1), player(2)],
        livePlan: nil,
        nextPlan: nil,
        themeOptions: []
    )

    #expect(RemotePlayerOrdering.orderedPlayers(in: projection).map(\.playerNumber) == [2, 1])
}

@Test
func completedPinchAppliesExactlyOnceAndStaysInsideTheTrack() {
    #expect(
        RemoteWaveformViewportMath.committedVisibleBars(
            baseVisibleBars: 40,
            magnification: 2,
            totalBars: 128
        ) == 20
    )
    #expect(
        RemoteWaveformViewportMath.committedVisibleBars(
            baseVisibleBars: 40,
            magnification: 100,
            totalBars: 128
        ) == 2
    )
    #expect(
        RemoteWaveformViewportMath.committedVisibleBars(
            baseVisibleBars: 40,
            magnification: 0.1,
            totalBars: 128
        ) == 128
    )
}

@Test
func playingTransportInterpolatesSmoothlyButNeverRunsAwayWhenStale() {
    let player = RemotePlayer(
        playerNumber: 1,
        hardwareModel: "CDJ-1500X",
        trackLoadID: 8,
        transport: RemoteTransportAnchor(
            trackLoadID: 8,
            beat: 0,
            positionMillis: 250,
            effectiveBPMMilli: 120_000,
            playing: true,
            discontinuityRevision: 1,
            observedAtUnixMillis: 1_000
        ),
        track: RemoteTrack(
            trackID: 1,
            title: "Fixture",
            artist: "Lumi",
            originalBPMMilli: 120_000,
            colorRGB: nil,
            key: "",
            durationBeats: 4,
            beatGrid: RemoteBeatGrid(
                beatsPerBar: 4,
                durationMillis: 2_000,
                timesMillis: [0, 500, 1_000, 1_500]
            ),
            waveform: [],
            hotCues: [],
            phrases: []
        )
    )

    #expect(RemoteTransportInterpolation.visualBeat(player: player, atUnixMillis: 1_250) == 1)
    #expect(RemoteTransportInterpolation.visualBeat(player: player, atUnixMillis: 5_000) == 2)
}

private func player(_ number: UInt8) -> RemotePlayer {
    RemotePlayer(
        playerNumber: number,
        hardwareModel: "CDJ-1500X",
        trackLoadID: UInt64(number),
        transport: RemoteTransportAnchor(
            trackLoadID: UInt64(number),
            beat: 0,
            positionMillis: 0,
            effectiveBPMMilli: 140_000,
            playing: false,
            discontinuityRevision: 0,
            observedAtUnixMillis: 1
        ),
        track: RemoteTrack(
            trackID: UInt64(number),
            title: "Track \(number)",
            artist: "Artist",
            originalBPMMilli: 140_000,
            colorRGB: nil,
            key: "A minor",
            durationBeats: 512,
            beatGrid: nil,
            waveform: [],
            hotCues: [],
            phrases: []
        )
    )
}
