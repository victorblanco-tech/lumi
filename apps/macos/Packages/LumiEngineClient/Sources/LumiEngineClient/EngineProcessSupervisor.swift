import Foundation
import LumiProtocol
import OSLog

public actor EngineProcessSupervisor {
    private static let logger = Logger(
        subsystem: "nl.blancoservices.lumi",
        category: "EngineProcessSupervisor"
    )

    private let transport: any EngineTransport
    private var process: Process?
    private var sessionToken: String?
    private var commandSequence: UInt64 = 0

    public init(transport: any EngineTransport = LoopbackEngineTransport()) {
        self.transport = transport
    }

    public func launch(engineExecutable: URL) async throws -> EngineEndpoint {
        await stop()

        guard FileManager.default.isExecutableFile(atPath: engineExecutable.path) else {
            throw EngineClientError.executableMissing
        }

        let token = try SessionTokenGenerator.generate()
        let process = Process()
        let stdout = Pipe()
        process.executableURL = engineExecutable
        process.standardOutput = stdout
        process.standardError = FileHandle.nullDevice
        var environment = ProcessInfo.processInfo.environment
        environment["LUMI_SESSION_TOKEN"] = token
        process.environment = environment

        do {
            try process.run()
        } catch {
            throw EngineClientError.processLaunchFailed
        }

        self.process = process
        sessionToken = token
        Self.logger.info("Started local Lumi engine process")

        do {
            let endpoint = try await readStartupEndpoint(
                from: stdout.fileHandleForReading
            )
            try validate(endpoint: endpoint)
            return endpoint
        } catch {
            await stop()
            throw error
        }
    }

    public func connect(to endpoint: EngineEndpoint) async throws -> MessageEnvelope {
        guard let sessionToken else {
            throw EngineClientError.authenticationFailed
        }

        try await transport.connect(to: endpoint)
        let snapshot = try await transport.authenticate(sessionToken: sessionToken)
        Self.logger.info("Authenticated local Lumi engine session")
        return snapshot
    }

    public func send(
        _ command: EnginePlanCommand,
        messageID: String = "cmd-\(UUID().uuidString)"
    ) async throws -> MessageEnvelope {
        try await exchange(payload: command.payload(), messageID: messageID)
    }

    public func getSnapshot(
        messageID: String = "cmd-\(UUID().uuidString)"
    ) async throws -> MessageEnvelope {
        try await exchange(
            payload: ["kind": .string("getSnapshot")],
            messageID: messageID
        )
    }

    public func isRunning() -> Bool {
        process?.isRunning == true
    }

    public func stop() async {
        await transport.close()

        if let process, process.isRunning {
            process.terminate()
        }
        process = nil
        sessionToken = nil
        commandSequence = 0
    }

    private func exchange(
        payload: [String: JSONValue],
        messageID: String
    ) async throws -> MessageEnvelope {
        guard process?.isRunning == true, sessionToken != nil else {
            throw EngineClientError.connectionClosed
        }
        let increment = commandSequence.addingReportingOverflow(1)
        guard !increment.overflow else {
            throw EngineClientError.commandSequenceOverflow
        }
        let nextSequence = increment.partialValue
        commandSequence = nextSequence
        let envelope = MessageEnvelope(
            protocolVersion: WireProtocol.version,
            messageType: .command,
            messageId: messageID,
            sequence: nextSequence,
            correlationId: messageID,
            sentAt: Date().ISO8601Format(),
            payload: payload
        )
        return try await transport.exchange(envelope)
    }

    private func validate(endpoint: EngineEndpoint) throws {
        guard endpoint.recordType == "engineReady" else {
            throw EngineClientError.invalidStartupRecord
        }
        guard endpoint.host == "127.0.0.1" else {
            throw EngineClientError.nonLoopbackEndpoint
        }
        guard endpoint.protocolVersion == WireProtocol.version else {
            throw EngineClientError.protocolMismatch(
                expected: WireProtocol.version,
                received: endpoint.protocolVersion
            )
        }
    }

    private func readStartupEndpoint(from handle: FileHandle) async throws -> EngineEndpoint {
        try await withThrowingTaskGroup(of: EngineEndpoint.self) { group in
            group.addTask {
                let data = handle.availableData
                guard !data.isEmpty else {
                    throw EngineClientError.invalidStartupRecord
                }
                do {
                    return try JSONDecoder().decode(EngineEndpoint.self, from: data)
                } catch {
                    throw EngineClientError.invalidStartupRecord
                }
            }
            group.addTask {
                try await Task.sleep(for: .seconds(5))
                throw EngineClientError.startupTimedOut
            }

            guard let endpoint = try await group.next() else {
                throw EngineClientError.invalidStartupRecord
            }
            group.cancelAll()
            return endpoint
        }
    }
}
