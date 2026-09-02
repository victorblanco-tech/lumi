import Foundation
import LumiProtocol
@preconcurrency import Network
import ServiceManagement

public actor RemoteGatewaySupervisor {
    private let launchAgentPlistName: String?
    private let expectedProductVersion: String?
    private var service: SMAppService?

    public init(
        launchAgentPlistName: String? = Bundle.main.object(
            forInfoDictionaryKey: "LumiRemoteGatewayLaunchAgentPlistName"
        ) as? String,
        expectedProductVersion: String? = Bundle.main.object(
            forInfoDictionaryKey: "LumiProductVersion"
        ) as? String
    ) {
        self.launchAgentPlistName = launchAgentPlistName
        self.expectedProductVersion = expectedProductVersion
    }

    public func refresh(recordURL: URL) async -> RemoteGatewayManagementSnapshot {
        guard let launchAgentPlistName else {
            return .init(serviceState: .unavailable, errorCode: "gatewayNotPackaged")
        }
        let service = service ?? SMAppService.agent(plistName: launchAgentPlistName)
        self.service = service
        switch service.status {
        case .requiresApproval:
            return .init(serviceState: .requiresApproval)
        case .enabled:
            do {
                return try await exchange(.status, recordURL: recordURL)
            } catch RemoteGatewayClientError.serviceVersionMismatch {
                return .init(
                    serviceState: .unavailable,
                    errorCode: "gatewayUpdateRequired"
                )
            } catch {
                return .init(serviceState: .starting, errorCode: "gatewayStarting")
            }
        case .notRegistered, .notFound:
            return .disabled
        @unknown default:
            return .init(serviceState: .unavailable, errorCode: "gatewayUnavailable")
        }
    }

    public func enable(recordURL: URL) async throws -> RemoteGatewayManagementSnapshot {
        guard let launchAgentPlistName else { throw RemoteGatewayClientError.notPackaged }
        let service = service ?? SMAppService.agent(plistName: launchAgentPlistName)
        self.service = service
        switch service.status {
        case .enabled:
            if let expectedProductVersion,
               (try? readRecord(at: recordURL).productVersion) != expectedProductVersion {
                try await service.unregister()
                try service.register()
            }
        case .notRegistered, .notFound:
            try service.register()
        case .requiresApproval:
            throw RemoteGatewayClientError.requiresApproval
        @unknown default:
            throw RemoteGatewayClientError.registrationFailed
        }
        if service.status == .requiresApproval {
            throw RemoteGatewayClientError.requiresApproval
        }
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .seconds(8))
        while clock.now < deadline {
            if let snapshot = try? await exchange(.status, recordURL: recordURL) {
                return snapshot
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        throw RemoteGatewayClientError.startupTimedOut
    }

    public func disable(recordURL: URL) async throws -> RemoteGatewayManagementSnapshot {
        guard let launchAgentPlistName else { return .disabled }
        let service = service ?? SMAppService.agent(plistName: launchAgentPlistName)
        self.service = service
        if service.status == .enabled {
            try await service.unregister()
        }
        try? FileManager.default.removeItem(at: recordURL)
        return .disabled
    }

    public func createInvitation(recordURL: URL) async throws -> RemoteGatewayManagementSnapshot {
        try await exchange(.createInvitation, recordURL: recordURL)
    }

    public func approve(
        invitationID: String,
        shortCode: String,
        recordURL: URL
    ) async throws -> RemoteGatewayManagementSnapshot {
        try await exchange(
            .approveInvitation(invitationID: invitationID, shortCode: shortCode),
            recordURL: recordURL
        )
    }

    public func revoke(
        deviceID: String,
        recordURL: URL
    ) async throws -> RemoteGatewayManagementSnapshot {
        try await exchange(.revokeDevice(deviceID: deviceID), recordURL: recordURL)
    }

    public func transferControl(
        to deviceID: String,
        recordURL: URL
    ) async throws -> RemoteGatewayManagementSnapshot {
        try await exchange(.transferControl(deviceID: deviceID), recordURL: recordURL)
    }

    private func exchange(
        _ request: RemoteGatewayAdminRequest,
        recordURL: URL
    ) async throws -> RemoteGatewayManagementSnapshot {
        let record = try readRecord(at: recordURL)
        let response = try await GatewayAdminTransport.exchange(record: record, request: request)
        guard response.ok else {
            throw RemoteGatewayClientError.rejected(response.errorCode ?? "gatewayRejected")
        }
        return .init(
            serviceState: .ready,
            status: response.status,
            invitation: response.invitation,
            errorCode: response.errorCode
        )
    }

    private func readRecord(at url: URL) throws -> RemoteGatewayServiceRecord {
        let values = try url.resourceValues(forKeys: [
            .isRegularFileKey,
            .isSymbolicLinkKey,
            .fileSizeKey
        ])
        guard values.isRegularFile == true,
              values.isSymbolicLink != true,
              (values.fileSize ?? .max) <= 16_384 else {
            throw RemoteGatewayClientError.invalidServiceRecord
        }
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        let permissions = (attributes[.posixPermissions] as? NSNumber)?.uint16Value ?? 0o777
        guard permissions & 0o077 == 0 else {
            throw RemoteGatewayClientError.invalidServiceRecord
        }
        let record = try JSONDecoder().decode(
            RemoteGatewayServiceRecord.self,
            from: Data(contentsOf: url)
        )
        try record.validate(expectedProductVersion: expectedProductVersion)
        return record
    }
}

struct RemoteGatewayServiceRecord: Codable, Sendable {
    let endpointHost: String
    let endpointPort: UInt16
    let adminToken: String
    let processID: Int32
    let productVersion: String
    let installationID: String
    let certificateFingerprintSHA256: String
    let lanPort: UInt16

    private enum CodingKeys: String, CodingKey {
        case endpointHost
        case endpointPort
        case adminToken
        case processID
        case productVersion
        case installationID = "installationId"
        case certificateFingerprintSHA256 = "certificateFingerprintSha256"
        case lanPort
    }

    func validate(expectedProductVersion: String?) throws {
        guard endpointHost == "127.0.0.1",
              endpointPort > 0,
              adminToken.count >= 32,
              processID > 1,
              installationID.count == 32,
              certificateFingerprintSHA256.count == 64,
              lanPort > 0 else {
            throw RemoteGatewayClientError.invalidServiceRecord
        }
        if let expectedProductVersion, productVersion != expectedProductVersion {
            throw RemoteGatewayClientError.serviceVersionMismatch
        }
    }
}

struct RemoteGatewayAdminResponse: Codable, Sendable {
    let ok: Bool
    let status: RemoteGatewayStatus
    let invitation: RemoteGatewayPairingInvitation?
    let errorCode: String?
}

enum RemoteGatewayAdminRequest: Encodable, Sendable {
    case status
    case createInvitation
    case approveInvitation(invitationID: String, shortCode: String)
    case revokeDevice(deviceID: String)
    case transferControl(deviceID: String)

    enum CodingKeys: String, CodingKey {
        case action
        case invitationID = "invitationId"
        case shortCode
        case deviceID = "deviceId"
    }

    func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .status:
            try values.encode("status", forKey: .action)
        case .createInvitation:
            try values.encode("createInvitation", forKey: .action)
        case let .approveInvitation(invitationID, shortCode):
            try values.encode("approveInvitation", forKey: .action)
            try values.encode(invitationID, forKey: .invitationID)
            try values.encode(shortCode, forKey: .shortCode)
        case let .revokeDevice(deviceID):
            try values.encode("revokeDevice", forKey: .action)
            try values.encode(deviceID, forKey: .deviceID)
        case let .transferControl(deviceID):
            try values.encode("transferControl", forKey: .action)
            try values.encode(deviceID, forKey: .deviceID)
        }
    }
}

private struct RemoteGatewayAdminAuthentication: Encodable, Sendable {
    let adminToken: String
}

private enum GatewayAdminTransport {
    private static let queue = DispatchQueue(
        label: "co.victorblan.tech.lumi.remote-gateway-admin",
        qos: .userInitiated
    )

    static func exchange(
        record: RemoteGatewayServiceRecord,
        request: RemoteGatewayAdminRequest
    ) async throws -> RemoteGatewayAdminResponse {
        guard let port = NWEndpoint.Port(rawValue: record.endpointPort) else {
            throw RemoteGatewayClientError.invalidServiceRecord
        }
        let connection = NWConnection(host: .ipv4(.loopback), port: port, using: .tcp)
        try await connect(connection)
        defer { connection.cancel() }
        let encoder = JSONEncoder()
        var outbound = try encoder.encode(
            RemoteGatewayAdminAuthentication(adminToken: record.adminToken)
        )
        outbound.append(0x0A)
        outbound.append(try encoder.encode(request))
        outbound.append(0x0A)
        try await send(outbound, over: connection)
        let response = try await receiveLine(over: connection)
        return try JSONDecoder().decode(RemoteGatewayAdminResponse.self, from: response)
    }

    private static func connect(_ connection: NWConnection) async throws {
        try await withCheckedThrowingContinuation { continuation in
            let gate = DeadlineContinuationGate<Void>(continuation)
            gate.arm(
                on: queue,
                after: 3,
                error: RemoteGatewayClientError.connectionTimedOut,
                onTimeout: { connection.cancel() }
            )
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready: gate.succeed(())
                case .failed, .cancelled:
                    gate.fail(RemoteGatewayClientError.connectionFailed)
                default: break
                }
            }
            connection.start(queue: queue)
        }
    }

    private static func send(_ data: Data, over connection: NWConnection) async throws {
        try await withCheckedThrowingContinuation { continuation in
            let gate = DeadlineContinuationGate<Void>(continuation)
            gate.arm(
                on: queue,
                after: 3,
                error: RemoteGatewayClientError.requestTimedOut,
                onTimeout: { connection.cancel() }
            )
            connection.send(content: data, completion: .contentProcessed { error in
                if error == nil {
                    gate.succeed(())
                } else {
                    gate.fail(RemoteGatewayClientError.connectionClosed)
                }
            })
        }
    }

    private static func receiveLine(over connection: NWConnection) async throws -> Data {
        var received = Data()
        while received.count <= 64 * 1_024 {
            let chunk: Data = try await withCheckedThrowingContinuation { continuation in
                let gate = DeadlineContinuationGate<Data>(continuation)
                gate.arm(
                    on: queue,
                    after: 3,
                    error: RemoteGatewayClientError.requestTimedOut,
                    onTimeout: { connection.cancel() }
                )
                connection.receive(minimumIncompleteLength: 1, maximumLength: 4_096) {
                    data, _, complete, error in
                    if let data, !data.isEmpty {
                        gate.succeed(data)
                    } else if complete || error != nil {
                        gate.fail(RemoteGatewayClientError.connectionClosed)
                    } else {
                        gate.fail(RemoteGatewayClientError.connectionFailed)
                    }
                }
            }
            received.append(chunk)
            if let newline = received.firstIndex(of: 0x0A) {
                return Data(received[..<newline])
            }
        }
        throw RemoteGatewayClientError.oversizedResponse
    }
}

public enum RemoteGatewayClientError: Error, Equatable, LocalizedError {
    case notPackaged
    case requiresApproval
    case registrationFailed
    case startupTimedOut
    case invalidServiceRecord
    case serviceVersionMismatch
    case connectionTimedOut
    case connectionFailed
    case connectionClosed
    case requestTimedOut
    case oversizedResponse
    case rejected(String)

    public var errorDescription: String? {
        switch self {
        case .notPackaged: "Lumi Remote Gateway is not included in this app build."
        case .requiresApproval: "Allow Lumi Remote Gateway in System Settings > Login Items."
        case .registrationFailed: "Lumi Remote Gateway could not be enabled."
        case .startupTimedOut: "Lumi Remote Gateway did not become ready in time."
        case .invalidServiceRecord: "Lumi Remote Gateway published an invalid local record."
        case .serviceVersionMismatch: "Lumi Remote Gateway must be updated for this Lumi version."
        case .connectionTimedOut: "Lumi Remote Gateway connection timed out."
        case .connectionFailed: "Lumi Remote Gateway could not be reached."
        case .connectionClosed: "Lumi Remote Gateway closed the connection."
        case .requestTimedOut: "Lumi Remote Gateway did not answer in time."
        case .oversizedResponse: "Lumi Remote Gateway returned an oversized response."
        case let .rejected(code): "Lumi Remote Gateway rejected the request (\(code))."
        }
    }
}
