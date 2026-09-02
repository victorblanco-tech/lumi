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
    #expect(
        RemotePlayerOrdering.visibleSlots(in: projection, isLandscape: false)
            .map(\.playerNumber) == [2, 1]
    )
    #expect(
        RemotePlayerOrdering.visibleSlots(in: projection, isLandscape: true)
            .map(\.playerNumber) == [1, 2]
    )
}

@Test
func aSingleLoadedPlayerKeepsTwoFixedNumberedSlots() {
    let projection = projection(players: [player(1)], leaderPlayerNumber: 1)

    let portrait = RemotePlayerOrdering.visibleSlots(in: projection, isLandscape: false)
    let landscape = RemotePlayerOrdering.visibleSlots(in: projection, isLandscape: true)

    #expect(portrait.map(\.playerNumber) == [1, 2])
    #expect(portrait.map { $0.player != nil } == [true, false])
    #expect(landscape.map(\.playerNumber) == [1, 2])
    #expect(landscape.map { $0.player != nil } == [true, false])
}

@Test
func liveAndNextPlansSelectTheCorrectNumberedPlayersFromAFourPlayerNetwork() {
    let livePlan = RemoteLightPlan(
        planID: "live-3",
        playerNumber: 3,
        trackLoadID: 3,
        revision: 1,
        themeID: nil,
        themeName: nil,
        cues: []
    )
    let nextPlan = RemoteLightPlan(
        planID: "next-1",
        playerNumber: 1,
        trackLoadID: 1,
        revision: 1,
        themeID: nil,
        themeName: nil,
        cues: []
    )
    let base = projection(
        players: [player(4), player(2), player(1), player(3)],
        leaderPlayerNumber: 3
    )
    let projection = RemoteLiveProjection(
        projectionRevision: base.projectionRevision,
        stateRevision: base.stateRevision,
        engineVersion: base.engineVersion,
        operationState: base.operationState,
        leaderPlayerNumber: base.leaderPlayerNumber,
        integrations: base.integrations,
        players: base.players,
        livePlan: livePlan,
        nextPlan: nextPlan,
        themeOptions: []
    )

    #expect(
        RemotePlayerOrdering.visibleSlots(in: projection, isLandscape: false)
            .map(\.playerNumber) == [3, 1]
    )
    #expect(
        RemotePlayerOrdering.visibleSlots(in: projection, isLandscape: true)
            .map(\.playerNumber) == [1, 3]
    )
}

@Test
func remoteWaveformUsesTheSharedRekordboxRGBChannelOrderAndCurve() throws {
    let sample = try #require(
        RemoteWaveformSampling.sample(
            points: [
                RemoteWaveformPoint(low: 0, mid: 0, high: 255),
                RemoteWaveformPoint(low: 255, mid: 0, high: 0),
            ],
            trackProgress: 0
        )
    )
    let midpoint = try #require(
        RemoteWaveformSampling.sample(
            points: [
                RemoteWaveformPoint(low: 0, mid: 0, high: 255),
                RemoteWaveformPoint(low: 255, mid: 0, high: 0),
            ],
            trackProgress: 0.5
        )
    )

    #expect(sample.red == 1)
    #expect(sample.green == 0)
    #expect(sample.blue == 0)
    #expect(midpoint.red > 0.9)
    #expect(midpoint.green > 0.9)
    #expect(midpoint.blue == 0)
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
func liveViewportKeepsOneZoomLevelAndFixedPlayheadAtTrackBoundaries() {
    let portraitBars = RemoteWaveformViewportMath.automaticVisibleBars(
        isMaster: true,
        totalBars: 188
    )
    let landscapeBars = RemoteWaveformViewportMath.automaticVisibleBars(
        isMaster: true,
        totalBars: 188
    )
    #expect(portraitBars == 40)
    #expect(landscapeBars == portraitBars)

    let visibleBeats = portraitBars * 4
    for beat in [0.0, 4.0, 751.0, 752.0] {
        let start = RemoteWaveformViewportMath.automaticStartBeat(
            currentBeat: beat,
            visibleBeats: visibleBeats,
            totalBeats: 752,
            isMaster: true
        )
        #expect(abs((beat - start) / visibleBeats - 0.22) < 0.000_1)
    }
}

@Test
func waveformSamplingLeavesOutOfTrackLeadSpaceEmpty() {
    let points = [RemoteWaveformPoint(low: 0, mid: 0, high: 255)]

    #expect(RemoteWaveformSampling.sample(points: points, trackProgress: -0.01) == nil)
    #expect(RemoteWaveformSampling.sample(points: points, trackProgress: 1.01) == nil)
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

@Test
func onlyAnUpcomingPhraseCanBeAdjustedByTheController() {
    let cue = RemotePlanCue(
        phraseIndex: 3,
        startBeat: 64,
        endBeat: 96,
        locked: false,
        themeID: 1,
        themeName: "Blue Pink",
        autoloopNumber: 7,
        autoloopName: "Intro Blue Pink",
        staticLookName: nil,
        availableAutoloops: []
    )

    #expect(RemotePlanCueEditing.phase(cue: cue, currentBeat: nil) == .unavailable)
    #expect(RemotePlanCueEditing.phase(cue: cue, currentBeat: 32) == .planned)
    #expect(RemotePlanCueEditing.phase(cue: cue, currentBeat: 64) == .live)
    #expect(RemotePlanCueEditing.phase(cue: cue, currentBeat: 96) == .completed)
    #expect(
        RemotePlanCueEditing.canEdit(
            cue: cue,
            currentBeat: 32,
            controlsEnabled: true
        )
    )
    #expect(
        !RemotePlanCueEditing.canEdit(
            cue: cue,
            currentBeat: 32,
            controlsEnabled: false
        )
    )
    #expect(
        !RemotePlanCueEditing.canEdit(
            cue: cue,
            currentBeat: 64,
            controlsEnabled: true
        )
    )
}

@Test
func livePlanPresentationMarksExactlyTheActiveAndNextPhrases() {
    let cues = [
        planCue(index: 0, startBeat: 0, endBeat: 32),
        planCue(index: 1, startBeat: 32, endBeat: 64),
        planCue(index: 2, startBeat: 64, endBeat: 96),
        planCue(index: 3, startBeat: 96, endBeat: 128),
    ]

    #expect(
        RemotePlanCuePresentation.status(
            for: cues[0],
            in: cues,
            currentBeat: 47.5
        ) == .completed
    )
    #expect(
        RemotePlanCuePresentation.status(
            for: cues[1],
            in: cues,
            currentBeat: 47.5
        ) == .active
    )
    #expect(
        RemotePlanCuePresentation.status(
            for: cues[2],
            in: cues,
            currentBeat: 47.5
        ) == .next
    )
    #expect(
        RemotePlanCuePresentation.status(
            for: cues[3],
            in: cues,
            currentBeat: 47.5
        ) == .planned
    )
}

@Test
func nextPhrasePresentationMovesAtTheExactPhraseBoundary() {
    let cues = [
        planCue(index: 0, startBeat: 0, endBeat: 32),
        planCue(index: 1, startBeat: 32, endBeat: 64),
        planCue(index: 2, startBeat: 64, endBeat: 96),
    ]

    #expect(
        RemotePlanCuePresentation.status(
            for: cues[0],
            in: cues,
            currentBeat: 32
        ) == .completed
    )
    #expect(
        RemotePlanCuePresentation.status(
            for: cues[1],
            in: cues,
            currentBeat: 32
        ) == .active
    )
    #expect(
        RemotePlanCuePresentation.status(
            for: cues[2],
            in: cues,
            currentBeat: 32
        ) == .next
    )
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

private func planCue(
    index: UInt16,
    startBeat: UInt64,
    endBeat: UInt64
) -> RemotePlanCue {
    RemotePlanCue(
        phraseIndex: index,
        startBeat: startBeat,
        endBeat: endBeat,
        locked: false,
        themeID: 1,
        themeName: "Blue Pink",
        autoloopNumber: UInt8(index + 1),
        autoloopName: "AutoLoop \(index + 1)",
        staticLookName: nil,
        availableAutoloops: []
    )
}

private func projection(
    players: [RemotePlayer],
    leaderPlayerNumber: UInt8?
) -> RemoteLiveProjection {
    RemoteLiveProjection(
        projectionRevision: 1,
        stateRevision: 1,
        engineVersion: "0.6.0-dev-7",
        operationState: .armed,
        leaderPlayerNumber: leaderPlayerNumber,
        integrations: RemoteIntegrationStatus(
            proDJLink: .ready,
            lightOutput: .ready,
            abletonLink: .ready,
            abletonLinkEnabled: true,
            abletonLinkBPMMilli: 140_000,
            timingOffsetMillis: -20,
            pendingTimingOffsetMillis: nil
        ),
        players: players,
        livePlan: nil,
        nextPlan: nil,
        themeOptions: []
    )
}
