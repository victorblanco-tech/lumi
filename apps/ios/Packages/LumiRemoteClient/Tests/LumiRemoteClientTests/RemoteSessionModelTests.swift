import Foundation
import LumiProtocol
import Testing

@testable import LumiRemoteClient

@MainActor
@Test
func reconnectDisablesControlsWithoutDiscardingTheLastProjection() throws {
    let model = RemoteSessionModel()
    model.grantControllerLease("lease-1")
    try model.apply(.fixture(revision: 1), from: "MacBook Pro")
    #expect(model.controlsEnabled)
    model.reconnecting(to: "MacBook Pro", at: Date(timeIntervalSince1970: 10))
    #expect(!model.controlsEnabled)
    #expect(model.projection?.projectionRevision == 1)
}

@MainActor
@Test
func staleProjectionCannotRewindTheRemoteView() throws {
    let model = RemoteSessionModel()
    try model.apply(.fixture(revision: 2), from: "MacBook Pro")
    #expect(throws: RemoteContractError.nonIncreasingRevision) {
        try model.apply(.fixture(revision: 1), from: "MacBook Pro")
    }
    #expect(model.projection?.projectionRevision == 2)
}

@MainActor
@Test
func staleTransportAnchorCannotRewindThePlayer() throws {
    let model = RemoteSessionModel()
    try model.apply(.fixture(revision: 1), from: "MacBook Pro")
    try model.applyTransportAnchor(
        playerNumber: 1,
        anchor: .init(
            trackLoadID: 99,
            beat: 16,
            positionMillis: 8_000,
            effectiveBPMMilli: 140_000,
            playing: true,
            discontinuityRevision: 1,
            observedAtUnixMillis: 90
        )
    )
    #expect(model.projection?.players[0].transport.beat == 32)
}

@MainActor
@Test
func leaderTransportAnchorUpdatesTheDisplayedAbletonLinkTempo() throws {
    let model = RemoteSessionModel()
    try model.apply(.fixture(revision: 1), from: "MacBook Pro")
    try model.applyTransportAnchor(
        playerNumber: 1,
        anchor: .init(
            trackLoadID: 99,
            beat: 36,
            positionMillis: 18_000,
            effectiveBPMMilli: 142_500,
            playing: true,
            discontinuityRevision: 1,
            observedAtUnixMillis: 110
        )
    )
    #expect(model.projection?.integrations.abletonLinkBPMMilli == 142_500)
}

@MainActor
@Test
func sourceClockIsTranslatedIntoTheReceivingIPhoneClockDomain() throws {
    let model = RemoteSessionModel()
    let fixture = RemoteLiveProjection.fixture(revision: 1)
    let sourcePlayer = fixture.players[0]
    let sourceTransport = RemoteTransportAnchor(
        trackLoadID: sourcePlayer.trackLoadID,
        beat: sourcePlayer.transport.beat,
        positionMillis: sourcePlayer.transport.positionMillis,
        effectiveBPMMilli: sourcePlayer.transport.effectiveBPMMilli,
        playing: sourcePlayer.transport.playing,
        discontinuityRevision: sourcePlayer.transport.discontinuityRevision,
        observedAtUnixMillis: 9_900,
        publishedAtUnixMillis: 10_000
    )
    let sourceProjection = RemoteLiveProjection(
        projectionRevision: fixture.projectionRevision,
        stateRevision: fixture.stateRevision,
        engineVersion: fixture.engineVersion,
        operationState: fixture.operationState,
        leaderPlayerNumber: fixture.leaderPlayerNumber,
        integrations: fixture.integrations,
        players: [RemotePlayer(
            playerNumber: sourcePlayer.playerNumber,
            hardwareModel: sourcePlayer.hardwareModel,
            trackLoadID: sourcePlayer.trackLoadID,
            transport: sourceTransport,
            track: sourcePlayer.track
        )],
        livePlan: fixture.livePlan,
        nextPlan: fixture.nextPlan,
        themeOptions: fixture.themeOptions
    )

    model.replaceWithSnapshot(
        sourceProjection,
        from: "MacBook Pro",
        receivedAt: Date(timeIntervalSince1970: 1_000)
    )

    #expect(model.projection?.players[0].transport.observedAtUnixMillis == 999_900)
    #expect(model.projection?.players[0].transport.publishedAtUnixMillis == 1_000_000)

    try model.applyTransportAnchor(
        playerNumber: 1,
        anchor: RemoteTransportAnchor(
            trackLoadID: sourcePlayer.trackLoadID,
            beat: 36,
            positionMillis: 18_000,
            effectiveBPMMilli: 140_000,
            playing: true,
            discontinuityRevision: 1,
            observedAtUnixMillis: 10_400,
            publishedAtUnixMillis: 10_450
        ),
        receivedAt: Date(timeIntervalSince1970: 1_001)
    )

    #expect(model.projection?.players[0].transport.observedAtUnixMillis == 1_000_950)
    #expect(model.projection?.players[0].transport.beat == 36)
}

@MainActor
@Test
func frameGapDisablesControlsUntilACompleteSnapshotArrives() throws {
    let model = RemoteSessionModel()
    let processor = RemoteFrameProcessor(model: model, macName: "MacBook Pro")
    model.grantControllerLease("lease-1")
    let initial = try frameData(kind: .snapshot, sequence: 4, projectionRevision: 1)
    #expect(try processor.process(initial) == .applied)
    #expect(model.controlsEnabled)

    let gap = try frameData(kind: .projection, sequence: 6, projectionRevision: 2)
    #expect(
        try processor.process(gap)
            == .snapshotRequired(expected: 5, received: 6)
    )
    #expect(!model.controlsEnabled)

    let ignored = try frameData(kind: .projection, sequence: 7, projectionRevision: 3)
    #expect(try processor.process(ignored) == .unrelated)
    let replacement = try frameData(kind: .snapshot, sequence: 8, projectionRevision: 3)
    #expect(try processor.process(replacement) == .applied)
    // A state gap did not revoke the authenticated Controller lease. The
    // fresh authoritative snapshot restores controls on this same connection.
    #expect(model.controllerLeaseID == "lease-1")
    #expect(model.controlsEnabled)
    #expect(model.projection?.projectionRevision == 3)
}

@MainActor
@Test
func duplicateFrameCannotApplyASecondMutationResult() throws {
    let model = RemoteSessionModel()
    let processor = RemoteFrameProcessor(model: model, macName: "MacBook Pro")
    #expect(
        try processor.process(frameData(kind: .snapshot, sequence: 1, projectionRevision: 1))
            == .applied
    )
    model.markCommandPending("command-1")
    let resultPayload = try JSONDecoder().decode(
        JSONValue.self,
        from: JSONEncoder().encode(
            RemoteCommandResult(
                commandID: "command-1",
                status: .accepted,
                stateRevision: 2,
                planRevision: nil,
                reasonCode: nil
            )
        )
    )
    let result = RemoteFrame(
        frameKind: .commandResult,
        sequence: 2,
        correlationID: "command-1",
        payload: resultPayload
    )
    let data = try JSONEncoder().encode(result)
    #expect(try processor.process(data) == .applied)
    #expect(model.pendingCommandIDs.isEmpty)
    #expect(try processor.process(data) == .duplicateIgnored)
}

@MainActor
@Test
func legacyDuplicateAndUncertainOutcomesRequireFreshStateInsteadOfSuccess() throws {
    for (status, reason) in [(RemoteCommandResultStatus.duplicate, Optional<String>.none),
                              (.rejected, "commandOutcomeUnknown"), (.rejected, "commandOutcomePending")] {
        let model = RemoteSessionModel()
        let processor = RemoteFrameProcessor(model: model, macName: "Mac")
        _ = try processor.process(frameData(kind: .snapshot, sequence: 1, projectionRevision: 1))
        model.markCommandPending("command-1")
        let payload = try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(
            RemoteCommandResult(commandID: "command-1", status: status, stateRevision: nil,
                                planRevision: nil, reasonCode: reason)))
        let frame = RemoteFrame(frameKind: .commandResult, sequence: 2,
                                correlationID: "command-1", payload: payload)
        #expect(try processor.process(JSONEncoder().encode(frame)) == .authoritativeSnapshotRequired)
        #expect(!model.controlsEnabled)
        #expect(model.pendingCommandIDs.isEmpty)
    }
}

@MainActor
@Test
func engineUnavailabilityDisablesStaleControlsAndRecoversOnCompleteProjection() throws {
    let model = RemoteSessionModel()
    let processor = RemoteFrameProcessor(model: model, macName: "Mac")
    model.grantControllerLease("lease-1")
    _ = try processor.process(frameData(kind: .snapshot, sequence: 1, projectionRevision: 1))
    #expect(model.controlsEnabled)
    let error = RemoteFrame(frameKind: .error, sequence: 2,
        payload: .object(["reasonCode": .string("engineUnavailable")]))
    #expect(try processor.process(JSONEncoder().encode(error)) == .applied)
    #expect(!model.controlsEnabled)
    #expect(model.projection?.projectionRevision == 1)
    #expect(model.controllerLeaseID == "lease-1")
    #expect(try processor.process(frameData(kind: .projection, sequence: 3, projectionRevision: 2)) == .applied)
    #expect(model.controlsEnabled)
    #expect(model.projection?.projectionRevision == 2)
}

@MainActor
@Test
func reauthenticationDoesNotEnableControlsBeforeANewSnapshot() throws {
    let model = RemoteSessionModel()
    try model.apply(.fixture(revision: 1), from: "Mac")
    model.grantControllerLease("lease-new")
    model.connected(to: "Mac")
    let processor = RemoteFrameProcessor(model: model, macName: "Mac")
    processor.reset(for: "Mac")
    #expect(!model.controlsEnabled)
    _ = try processor.process(frameData(kind: .snapshot, sequence: 1, projectionRevision: 2))
    #expect(model.controlsEnabled)
}

@MainActor
@Test
func revisionConflictDisablesControlsUntilAnAuthoritativeSnapshotArrives() throws {
    let model = RemoteSessionModel()
    let processor = RemoteFrameProcessor(model: model, macName: "MacBook Pro")
    #expect(
        try processor.process(frameData(kind: .snapshot, sequence: 1, projectionRevision: 1))
            == .applied
    )
    model.grantControllerLease("lease-1")
    model.markCommandPending("command-1")

    let resultPayload = try JSONDecoder().decode(
        JSONValue.self,
        from: JSONEncoder().encode(
            RemoteCommandResult(
                commandID: "command-1",
                status: .conflict,
                stateRevision: 2,
                planRevision: nil,
                reasonCode: "revisionConflict"
            )
        )
    )
    let result = RemoteFrame(
        frameKind: .commandResult,
        sequence: 2,
        correlationID: "command-1",
        payload: resultPayload
    )
    #expect(
        try processor.process(JSONEncoder().encode(result))
            == .authoritativeSnapshotRequired
    )
    #expect(!model.controlsEnabled)
    #expect(model.lastError == "The show changed on the Mac. Refresh before trying again.")

    #expect(
        try processor.process(frameData(kind: .snapshot, sequence: 3, projectionRevision: 2))
            == .applied
    )
    #expect(model.controllerLeaseID == "lease-1")
    #expect(model.controlsEnabled)
}

private func frameData(
    kind: RemoteFrameKind,
    sequence: UInt64,
    projectionRevision: UInt64
) throws -> Data {
    let projection = RemoteLiveProjection.fixture(revision: projectionRevision)
    let value = try JSONDecoder().decode(
        JSONValue.self,
        from: JSONEncoder().encode(projection)
    )
    return try JSONEncoder().encode(
        RemoteFrame(frameKind: kind, sequence: sequence, payload: value)
    )
}

extension RemoteLiveProjection {
    static func fixture(revision: UInt64) -> Self {
        Self(
            projectionRevision: revision,
            stateRevision: revision,
            engineVersion: "0.6.0-dev-4",
            operationState: .armed,
            leaderPlayerNumber: 1,
            integrations: .init(
                proDJLink: .ready,
                lightOutput: .ready,
                abletonLink: .ready,
                abletonLinkEnabled: true,
                abletonLinkBPMMilli: 140_000,
                timingOffsetMillis: -20,
                pendingTimingOffsetMillis: nil
            ),
            players: [.init(
                playerNumber: 1,
                hardwareModel: "CDJ-1500X",
                trackLoadID: 99,
                transport: .init(
                    trackLoadID: 99,
                    beat: 32,
                    positionMillis: 16_000,
                    effectiveBPMMilli: 140_000,
                    playing: true,
                    discontinuityRevision: 1,
                    observedAtUnixMillis: 100
                ),
                track: .init(
                    trackID: 42,
                    title: "Example Track",
                    artist: "Example Artist",
                    originalBPMMilli: 140_000,
                    colorRGB: nil,
                    key: "A minor",
                    durationBeats: 512,
                    beatGrid: nil,
                    waveform: [],
                    hotCues: [],
                    phrases: []
                )
            )],
            livePlan: nil,
            nextPlan: nil,
            themeOptions: []
        )
    }
}
