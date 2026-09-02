import Foundation

enum RemoteClientHello: Encodable, Sendable {
    case authenticate(deviceID: String, credential: String)
    case pair(
        invitationID: String,
        invitationSecret: String,
        deviceID: String,
        displayName: String,
        deviceCredential: String
    )

    private enum CodingKeys: String, CodingKey {
        case kind
        case deviceID = "deviceId"
        case credential
        case invitationID = "invitationId"
        case invitationSecret
        case displayName
        case deviceCredential
    }

    func encode(to encoder: any Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .authenticate(deviceID, credential):
            try values.encode("authenticate", forKey: .kind)
            try values.encode(deviceID, forKey: .deviceID)
            try values.encode(credential, forKey: .credential)
        case let .pair(invitationID, invitationSecret, deviceID, displayName, credential):
            try values.encode("pair", forKey: .kind)
            try values.encode(invitationID, forKey: .invitationID)
            try values.encode(invitationSecret, forKey: .invitationSecret)
            try values.encode(deviceID, forKey: .deviceID)
            try values.encode(displayName, forKey: .displayName)
            try values.encode(credential, forKey: .deviceCredential)
        }
    }
}

enum RemoteServerHello: Decodable, Equatable, Sendable {
    case authenticated(installationID: String, controllerLeaseID: String?)
    case paired(installationID: String, controllerLeaseID: String?)

    private enum CodingKeys: String, CodingKey {
        case kind
        case installationID = "installationId"
        case controllerLeaseID = "controllerLeaseId"
    }

    init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let installationID = try values.decode(String.self, forKey: .installationID)
        let lease = try values.decodeIfPresent(String.self, forKey: .controllerLeaseID)
        switch try values.decode(String.self, forKey: .kind) {
        case "authenticated": self = .authenticated(
            installationID: installationID,
            controllerLeaseID: lease
        )
        case "paired": self = .paired(
            installationID: installationID,
            controllerLeaseID: lease
        )
        default: throw RemoteTransportError.invalidAuthenticationResponse
        }
    }

    var installationID: String {
        switch self {
        case let .authenticated(installationID, _), let .paired(installationID, _):
            installationID
        }
    }

    var controllerLeaseID: String? {
        switch self {
        case let .authenticated(_, lease), let .paired(_, lease): lease
        }
    }
}
