import CryptoKit
import Foundation
import LumiProtocol
@preconcurrency import Network
import Security

public actor PinnedRemoteTransport {
    private let queue = DispatchQueue(
        label: "co.victorblan.tech.lumi.remote.tls",
        qos: .userInitiated
    )
    private var connection: NWConnection?
    private var receiveBuffer = Data()
    private var outgoingSequence: UInt64 = 1

    public init() {}

    func connect(
        to endpoint: NWEndpoint,
        certificateFingerprintSHA256: String
    ) async throws {
        close()
        let expectedFingerprint = certificateFingerprintSHA256.lowercased()
        guard RemoteCertificatePin.valid(expectedFingerprint) else {
            throw RemoteTransportError.invalidCertificatePin
        }
        let tls = NWProtocolTLS.Options()
        sec_protocol_options_set_verify_block(
            tls.securityProtocolOptions,
            { _, trust, completion in
                let secTrust = sec_trust_copy_ref(trust).takeRetainedValue()
                completion(
                    RemoteCertificatePin.matches(
                        trust: secTrust,
                        expectedSHA256: expectedFingerprint
                    )
                )
            },
            queue
        )
        let parameters = NWParameters(tls: tls, tcp: NWProtocolTCP.Options())
        parameters.includePeerToPeer = true
        let connection = NWConnection(to: endpoint, using: parameters)
        self.connection = connection
        receiveBuffer.removeAll(keepingCapacity: true)
        outgoingSequence = 1
        try await withCheckedThrowingContinuation { continuation in
            let gate = RemoteContinuationGate<Void>(continuation)
            gate.arm(on: queue, after: 5, error: .connectionTimedOut) {
                connection.cancel()
            }
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready: gate.succeed(())
                case .failed, .cancelled: gate.fail(RemoteTransportError.connectionFailed)
                default: break
                }
            }
            connection.start(queue: queue)
        }
    }

    func authenticate(_ hello: RemoteClientHello) async throws -> RemoteServerHello {
        let payload = try Self.jsonValue(hello)
        try await sendFrame(
            RemoteFrame(
                frameKind: .hello,
                sequence: nextSequence(),
                payload: payload
            )
        )
        guard let data = try await receiveLine() else {
            throw RemoteTransportError.connectionClosed
        }
        let frame = try RemoteFrameDecoder().decodeFrame(data)
        if frame.frameKind == .error {
            throw RemoteTransportError.gatewayRejected(Self.errorReason(in: frame))
        }
        guard frame.frameKind == .hello else {
            throw RemoteTransportError.invalidAuthenticationResponse
        }
        do {
            return try JSONDecoder().decode(
                RemoteServerHello.self,
                from: JSONEncoder().encode(frame.payload)
            )
        } catch let error as RemoteTransportError {
            throw error
        } catch {
            throw RemoteTransportError.invalidAuthenticationResponse
        }
    }

    func send(command: RemoteCommand) async throws {
        try await sendFrame(
            RemoteFrame(
                frameKind: .command,
                sequence: nextSequence(),
                correlationID: command.commandID,
                payload: try Self.jsonValue(command)
            )
        )
    }

    func nextFrame() async throws -> Data? {
        try await receiveLine()
    }

    public func close() {
        connection?.stateUpdateHandler = nil
        connection?.cancel()
        connection = nil
        receiveBuffer.removeAll(keepingCapacity: false)
    }

    private func sendFrame(_ frame: RemoteFrame) async throws {
        guard let connection else { throw RemoteTransportError.notConnected }
        var data = try JSONEncoder().encode(frame)
        guard data.count <= lumiRemoteMaximumFrameBytes else {
            throw RemoteTransportError.oversizedFrame
        }
        data.append(0x0A)
        try await withCheckedThrowingContinuation { continuation in
            let gate = RemoteContinuationGate<Void>(continuation)
            gate.arm(on: queue, after: 5, error: .requestTimedOut) {
                connection.cancel()
            }
            connection.send(content: data, completion: .contentProcessed { error in
                error == nil
                    ? gate.succeed(())
                    : gate.fail(RemoteTransportError.connectionClosed)
            })
        }
    }

    private func receiveLine() async throws -> Data? {
        while receiveBuffer.count <= lumiRemoteMaximumFrameBytes {
            if let newline = receiveBuffer.firstIndex(of: 0x0A) {
                let line = Data(receiveBuffer[..<newline])
                receiveBuffer.removeSubrange(...newline)
                return line
            }
            guard let connection else { throw RemoteTransportError.notConnected }
            let chunk: Data? = try await withCheckedThrowingContinuation { continuation in
                let gate = RemoteContinuationGate<Data?>(continuation)
                gate.arm(on: queue, after: 20, error: .requestTimedOut) {
                    connection.cancel()
                }
                connection.receive(minimumIncompleteLength: 1, maximumLength: 16_384) {
                    data, _, complete, error in
                    if let data, !data.isEmpty {
                        gate.succeed(data)
                    } else if complete || error != nil {
                        gate.succeed(nil)
                    } else {
                        gate.fail(RemoteTransportError.connectionFailed)
                    }
                }
            }
            guard let chunk else { return nil }
            receiveBuffer.append(chunk)
        }
        throw RemoteTransportError.oversizedFrame
    }

    private func nextSequence() -> UInt64 {
        let value = outgoingSequence
        outgoingSequence = outgoingSequence.saturatingAdding(1)
        return value
    }

    private static func jsonValue<T: Encodable>(_ value: T) throws -> JSONValue {
        try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(value))
    }

    private static func errorReason(in frame: RemoteFrame) -> String {
        guard case let .object(payload) = frame.payload,
              case let .string(reason)? = payload["reasonCode"] else {
            return "gatewayRejected"
        }
        return reason
    }
}

enum RemoteCertificatePin {
    static func valid(_ fingerprint: String) -> Bool {
        fingerprint.count == 64 && fingerprint.allSatisfy(\.isHexDigit)
    }

    static func matches(trust: SecTrust, expectedSHA256: String) -> Bool {
        guard valid(expectedSHA256),
              let chain = SecTrustCopyCertificateChain(trust) as? [SecCertificate],
              let leaf = chain.first else {
            return false
        }
        let certificate = SecCertificateCopyData(leaf) as Data
        let digest = SHA256.hash(data: certificate)
        let actual = digest.map { String(format: "%02x", $0) }.joined()
        return actual == expectedSHA256.lowercased()
    }
}

private final class RemoteContinuationGate<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Value, any Error>?
    private var deadline: DispatchWorkItem?

    init(_ continuation: CheckedContinuation<Value, any Error>) {
        self.continuation = continuation
    }

    func arm(
        on queue: DispatchQueue,
        after seconds: TimeInterval,
        error: RemoteTransportError,
        onTimeout: @escaping @Sendable () -> Void
    ) {
        let item = DispatchWorkItem { [self] in
            guard let pending = take() else { return }
            onTimeout()
            pending.resume(throwing: error)
        }
        lock.withLock {
            guard continuation != nil else { return }
            deadline = item
            queue.asyncAfter(deadline: .now() + seconds, execute: item)
        }
    }

    func succeed(_ value: Value) { resolve(.success(value)) }
    func fail(_ error: any Error) { resolve(.failure(error)) }

    private func resolve(_ result: Result<Value, any Error>) {
        let values = lock.withLock { () -> (CheckedContinuation<Value, any Error>?, DispatchWorkItem?) in
            let values = (continuation, deadline)
            continuation = nil
            deadline = nil
            return values
        }
        values.1?.cancel()
        values.0?.resume(with: result)
    }

    private func take() -> CheckedContinuation<Value, any Error>? {
        lock.withLock {
            let value = continuation
            continuation = nil
            deadline = nil
            return value
        }
    }
}

public enum RemoteTransportError: Error, Equatable, LocalizedError {
    case invalidCertificatePin
    case connectionTimedOut
    case connectionFailed
    case connectionClosed
    case notConnected
    case requestTimedOut
    case oversizedFrame
    case invalidAuthenticationResponse
    case gatewayRejected(String)

    public var errorDescription: String? {
        switch self {
        case .invalidCertificatePin: "The Lumi Mac certificate identity is invalid."
        case .connectionTimedOut: "The Lumi Mac did not accept a secure connection in time."
        case .connectionFailed: "The secure connection to the Lumi Mac failed."
        case .connectionClosed: "The Lumi Mac closed the secure connection."
        case .notConnected: "Lumi Remote is not connected to a Mac."
        case .requestTimedOut: "The Lumi Mac did not respond in time."
        case .oversizedFrame: "The Lumi Mac sent more Remote data than allowed."
        case .invalidAuthenticationResponse: "The Lumi Mac sent an invalid authentication response."
        case let .gatewayRejected(reason): "The Lumi Mac rejected access (\(reason))."
        }
    }
}

private extension UInt64 {
    func saturatingAdding(_ value: UInt64) -> UInt64 {
        let (sum, overflow) = addingReportingOverflow(value)
        return overflow ? .max : sum
    }
}
