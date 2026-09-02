import Foundation

public enum RemoteReleaseChannel: String, Codable, Sendable {
    case dev
    case rc
    case production

    public var bonjourServiceType: String {
        switch self {
        case .dev: "_lumi-remote-dev._tcp"
        case .rc: "_lumi-remote-rc._tcp"
        case .production: "_lumi-remote._tcp"
        }
    }
}

public struct RemoteServiceIdentity: Codable, Equatable, Sendable {
    public let name: String
    public let installationID: String
    public let protocolVersion: Int
    public let releaseChannel: RemoteReleaseChannel

    public init(
        name: String,
        installationID: String,
        protocolVersion: Int,
        releaseChannel: RemoteReleaseChannel
    ) {
        self.name = name
        self.installationID = installationID
        self.protocolVersion = protocolVersion
        self.releaseChannel = releaseChannel
    }

    public func validate(for expectedChannel: RemoteReleaseChannel) throws {
        guard releaseChannel == expectedChannel else {
            throw RemoteTrustError.releaseChannelMismatch
        }
        guard protocolVersion == lumiRemoteProtocolVersion else {
            throw RemoteTrustError.protocolMismatch
        }
        guard Self.validIdentifier(installationID), !name.isEmpty, name.count <= 128 else {
            throw RemoteTrustError.invalidServiceIdentity
        }
    }

    private static func validIdentifier(_ value: String) -> Bool {
        !value.isEmpty && value.count <= 128 && !value.contains(where: \.isNewline)
    }
}

public struct RemotePairingInvitation: Codable, Equatable, Sendable {
    public let installationID: String
    public let invitationID: String
    public let invitationSecret: String
    public let certificateFingerprintSHA256: String
    public let expiresAtUnixMillis: UInt64

    public init(
        installationID: String,
        invitationID: String,
        invitationSecret: String,
        certificateFingerprintSHA256: String,
        expiresAtUnixMillis: UInt64
    ) {
        self.installationID = installationID
        self.invitationID = invitationID
        self.invitationSecret = invitationSecret
        self.certificateFingerprintSHA256 = certificateFingerprintSHA256
        self.expiresAtUnixMillis = expiresAtUnixMillis
    }

    public func validate(nowUnixMillis: UInt64) throws {
        guard expiresAtUnixMillis > nowUnixMillis else { throw RemoteTrustError.invitationExpired }
        guard invitationSecret.count >= 32, invitationID.count >= 16 else {
            throw RemoteTrustError.invalidInvitation
        }
        let fingerprint = certificateFingerprintSHA256.lowercased()
        guard fingerprint.count == 64,
              fingerprint.allSatisfy({ $0.isHexDigit }) else {
            throw RemoteTrustError.invalidCertificateFingerprint
        }
    }
}

public struct RemoteDeviceCredential: Codable, Equatable, Sendable {
    public let installationID: String
    public let deviceID: String
    public let credential: String
    public let certificateFingerprintSHA256: String
    public let releaseChannel: RemoteReleaseChannel

    public init(
        installationID: String,
        deviceID: String,
        credential: String,
        certificateFingerprintSHA256: String,
        releaseChannel: RemoteReleaseChannel
    ) {
        self.installationID = installationID
        self.deviceID = deviceID
        self.credential = credential
        self.certificateFingerprintSHA256 = certificateFingerprintSHA256
        self.releaseChannel = releaseChannel
    }

    public func validate(
        for expectedInstallationID: String,
        expectedChannel: RemoteReleaseChannel
    ) throws {
        guard installationID == expectedInstallationID,
              releaseChannel == expectedChannel else {
            throw RemoteTrustError.credentialScopeMismatch
        }
        guard Self.validIdentifier(installationID),
              Self.validIdentifier(deviceID),
              credential.count >= 32,
              credential.count <= 512 else {
            throw RemoteTrustError.invalidCredential
        }
        let fingerprint = certificateFingerprintSHA256.lowercased()
        guard fingerprint.count == 64,
              fingerprint.allSatisfy({ $0.isHexDigit }) else {
            throw RemoteTrustError.invalidCertificateFingerprint
        }
    }

    private static func validIdentifier(_ value: String) -> Bool {
        !value.isEmpty && value.count <= 128 && !value.contains(where: \.isNewline)
    }
}

public protocol RemoteCredentialStore: Sendable {
    func credential(for installationID: String) async throws -> RemoteDeviceCredential?
    func save(_ credential: RemoteDeviceCredential) async throws
    func remove(installationID: String) async throws
}

public enum RemoteTrustError: Error, Equatable {
    case releaseChannelMismatch
    case protocolMismatch
    case invalidServiceIdentity
    case invitationExpired
    case invalidInvitation
    case invalidCertificateFingerprint
    case credentialScopeMismatch
    case invalidCredential
}
