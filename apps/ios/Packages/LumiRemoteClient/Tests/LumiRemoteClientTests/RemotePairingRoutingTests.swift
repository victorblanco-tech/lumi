import Foundation
import LumiProtocol
import Testing
@testable import LumiRemoteClient

@Test func pairingLinksAreExclusiveToTheirReleaseChannel() throws {
    let invitation = RemotePairingInvitation(
        installationID: "installation-123",
        invitationID: "invitation-123456",
        invitationSecret: String(repeating: "s", count: 32),
        shortCode: "123456",
        certificateFingerprintSHA256: String(repeating: "a", count: 64),
        expiresAtUnixMillis: 2_000
    )
    let channels: [RemoteReleaseChannel] = [.production, .dev, .rc]
    for source in channels {
        let url = try RemotePairingCodeCodec(releaseChannel: source).encode(invitation)
        #expect(url.scheme == RemotePairingRoute.scheme(for: source.rawValue))
        for target in channels {
            let codec = RemotePairingCodeCodec(releaseChannel: target)
            if target == source {
                #expect(try codec.decode(url, nowUnixMillis: 1_000) == invitation)
                #expect(throws: RemoteTrustError.invitationExpired) {
                    try codec.decode(url, nowUnixMillis: 2_000)
                }
            } else {
                #expect(throws: RemotePairingCodeError.invalidURL) {
                    try codec.decode(url, nowUnixMillis: 1_000)
                }
            }
        }
    }
    #expect(RemotePairingRoute.scheme(for: "production") == "lumi")
    #expect(RemotePairingRoute.scheme(for: "release") == "lumi")
    #expect(RemotePairingRoute.scheme(for: "dev") == "lumi-dev")
    #expect(RemotePairingRoute.scheme(for: "rc") == "lumi-rc")
    #expect(RemotePairingRoute.scheme(for: "unknown") == nil)
}

@Test func iphoneRegistrationsMatchThePairingRouteWithoutSharedAliases() throws {
    var root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while !FileManager.default.fileExists(atPath: root.appendingPathComponent("apps/ios/Config").path) {
        let parent = root.deletingLastPathComponent()
        #expect(parent != root, "Cannot locate repository configuration")
        guard parent != root else { return }
        root = parent
    }
    let infoData = try Data(contentsOf: root.appendingPathComponent("apps/ios/LumiRemote/Resources/Info.plist"))
    let info = try #require(PropertyListSerialization.propertyList(from: infoData, format: nil) as? [String: Any])
    let types = try #require(info["CFBundleURLTypes"] as? [[String: Any]])
    #expect(types.count == 1)
    #expect(types[0]["CFBundleURLSchemes"] as? [String] == ["$(LUMI_PAIRING_URL_SCHEME)"])
    for (configuration, channel) in [("Release", "production"), ("Dev", "dev"), ("RC", "rc")] {
        let contents = try String(contentsOf: root.appendingPathComponent("apps/ios/Config/\(configuration).xcconfig"), encoding: .utf8)
        let routes = contents.split(separator: "\n").filter { $0.hasPrefix("LUMI_PAIRING_URL_SCHEME = ") }
        #expect(routes.map(String.init) == ["LUMI_PAIRING_URL_SCHEME = \(try #require(RemotePairingRoute.scheme(for: channel)))"])
    }
}
