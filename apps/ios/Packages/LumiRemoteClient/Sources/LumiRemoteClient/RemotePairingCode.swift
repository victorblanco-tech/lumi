import Foundation

public struct RemotePairingCodeCodec: Sendable {
    private static let maximumEncodedBytes = 4_096
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    public init() {}

    public func encode(_ invitation: RemotePairingInvitation) throws -> URL {
        let data = try encoder.encode(invitation)
        guard data.count <= Self.maximumEncodedBytes else {
            throw RemotePairingCodeError.oversized
        }
        let token = data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        guard var components = URLComponents(string: "lumi://pair") else {
            throw RemotePairingCodeError.invalidURL
        }
        components.queryItems = [URLQueryItem(name: "invitation", value: token)]
        guard let url = components.url else { throw RemotePairingCodeError.invalidURL }
        return url
    }

    public func decode(_ url: URL, nowUnixMillis: UInt64) throws -> RemotePairingInvitation {
        guard url.scheme == "lumi", url.host == "pair",
              let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let encoded = components.queryItems?.first(where: {
                  $0.name == "invitation"
              })?.value,
              encoded.utf8.count <= Self.maximumEncodedBytes * 2 else {
            throw RemotePairingCodeError.invalidURL
        }
        var base64 = encoded
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let remainder = base64.count % 4
        if remainder != 0 {
            base64 += String(repeating: "=", count: 4 - remainder)
        }
        guard let data = Data(base64Encoded: base64),
              data.count <= Self.maximumEncodedBytes else {
            throw RemotePairingCodeError.invalidPayload
        }
        do {
            let invitation = try decoder.decode(RemotePairingInvitation.self, from: data)
            try invitation.validate(nowUnixMillis: nowUnixMillis)
            return invitation
        } catch let error as RemoteTrustError {
            throw error
        } catch {
            throw RemotePairingCodeError.invalidPayload
        }
    }
}

public enum RemotePairingCodeError: Error, Equatable {
    case invalidURL
    case invalidPayload
    case oversized
}
