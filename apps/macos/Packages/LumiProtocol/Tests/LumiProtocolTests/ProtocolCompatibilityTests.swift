import Foundation
import Testing
@testable import LumiProtocol

private struct ContractManifest: Decodable {
    let protocolVersion: Int
    let maxMessageBytes: Int
    let canonicalFixtures: [String]
}

private enum FixtureError: Error {
    case repositoryRootNotFound
}

private func repositoryRoot() throws -> URL {
    var candidate = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    let fileManager = FileManager.default

    while candidate.path != "/" {
        let contract = candidate.appendingPathComponent("contracts/protocol/v1/manifest.json")
        if fileManager.fileExists(atPath: contract.path) {
            return candidate
        }
        candidate.deleteLastPathComponent()
    }

    throw FixtureError.repositoryRootNotFound
}

private func contractDirectory() throws -> URL {
    try repositoryRoot().appendingPathComponent("contracts/protocol/v1")
}

private func loadManifest() throws -> ContractManifest {
    let data = try Data(contentsOf: contractDirectory().appendingPathComponent("manifest.json"))
    return try JSONDecoder().decode(ContractManifest.self, from: data)
}

@Test("Swift decodes every canonical protocol fixture")
func decodesCanonicalFixtures() throws {
    let manifest = try loadManifest()
    #expect(manifest.protocolVersion == WireProtocol.version)
    #expect(manifest.maxMessageBytes == WireProtocol.maximumMessageBytes)

    for fixtureName in manifest.canonicalFixtures {
        let fixture = try contractDirectory()
            .appendingPathComponent("fixtures")
            .appendingPathComponent(fixtureName)
        let envelope = try ProtocolMessageDecoder.decode(Data(contentsOf: fixture))
        #expect(envelope.protocolVersion == WireProtocol.version)
    }
}

@Test("Unknown optional v1 fields remain backward compatible")
func ignoresUnknownOptionalFields() throws {
    let fixture = try contractDirectory()
        .appendingPathComponent("fixtures/event-forward-compatible.json")
    let envelope = try ProtocolMessageDecoder.decode(Data(contentsOf: fixture))

    #expect(envelope.messageType == .event)
}

@Test("A forward event sequence gap requests a full snapshot")
func requestsSnapshotForSequenceGap() {
    var tracker = SequenceTracker()

    #expect(tracker.observe(42) == .accepted)
    #expect(tracker.observe(43) == .accepted)
    #expect(tracker.observe(45) == .requestSnapshot(expected: 44, received: 45))
}

@Test("Duplicate event sequences are ignored")
func ignoresDuplicateSequence() {
    var tracker = SequenceTracker()

    #expect(tracker.observe(42) == .accepted)
    #expect(tracker.observe(42) == .duplicate)
}

@Test("Unsupported protocol versions fail safely")
func rejectsUnsupportedProtocolVersion() throws {
    let input = Data("""
        {
          "protocolVersion": 2,
          "messageType": "event",
          "messageId": "event-1",
          "sequence": 1,
          "correlationId": "interaction-1",
          "sentAt": "2026-08-02T18:00:00Z",
          "payload": {}
        }
        """.utf8)

    #expect(throws: ProtocolDecodingError.unsupportedProtocolVersion(2)) {
        try ProtocolMessageDecoder.decode(input)
    }
}

@Test("Malformed and oversized messages fail safely")
func rejectsInvalidMessages() {
    #expect(throws: ProtocolDecodingError.malformed) {
        try ProtocolMessageDecoder.decode(Data("{".utf8))
    }

    let oversized = Data(repeating: 0x20, count: WireProtocol.maximumMessageBytes + 1)
    #expect(
        throws: ProtocolDecodingError.oversized(
            actual: WireProtocol.maximumMessageBytes + 1,
            maximum: WireProtocol.maximumMessageBytes
        )
    ) {
        try ProtocolMessageDecoder.decode(oversized)
    }
}

@Test("Swift decodes the Rust remote gateway admin wire spelling")
func decodesRemoteGatewayAdminWireSpelling() throws {
    let status = try JSONDecoder().decode(
        RemoteGatewayStatus.self,
        from: Data("""
            {
              "engineConnected": true,
              "installationId": "0123456789abcdef0123456789abcdef",
              "certificateFingerprintSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "lanPort": 49152,
              "pairedDevices": [{
                "deviceId": "iphone-1",
                "displayName": "Victor's iPhone",
                "pairedAtUnixMillis": 100,
                "lastSeenUnixMillis": 200,
                "controller": true
              }],
              "controllerDeviceId": "iphone-1"
            }
            """.utf8)
    )

    #expect(status.installationID == "0123456789abcdef0123456789abcdef")
    #expect(status.certificateFingerprintSHA256.count == 64)
    #expect(status.pairedDevices.first?.deviceID == "iphone-1")
    #expect(status.controllerDeviceID == "iphone-1")

    let invitation = try JSONDecoder().decode(
        RemoteGatewayPairingInvitation.self,
        from: Data("""
            {
              "installationId": "0123456789abcdef0123456789abcdef",
              "invitationId": "invitation-1",
              "invitationSecret": "secret",
              "shortCode": "123456",
              "certificateFingerprintSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              "expiresAtUnixMillis": 300,
              "approved": false
            }
            """.utf8)
    )

    #expect(invitation.invitationID == "invitation-1")
    #expect(invitation.installationID == status.installationID)
    #expect(invitation.certificateFingerprintSHA256.count == 64)
}
