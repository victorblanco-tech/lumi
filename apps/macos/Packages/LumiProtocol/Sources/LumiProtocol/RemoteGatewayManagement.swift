/// Provider-neutral presentation state for the separately supervised Remote
/// Gateway. Process ownership stays in LumiEngineClient; feature views only
/// consume these immutable values.
public enum RemoteGatewayServiceState: String, Equatable, Sendable {
    case disabled
    case starting
    case ready
    case requiresApproval
    case unavailable
}

public struct RemoteGatewayDeviceStatus: Codable, Equatable, Identifiable, Sendable {
    public var id: String { deviceID }
    public let deviceID: String
    public let displayName: String
    public let pairedAtUnixMillis: UInt64
    public let lastSeenUnixMillis: UInt64
    public let controller: Bool
    public let clientVersion: String?

    private enum CodingKeys: String, CodingKey {
        case deviceID = "deviceId"
        case displayName
        case pairedAtUnixMillis
        case lastSeenUnixMillis
        case controller
        case clientVersion
    }
}

public struct RemoteGatewayStatus: Codable, Equatable, Sendable {
    public let engineConnected: Bool
    public let installationID: String
    public let certificateFingerprintSHA256: String
    public let lanPort: UInt16
    public let pairedDevices: [RemoteGatewayDeviceStatus]
    public let controllerDeviceID: String?
    public let controllerChanges: [RemoteControllerChange]?

    private enum CodingKeys: String, CodingKey {
        case engineConnected
        case installationID = "installationId"
        case certificateFingerprintSHA256 = "certificateFingerprintSha256"
        case lanPort
        case pairedDevices
        case controllerDeviceID = "controllerDeviceId"
        case controllerChanges
    }
}

public struct RemoteControllerChange: Codable, Equatable, Sendable {
    public let atUnixMillis: UInt64
    public let reason: String
    public let previousDeviceId: String?
    public let deviceId: String?
}

public struct RemoteGatewayPairingInvitation: Codable, Equatable, Sendable {
    public let installationID: String
    public let invitationID: String
    public let invitationSecret: String
    public let shortCode: String
    public let certificateFingerprintSHA256: String
    public let expiresAtUnixMillis: UInt64
    public let approved: Bool

    private enum CodingKeys: String, CodingKey {
        case installationID = "installationId"
        case invitationID = "invitationId"
        case invitationSecret
        case shortCode
        case certificateFingerprintSHA256 = "certificateFingerprintSha256"
        case expiresAtUnixMillis
        case approved
    }
}

public struct RemoteGatewayManagementSnapshot: Equatable, Sendable {
    public let serviceState: RemoteGatewayServiceState
    public let status: RemoteGatewayStatus?
    public let invitation: RemoteGatewayPairingInvitation?
    public let errorCode: String?

    public init(
        serviceState: RemoteGatewayServiceState,
        status: RemoteGatewayStatus? = nil,
        invitation: RemoteGatewayPairingInvitation? = nil,
        errorCode: String? = nil
    ) {
        self.serviceState = serviceState
        self.status = status
        self.invitation = invitation
        self.errorCode = errorCode
    }

    public static let disabled = Self(serviceState: .disabled)
}
