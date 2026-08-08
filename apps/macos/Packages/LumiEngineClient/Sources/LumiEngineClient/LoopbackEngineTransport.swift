import Foundation
import LumiProtocol
@preconcurrency import Network

private final class ConnectionContinuationGate: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, any Error>?

    init(_ continuation: CheckedContinuation<Void, any Error>) {
        self.continuation = continuation
    }

    func succeed() {
        resolve(with: .success(()))
    }

    func fail() {
        resolve(with: .failure(EngineClientError.connectionFailed))
    }

    private func resolve(with result: Result<Void, any Error>) {
        lock.lock()
        let pending = continuation
        continuation = nil
        lock.unlock()
        pending?.resume(with: result)
    }
}

public actor LoopbackEngineTransport: EngineTransport {
    private let queue = DispatchQueue(label: "co.victorblan.tech.lumi.engine-transport")
    private var connection: NWConnection?

    public init() {}

    public func connect(to endpoint: EngineEndpoint) async throws {
        guard endpoint.host == "127.0.0.1",
              let port = NWEndpoint.Port(rawValue: endpoint.port) else {
            throw EngineClientError.nonLoopbackEndpoint
        }

        await close()
        let connection = NWConnection(host: .ipv4(.loopback), port: port, using: .tcp)
        self.connection = connection

        try await withCheckedThrowingContinuation { continuation in
            let gate = ConnectionContinuationGate(continuation)
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    gate.succeed()
                case .failed, .cancelled:
                    gate.fail()
                default:
                    break
                }
            }
            connection.start(queue: queue)
        }
    }

    public func authenticate(sessionToken: String) async throws -> MessageEnvelope {
        guard let connection else {
            throw EngineClientError.connectionFailed
        }

        let authentication = try JSONEncoder().encode(
            SessionAuthentication(sessionToken: sessionToken)
        )
        var framedAuthentication = authentication
        framedAuthentication.append(0x0A)
        try await send(framedAuthentication, over: connection)

        let response = try await receiveLine(over: connection)
        let envelope = try ProtocolMessageDecoder.decode(response)
        guard envelope.messageType == .snapshot else {
            throw EngineClientError.invalidInitialSnapshot
        }
        return envelope
    }

    public func exchange(_ envelope: MessageEnvelope) async throws -> MessageEnvelope {
        guard let connection else {
            throw EngineClientError.connectionFailed
        }
        var encoded = try JSONEncoder().encode(envelope)
        encoded.append(0x0A)
        try await send(encoded, over: connection)
        let response = try await receiveLine(over: connection)
        return try ProtocolMessageDecoder.decode(response)
    }

    public func close() async {
        connection?.stateUpdateHandler = nil
        connection?.cancel()
        connection = nil
    }

    private func send(_ data: Data, over connection: NWConnection) async throws {
        try await withCheckedThrowingContinuation { continuation in
            connection.send(content: data, completion: .contentProcessed { error in
                if error == nil {
                    continuation.resume()
                } else {
                    continuation.resume(throwing: EngineClientError.authenticationFailed)
                }
            })
        }
    }

    private func receiveLine(over connection: NWConnection) async throws -> Data {
        var accumulated = Data()

        while accumulated.count <= WireProtocol.maximumMessageBytes {
            let chunk = try await receiveChunk(over: connection)
            accumulated.append(chunk)

            if let newline = accumulated.firstIndex(of: 0x0A) {
                return Data(accumulated[..<newline])
            }
        }

        throw ProtocolDecodingError.oversized(
            actual: accumulated.count,
            maximum: WireProtocol.maximumMessageBytes
        )
    }

    private func receiveChunk(over connection: NWConnection) async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            connection.receive(
                minimumIncompleteLength: 1,
                maximumLength: 4_096
            ) { data, _, isComplete, error in
                if let data, !data.isEmpty {
                    continuation.resume(returning: data)
                } else if error != nil || isComplete {
                    continuation.resume(throwing: EngineClientError.connectionClosed)
                } else {
                    continuation.resume(throwing: EngineClientError.connectionFailed)
                }
            }
        }
    }
}

private struct SessionAuthentication: Encodable {
    let sessionToken: String
}
