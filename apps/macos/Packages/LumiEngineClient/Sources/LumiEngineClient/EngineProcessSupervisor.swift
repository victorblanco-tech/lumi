import Foundation
import LumiProtocol
import OSLog
import Darwin
import CryptoKit

public actor EngineProcessSupervisor {
    private static let logger = Logger(
        subsystem: Bundle.main.bundleIdentifier ?? "co.victorblan.tech.lumi",
        category: "EngineProcessSupervisor"
    )

    private let transport: any EngineTransport
    private var process: Process?
    private var attachedProcessID: Int32?
    private var sessionToken: String?
    private var serviceRecordURL: URL?
    private var commandSequence: UInt64 = 0
    private var exchangeInProgress = false
    private var exchangeWaiters: [CheckedContinuation<Void, Never>] = []

    private struct ServiceRecord: Codable {
        let endpoint: EngineEndpoint
        let sessionToken: String
        let processID: Int32
        let productVersion: String
        let serviceIdentity: ServiceIdentity?
    }

    private struct ServiceIdentity: Codable, Equatable {
        let productVersion: String
        let buildNumber: String
        let engineExecutablePath: String
        let engineExecutableSHA256: String
    }

    public init(transport: any EngineTransport = LoopbackEngineTransport()) {
        self.transport = transport
    }

    public func launch(
        engineExecutable: URL,
        libraryDatabaseURL: URL? = nil,
        automaticallyPublishesMidi: Bool = true
    ) async throws -> EngineEndpoint {
        guard FileManager.default.isExecutableFile(atPath: engineExecutable.path) else {
            throw EngineClientError.executableMissing
        }
        let serviceIdentity = try makeServiceIdentity(engineExecutable: engineExecutable)

        if let libraryDatabaseURL {
            let serviceRecordName = libraryDatabaseURL.lastPathComponent == "library.sqlite"
                ? "engine-service.json"
                : ".\(libraryDatabaseURL.lastPathComponent).engine-service.json"
            let recordURL = libraryDatabaseURL
                .deletingLastPathComponent()
                .appendingPathComponent(serviceRecordName)
            serviceRecordURL = recordURL
            if let record = readServiceRecord(at: recordURL),
               record.endpoint.protocolVersion == WireProtocol.version,
               processIsRunning(record.processID),
               record.serviceIdentity == serviceIdentity {
                sessionToken = record.sessionToken
                attachedProcessID = record.processID
                commandSequence = 0
                Self.logger.info(
                    "Attaching to Lumi engine service pid \(record.processID) version \(record.productVersion, privacy: .public)"
                )
                return record.endpoint
            }
            if let record = readServiceRecord(at: recordURL),
               processIsRunning(record.processID) {
                Self.logger.notice(
                    "Replacing stale Lumi engine service pid \(record.processID) version \(record.productVersion, privacy: .public)"
                )
                try await retireServiceProcess(record.processID)
            }
            try? FileManager.default.removeItem(at: recordURL)
        }

        await transport.close()
        process = nil
        attachedProcessID = nil
        sessionToken = nil
        commandSequence = 0

        let token = try SessionTokenGenerator.generate()
        let process = Process()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = engineExecutable
        process.standardOutput = stdout
        process.standardError = stderr
        stderr.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            guard !data.isEmpty,
                  let message = String(data: data, encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines),
                  !message.isEmpty else {
                return
            }
            Self.logger.error("Local Lumi engine: \(message, privacy: .private(mask: .hash))")
        }
        process.terminationHandler = { process in
            stderr.fileHandleForReading.readabilityHandler = nil
            Self.logger.error(
                "Local Lumi engine terminated with status \(process.terminationStatus)"
            )
        }
        var environment = ProcessInfo.processInfo.environment
        environment["LUMI_SESSION_TOKEN"] = token
        environment["LUMI_AUTO_PUBLISH_MIDI"] = automaticallyPublishesMidi ? "1" : "0"
        // An app-owned engine must not survive a UI crash or forced exit. A
        // normal Quit still uses the graceful stop path below; an unexpected
        // client disconnect makes the engine tear down its owned helpers.
        environment["LUMI_EXIT_AFTER_CLIENT_DISCONNECT"] = "1"
        if let libraryDatabaseURL {
            environment["LUMI_LIBRARY_DATABASE_PATH"] = libraryDatabaseURL.path
        }
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
            if let serviceRecordURL {
                try writeServiceRecord(
                    ServiceRecord(
                        endpoint: endpoint,
                        sessionToken: token,
                        processID: process.processIdentifier,
                        productVersion: serviceIdentity.productVersion,
                        serviceIdentity: serviceIdentity
                    ),
                    to: serviceRecordURL
                )
            }
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
        _ command: EngineCommand,
        messageID: String = "cmd-\(UUID().uuidString)"
    ) async throws -> MessageEnvelope {
        try await exchange(payload: command.payload(), messageID: messageID)
    }

    public func getSnapshot(
        includeLibrary: Bool = true,
        messageID: String = "cmd-\(UUID().uuidString)"
    ) async throws -> MessageEnvelope {
        try await exchange(
            payload: [
                "kind": .string("getSnapshot"),
                "includeLibrary": .boolean(includeLibrary),
            ],
            messageID: messageID
        )
    }

    public func isRunning() -> Bool {
        process?.isRunning == true || attachedProcessID.map(processIsRunning) == true
    }

    public func stop() async {
        await transport.close()

        if let process, process.isRunning {
            await terminateProcess(process.processIdentifier)
            if !process.isRunning {
                process.waitUntilExit()
            }
        } else if let attachedProcessID, processIsRunning(attachedProcessID) {
            await terminateProcess(attachedProcessID)
        }
        if let serviceRecordURL {
            try? FileManager.default.removeItem(at: serviceRecordURL)
        }
        process = nil
        attachedProcessID = nil
        sessionToken = nil
        commandSequence = 0
    }

    private func terminateProcess(_ processID: Int32) async {
        guard processIsRunning(processID) else { return }
        _ = Darwin.kill(processID, SIGTERM)
        let clock = ContinuousClock()
        let gracefulDeadline = clock.now.advanced(by: .seconds(5))
        while processIsRunning(processID), clock.now < gracefulDeadline {
            try? await Task.sleep(for: .milliseconds(25))
        }
        guard processIsRunning(processID) else { return }
        Self.logger.fault(
            "Lumi engine pid \(processID) ignored graceful termination; forcing exit"
        )
        _ = Darwin.kill(processID, SIGKILL)
        let forcedDeadline = clock.now.advanced(by: .seconds(2))
        while processIsRunning(processID), clock.now < forcedDeadline {
            try? await Task.sleep(for: .milliseconds(25))
        }
    }

    private func exchange(
        payload: [String: JSONValue],
        messageID: String
    ) async throws -> MessageEnvelope {
        await acquireExchangeLease()
        defer { releaseExchangeLease() }
        guard isRunning(), sessionToken != nil else {
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

    /// Actor isolation alone does not serialize an async network exchange:
    /// the actor can re-enter while the first receive is suspended. Keep one
    /// request/response pair on the loopback stream at a time so monitor polls
    /// cannot consume an interactive command's response (or vice versa).
    private func acquireExchangeLease() async {
        if !exchangeInProgress {
            exchangeInProgress = true
            return
        }
        await withCheckedContinuation { continuation in
            exchangeWaiters.append(continuation)
        }
    }

    private func releaseExchangeLease() {
        guard !exchangeWaiters.isEmpty else {
            exchangeInProgress = false
            return
        }
        exchangeWaiters.removeFirst().resume()
    }

    private func readServiceRecord(at url: URL) -> ServiceRecord? {
        guard let values = try? url.resourceValues(forKeys: [.isRegularFileKey]),
              values.isRegularFile == true,
              let data = try? Data(contentsOf: url),
              data.count <= 4_096 else {
            return nil
        }
        return try? JSONDecoder().decode(ServiceRecord.self, from: data)
    }

    private func writeServiceRecord(_ record: ServiceRecord, to url: URL) throws {
        let data = try JSONEncoder().encode(record)
        try data.write(to: url, options: [.atomic])
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: url.path
        )
    }

    private func processIsRunning(_ processID: Int32) -> Bool {
        processID > 1 && (Darwin.kill(processID, 0) == 0 || errno == EPERM)
    }

    private func retireServiceProcess(_ processID: Int32) async throws {
        guard processIsRunning(processID) else { return }
        guard Darwin.kill(processID, SIGTERM) == 0 || errno == ESRCH else {
            throw EngineClientError.serviceHandoverTimedOut
        }
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .seconds(10))
        while processIsRunning(processID), clock.now < deadline {
            try await Task.sleep(for: .milliseconds(50))
        }
        guard !processIsRunning(processID) else {
            throw EngineClientError.serviceHandoverTimedOut
        }
    }

    private func makeServiceIdentity(engineExecutable: URL) throws -> ServiceIdentity {
        let executableData: Data
        do {
            executableData = try Data(contentsOf: engineExecutable, options: [.mappedIfSafe])
        } catch {
            throw EngineClientError.executableMissing
        }
        let digest = SHA256.hash(data: executableData)
            .map { String(format: "%02x", $0) }
            .joined()
        return ServiceIdentity(
            productVersion: Bundle.main.object(
                forInfoDictionaryKey: "LumiProductVersion"
            ) as? String ?? "unknown",
            buildNumber: Bundle.main.object(
                forInfoDictionaryKey: "CFBundleVersion"
            ) as? String ?? "unknown",
            engineExecutablePath: engineExecutable.resolvingSymlinksInPath().path,
            engineExecutableSHA256: digest
        )
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
                // A large persistent library can briefly be held by the
                // previous helper during an immediate development restart.
                // Keep startup bounded, but allow that clean handover.
                try await Task.sleep(for: .seconds(15))
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
