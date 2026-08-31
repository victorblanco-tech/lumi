import Foundation
import LumiProtocol
@preconcurrency import Network

struct EngineTransportTimeouts: Equatable, Sendable {
    let connectSeconds: TimeInterval
    let requestSeconds: TimeInterval

    static let production = Self(connectSeconds: 5, requestSeconds: 15)
}

final class DeadlineContinuationGate<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Value, any Error>?
    private var deadline: DispatchWorkItem?

    init(_ continuation: CheckedContinuation<Value, any Error>) {
        self.continuation = continuation
    }

    func arm(
        on queue: DispatchQueue,
        after seconds: TimeInterval,
        error: EngineClientError,
        onTimeout: @escaping @Sendable () -> Void
    ) {
        let item = DispatchWorkItem { [self] in
            guard let pending = takeContinuation() else { return }
            onTimeout()
            pending.resume(throwing: error)
        }
        lock.lock()
        guard continuation != nil else {
            lock.unlock()
            return
        }
        deadline = item
        lock.unlock()
        queue.asyncAfter(deadline: .now() + seconds, execute: item)
    }

    func succeed(_ value: Value) {
        resolve(with: .success(value))
    }

    func fail(_ error: EngineClientError) {
        resolve(with: .failure(error))
    }

    private func resolve(with result: Result<Value, any Error>) {
        lock.lock()
        let pending = continuation
        continuation = nil
        let deadline = deadline
        self.deadline = nil
        lock.unlock()
        deadline?.cancel()
        pending?.resume(with: result)
    }

    private func takeContinuation() -> CheckedContinuation<Value, any Error>? {
        lock.lock()
        let pending = continuation
        continuation = nil
        deadline = nil
        lock.unlock()
        return pending
    }

}

public actor LoopbackEngineTransport: EngineTransport {
    private let queue = DispatchQueue(label: "co.victorblan.tech.lumi.engine-transport")
    private let timeouts: EngineTransportTimeouts
    private var connection: NWConnection?

    public init() {
        timeouts = .production
    }

    init(timeouts: EngineTransportTimeouts) {
        self.timeouts = timeouts
    }

    public func connect(to endpoint: EngineEndpoint) async throws {
        guard endpoint.host == "127.0.0.1",
              let port = NWEndpoint.Port(rawValue: endpoint.port) else {
            throw EngineClientError.nonLoopbackEndpoint
        }

        await close()
        let connection = NWConnection(host: .ipv4(.loopback), port: port, using: .tcp)
        self.connection = connection

        try await withCheckedThrowingContinuation { continuation in
            let gate = DeadlineContinuationGate<Void>(continuation)
            gate.arm(
                on: queue,
                after: timeouts.connectSeconds,
                error: .connectionTimedOut,
                onTimeout: { connection.cancel() }
            )
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    gate.succeed(())
                case .failed, .cancelled:
                    gate.fail(.connectionFailed)
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
        try await send(framedAuthentication, over: connection, timeout: timeouts.requestSeconds)

        let response = try await receiveLine(over: connection, timeout: timeouts.requestSeconds)
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
        try await send(encoded, over: connection, timeout: timeouts.requestSeconds)
        let response = try await receiveLine(over: connection, timeout: timeouts.requestSeconds)
        return try ProtocolMessageDecoder.decode(response)
    }

    public func close() async {
        connection?.stateUpdateHandler = nil
        connection?.cancel()
        connection = nil
    }

    private func send(
        _ data: Data,
        over connection: NWConnection,
        timeout: TimeInterval
    ) async throws {
        try await withCheckedThrowingContinuation { continuation in
            let gate = DeadlineContinuationGate<Void>(continuation)
            gate.arm(
                on: queue,
                after: timeout,
                error: .requestTimedOut,
                onTimeout: { connection.cancel() }
            )
            connection.send(content: data, completion: .contentProcessed { error in
                if error == nil {
                    gate.succeed(())
                } else {
                    gate.fail(.connectionClosed)
                }
            })
        }
    }

    private func receiveLine(
        over connection: NWConnection,
        timeout: TimeInterval
    ) async throws -> Data {
        var accumulated = Data()

        while accumulated.count <= WireProtocol.maximumMessageBytes {
            let chunk = try await receiveChunk(over: connection, timeout: timeout)
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

    private func receiveChunk(
        over connection: NWConnection,
        timeout: TimeInterval
    ) async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            let gate = DeadlineContinuationGate<Data>(continuation)
            gate.arm(
                on: queue,
                after: timeout,
                error: .requestTimedOut,
                onTimeout: { connection.cancel() }
            )
            connection.receive(
                minimumIncompleteLength: 1,
                maximumLength: 4_096
            ) { data, _, isComplete, error in
                if let data, !data.isEmpty {
                    gate.succeed(data)
                } else if error != nil || isComplete {
                    gate.fail(.connectionClosed)
                } else {
                    gate.fail(.connectionFailed)
                }
            }
        }
    }
}

private struct SessionAuthentication: Encodable {
    let sessionToken: String
}
