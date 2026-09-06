import Foundation
import LumiProtocol
import Testing
@testable import LumiLibraryWorkspace

@Test func macPairingQRRoutesToOnlyTheMatchingIPhoneChannel() throws {
    let data = Data("""
    {"installationId":"installation-123","invitationId":"invitation-123456",
     "invitationSecret":"ssssssssssssssssssssssssssssssss","shortCode":"123456",
     "certificateFingerprintSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
     "expiresAtUnixMillis":2000,"approved":false}
    """.utf8)
    let invitation = try JSONDecoder().decode(RemoteGatewayPairingInvitation.self, from: data)
    var encodedPayloads: [String] = []
    for (channel, scheme) in [("release", "lumi"), ("production", "lumi"), ("dev", "lumi-dev"), ("rc", "lumi-rc")] {
        let url = try #require(RemotePairingVisual.pairingURL(invitation, releaseChannel: channel))
        #expect(url.scheme == scheme)
        #expect(url.host == "pair")
        let components = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false))
        #expect(components.queryItems?.count == 1)
        let encoded = try #require(components.queryItems?.first?.value)
        var base64 = encoded.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        base64 += String(repeating: "=", count: (4 - base64.count % 4) % 4)
        let payloadData = try #require(Data(base64Encoded: base64))
        let payload = try #require(JSONSerialization.jsonObject(with: payloadData) as? [String: Any])
        #expect(payload["installationID"] as? String == invitation.installationID)
        #expect(payload["invitationSecret"] as? String == invitation.invitationSecret)
        #expect(payload["certificateFingerprintSHA256"] as? String == invitation.certificateFingerprintSHA256)
        #expect(payload["shortCode"] as? String == invitation.shortCode)
        #expect(payload["expiresAtUnixMillis"] as? Int == 2000)
        encodedPayloads.append(encoded)
    }
    #expect(encodedPayloads.count == 4)
    #expect(RemotePairingVisual.pairingURL(invitation, releaseChannel: "") == nil)
    #expect(RemotePairingVisual.pairingURL(invitation, releaseChannel: "unknown") == nil)
}
