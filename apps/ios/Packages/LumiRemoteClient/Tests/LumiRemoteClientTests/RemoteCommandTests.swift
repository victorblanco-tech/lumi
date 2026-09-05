import Foundation
import Network
import Testing

@testable import LumiRemoteClient

@Test("Remote waveform RGB points use the compact lossless wire form")
func compactRemoteWaveformPointRoundTripsAndReadsLegacyObjects() throws {
    let packed = try JSONDecoder().decode(
        RemoteWaveformPoint.self,
        from: Data(#""ff6004""#.utf8)
    )
    #expect(packed.low == 255)
    #expect(packed.mid == 96)
    #expect(packed.high == 4)
    #expect(String(decoding: try JSONEncoder().encode(packed), as: UTF8.self) == #""ff6004""#)

    let legacy = try JSONDecoder().decode(
        RemoteWaveformPoint.self,
        from: Data(#"{"low":255,"mid":96,"high":4}"#.utf8)
    )
    #expect(legacy == packed)
}

@MainActor
@Test
func commandEncodingMatchesTheRustTaggedAllowlist() throws {
    let coordinator = RemoteCommandCoordinator(controllerLeaseID: "lease-1")
    let projection = try fixtureProjection()
    let command = try coordinator.makeStateCommand(
        { .setOperationState(.live, expectedStateRevision: $0) },
        projection: projection,
        target: "operation",
        commandID: "command-1",
        now: Date(timeIntervalSince1970: 1)
    )
    let object = try #require(
        JSONSerialization.jsonObject(with: JSONEncoder().encode(command)) as? [String: Any]
    )
    let payload = try #require(object["command"] as? [String: Any])
    #expect(payload["kind"] as? String == "setOperationState")
    #expect(payload["operationState"] as? String == "live")
    #expect(payload["expectedStateRevision"] as? UInt64 == 7)
}

@Test
func authenticationHelloMatchesTheRustTaggedContract() throws {
    let encoded = try JSONEncoder().encode(
        RemoteClientHello.pair(
            invitationID: "invitation-123456",
            invitationSecret: String(repeating: "i", count: 32),
            deviceID: "iphone-1",
            displayName: "Test iPhone",
            deviceCredential: String(repeating: "c", count: 32)
        )
    )
    let object = try #require(
        JSONSerialization.jsonObject(with: encoded) as? [String: String]
    )
    #expect(object["kind"] == "pair")
    #expect(object["invitationId"] == "invitation-123456")
    #expect(object["deviceId"] == "iphone-1")
    #expect(object["deviceCredential"] == String(repeating: "c", count: 32))
    #expect(object["clientVersion"] == RemoteAppVersion.current)

    let response = Data(#"{"kind":"authenticated","installationId":"install-1","controllerLeaseId":"lease-1"}"#.utf8)
    #expect(
        try JSONDecoder().decode(RemoteServerHello.self, from: response)
            == .authenticated(installationID: "install-1", controllerLeaseID: "lease-1")
    )
}

@Test
func observerAuthenticationIncludesTheControllingDeviceWithoutGrantingALease() throws {
    let data = Data(#"{"kind":"authenticated","installationId":"mac","controllerLeaseId":null,"controllerDisplayName":"aiVoon"}"#.utf8)
    let hello = try JSONDecoder().decode(RemoteServerHello.self, from: data)
    #expect(hello.controllerLeaseID == nil)
    #expect(hello.controllerDisplayName == "aiVoon")
}

@MainActor
@Test
func scannedPairingInvitationReplacesAStaleStoredCredential() throws {
    let stored = RemoteDeviceCredential(
        installationID: "installation-123",
        deviceID: "stale-device",
        credential: String(repeating: "c", count: 32),
        certificateFingerprintSHA256: String(repeating: "a", count: 64),
        releaseChannel: .dev
    )
    let invitation = RemotePairingInvitation(
        installationID: "installation-123",
        invitationID: "invitation-123456",
        invitationSecret: String(repeating: "i", count: 32),
        shortCode: "123456",
        certificateFingerprintSHA256: String(repeating: "b", count: 64),
        expiresAtUnixMillis: 2_000
    )
    let hello = try #require(
        RemoteConnectionController.authenticationHello(
            stored: stored,
            pairingInvitation: invitation,
            pairingDeviceID: "replacement-device",
            pairingCredential: String(repeating: "n", count: 32),
            displayName: "Test iPhone"
        )
    )
    let object = try #require(
        JSONSerialization.jsonObject(with: JSONEncoder().encode(hello)) as? [String: String]
    )

    #expect(object["kind"] == "pair")
    #expect(object["invitationId"] == invitation.invitationID)
    #expect(object["deviceId"] == "replacement-device")
    #expect(object["deviceId"] != stored.deviceID)
}

@MainActor
@Test
func scannedPairingInvitationBypassesUnreadableKeychainState() async throws {
    let credential = try await RemoteConnectionController.storedCredential(
        for: "installation-123",
        pairingInProgress: true,
        store: RejectingCredentialStore()
    )
    #expect(credential == nil)
}

private actor RejectingCredentialStore: RemoteCredentialStore {
    func credential(for installationID: String) throws -> RemoteDeviceCredential? {
        throw RemoteCredentialStoreError.invalidStoredCredential
    }

    func save(_ credential: RemoteDeviceCredential) throws {}
    func remove(installationID: String) throws {}
}

private actor EmptyCredentialStore: RemoteCredentialStore {
    func credential(for installationID: String) -> RemoteDeviceCredential? { nil }
    func save(_ credential: RemoteDeviceCredential) {}
    func remove(installationID: String) {}
}

@MainActor
@Test
func unchangedBonjourResultsDoNotRestartTheRemoteConnection() throws {
    let model = RemoteSessionModel()
    let controller = RemoteConnectionController(
        model: model,
        releaseChannel: .dev,
        credentialStore: EmptyCredentialStore()
    )
    let first = try discoveredService(installationID: "installation-1", port: 61_001)
    let replacement = try discoveredService(installationID: "installation-1", port: 61_002)

    controller.update(discoveredServices: [first])
    let firstGeneration = controller.connectionGenerationForTesting
    controller.update(discoveredServices: [first])

    #expect(firstGeneration == 1)
    #expect(controller.connectionGenerationForTesting == firstGeneration)

    controller.update(discoveredServices: [replacement])
    #expect(controller.connectionGenerationForTesting == firstGeneration + 1)

    controller.stop()
    #expect(controller.connectionGenerationForTesting == firstGeneration + 2)
}

private func discoveredService(
    installationID: String,
    port: UInt16
) throws -> RemoteDiscoveredService {
    let endpointPort = try #require(NWEndpoint.Port(rawValue: port))
    return RemoteDiscoveredService(
        identity: RemoteServiceIdentity(
            name: "Lumi Mac",
            installationID: installationID,
            protocolVersion: lumiRemoteProtocolVersion,
            releaseChannel: .dev
        ),
        endpoint: .hostPort(host: "127.0.0.1", port: endpointPort)
    )
}

@MainActor
@Test
func localCommandFailureKeepsTheAuthenticatedSessionConnected() throws {
    let model = RemoteSessionModel()
    model.connected(to: "Booth Mac")
    model.grantControllerLease("lease-1")
    let before = model.rejectedCommandFeedbackRevision

    model.reportError("The timing offset is outside the supported range.")

    #expect(model.connectionPhase == .connected(macName: "Booth Mac"))
    #expect(model.controllerLeaseID == "lease-1")
    #expect(model.rejectedCommandFeedbackRevision == before + 1)
}

@Test func discoveryMetadataRejectsWrongReleaseAndProtocol() throws {
    let valid = try RemoteDiscoveryMetadata.identity(
        serviceName: "Booth Mac",
        textRecord: ["id": "installation-123", "pv": "1", "channel": "dev"],
        expectedChannel: .dev
    )
    #expect(valid.installationID == "installation-123")
    #expect(throws: RemoteTrustError.releaseChannelMismatch) {
        try RemoteDiscoveryMetadata.identity(
            serviceName: "Booth Mac",
            textRecord: ["id": "installation-123", "pv": "1", "channel": "production"],
            expectedChannel: .dev
        )
    }
    #expect(throws: RemoteTrustError.protocolMismatch) {
        try RemoteDiscoveryMetadata.identity(
            serviceName: "Booth Mac",
            textRecord: ["id": "installation-123", "pv": "9", "channel": "dev"],
            expectedChannel: .dev
        )
    }
}

@Test func discoveryMetadataAcceptsOnlyAUsableAdvertisedPort() {
    #expect(RemoteDiscoveryMetadata.advertisedPort(textRecord: ["port": "60755"]) == 60_755)
    #expect(RemoteDiscoveryMetadata.advertisedPort(textRecord: ["port": "0"]) == nil)
    #expect(RemoteDiscoveryMetadata.advertisedPort(textRecord: ["port": "70000"]) == nil)
    #expect(RemoteDiscoveryMetadata.advertisedPort(textRecord: [:]) == nil)
}

@Test func pairingCodeRoundTripsAndRejectsExpiredInvitation() throws {
    let invitation = RemotePairingInvitation(
        installationID: "installation-123",
        invitationID: "invitation-123456",
        invitationSecret: String(repeating: "s", count: 32),
        shortCode: "123456",
        certificateFingerprintSHA256: String(repeating: "a", count: 64),
        expiresAtUnixMillis: 2_000
    )
    let codec = RemotePairingCodeCodec()
    let url = try codec.encode(invitation)
    #expect(try codec.decode(url, nowUnixMillis: 1_000) == invitation)
    #expect(throws: RemoteTrustError.invitationExpired) {
        try codec.decode(url, nowUnixMillis: 2_000)
    }
}

@Test func credentialsAreBoundToInstallationAndReleaseChannel() throws {
    let credential = RemoteDeviceCredential(
        installationID: "installation-123",
        deviceID: "iphone-123",
        credential: String(repeating: "c", count: 32),
        certificateFingerprintSHA256: String(repeating: "b", count: 64),
        releaseChannel: .dev
    )
    try credential.validate(for: "installation-123", expectedChannel: .dev)
    #expect(throws: RemoteTrustError.credentialScopeMismatch) {
        try credential.validate(for: "other-installation", expectedChannel: .dev)
    }
    #expect(throws: RemoteTrustError.credentialScopeMismatch) {
        try credential.validate(for: "installation-123", expectedChannel: .production)
    }
}

@Test func sharedRemoteFixturesDecodeInTheSwiftClient() throws {
    let decoder = RemoteFrameDecoder()
    let snapshotFrame = try decoder.decodeFrame(
        Data(contentsOf: remoteFixture("snapshot-live.json"))
    )
    let projection = try decoder.decodeProjection(snapshotFrame)
    #expect(projection.players.first?.hardwareModel == "CDJ-1500X")
    #expect(projection.players.first?.track.phrases.first?.colorRGB == 0xFF_00_00)
    #expect(projection.livePlan?.cues.last?.staticLookName == "Moving Heads OFF")
    #expect(projection.phraseRoleOptions.contains(where: { $0.id == "buildup-1" }))

    let commandFrame = try decoder.decodeFrame(
        Data(contentsOf: remoteFixture("command-autoloop.json"))
    )
    let commandData = try JSONEncoder().encode(commandFrame.payload)
    let command = try JSONDecoder().decode(RemoteCommand.self, from: commandData)
    #expect(command.commandID == "command-123")
    #expect(
        command.command == .selectAutoloopForPhrase(
            .init(
                planID: "plan-99",
                trackLoadID: 99,
                expectedPlanRevision: 4,
                phraseIndex: 1
            ),
            autoloopNumber: 17
        )
    )

    let resultFrame = try decoder.decodeFrame(
        Data(contentsOf: remoteFixture("command-result-conflict.json"))
    )
    #expect(try decoder.decodeCommandResult(resultFrame).status == .conflict)
}

@Test func phraseTypeCommandRoundTripsWithTheSelectedRoleIdentity() throws {
    let payload = RemoteCommandPayload.changePhraseRole(
        .init(
            planID: "plan-99",
            trackLoadID: 99,
            expectedPlanRevision: 4,
            phraseIndex: 2
        ),
        roleID: "buildup-2"
    )
    let encoded = try JSONEncoder().encode(payload)
    let decoded = try JSONDecoder().decode(RemoteCommandPayload.self, from: encoded)
    #expect(decoded == payload)

    let json = try #require(
        try JSONSerialization.jsonObject(with: encoded) as? [String: Any]
    )
    #expect(json["kind"] as? String == "changePhraseRole")
    #expect(json["roleId"] as? String == "buildup-2")
}

@Test func olderProjectionWithoutPhraseRoleOptionsRemainsReadable() throws {
    let projection = try fixtureProjection()
    #expect(projection.phraseRoleOptions.isEmpty)
}

private func remoteFixture(_ name: String) -> URL {
    var repository = URL(fileURLWithPath: #filePath)
    for _ in 0 ..< 7 {
        repository.deleteLastPathComponent()
    }
    return repository
        .appendingPathComponent("contracts/remote/v1/fixtures")
        .appendingPathComponent(name)
}

@MainActor
@Test
func repeatedTapCannotCreateASecondPendingMutation() throws {
    let coordinator = RemoteCommandCoordinator(controllerLeaseID: "lease-1")
    let projection = try fixtureProjection()
    _ = try coordinator.makeStateCommand(
        { .setAbletonLinkEnabled(true, expectedStateRevision: $0) },
        projection: projection,
        target: "ableton-link",
        commandID: "command-1"
    )
    #expect(throws: RemoteCommandBuildError.duplicatePendingTarget) {
        try coordinator.makeStateCommand(
            { .setAbletonLinkEnabled(true, expectedStateRevision: $0) },
            projection: projection,
            target: "ableton-link",
            commandID: "command-2"
        )
    }
}

@MainActor
@Test
func currentPhraseCannotBeMutated() throws {
    let coordinator = RemoteCommandCoordinator(controllerLeaseID: "lease-1")
    let projection = try fixtureProjection()
    let player = try #require(projection.players.first)
    let plan = try #require(projection.livePlan)
    let cue = try #require(plan.cues.first)
    #expect(throws: RemoteCommandBuildError.phraseAlreadyStarted) {
        try coordinator.makePlanCommand(
            plan: plan,
            cue: cue,
            player: player,
            payload: { .setCueLock($0, locked: true) },
            target: "plan-cue"
        )
    }
}

@Test
func pairingInvitationRejectsAnExpiredOrInvalidFingerprint() {
    let expired = RemotePairingInvitation(
        installationID: "install-12345678",
        invitationID: "invitation-123456",
        invitationSecret: String(repeating: "s", count: 32),
        shortCode: "123456",
        certificateFingerprintSHA256: String(repeating: "a", count: 64),
        expiresAtUnixMillis: 99
    )
    #expect(throws: RemoteTrustError.invitationExpired) {
        try expired.validate(nowUnixMillis: 100)
    }

    let invalidFingerprint = RemotePairingInvitation(
        installationID: "install-12345678",
        invitationID: "invitation-123456",
        invitationSecret: String(repeating: "s", count: 32),
        shortCode: "123456",
        certificateFingerprintSHA256: String(repeating: "z", count: 64),
        expiresAtUnixMillis: 101
    )
    #expect(throws: RemoteTrustError.invalidCertificateFingerprint) {
        try invalidFingerprint.validate(nowUnixMillis: 100)
    }
}

private func fixtureProjection() throws -> RemoteLiveProjection {
    let json = #"""
    {
      "projectionRevision": 8,
      "stateRevision": 7,
      "engineVersion": "0.6.0-dev-4",
      "operationState": "armed",
      "leaderPlayerNumber": 1,
      "integrations": {
        "proDjLink": "ready", "lightOutput": "ready", "abletonLink": "ready",
        "abletonLinkEnabled": true, "abletonLinkBpmMilli": 140000,
        "timingOffsetMillis": -20, "pendingTimingOffsetMillis": null
      },
      "players": [{
        "playerNumber": 1, "hardwareModel": "CDJ-1500X", "trackLoadId": 9,
        "transport": {
          "trackLoadId": 9, "beat": 48, "positionMillis": 1000,
          "effectiveBpmMilli": 140000, "playing": true,
          "discontinuityRevision": 1, "observedAtUnixMillis": 1
        },
        "track": {
          "trackId": 1, "title": "Example Track", "artist": "Example Artist",
          "originalBpmMilli": 140000, "colorRgb": null, "key": "A minor",
          "durationBeats": 512, "beatGrid": null, "waveform": [],
          "hotCues": [], "phrases": []
        }
      }],
      "livePlan": {
        "planId": "plan-1", "playerNumber": 1, "trackLoadId": 9,
        "revision": 3, "themeId": 2, "themeName": "Blue Pink",
        "cues": [{
          "phraseIndex": 0, "startBeat": 32, "endBeat": 64, "locked": false,
          "themeId": 2, "themeName": "Blue Pink", "autoloopNumber": 1,
          "autoloopName": "Intro", "staticLookName": null,
          "availableAutoloops": []
        }]
      },
      "nextPlan": null,
      "themeOptions": []
    }
    """#
    return try JSONDecoder().decode(RemoteLiveProjection.self, from: Data(json.utf8))
}
