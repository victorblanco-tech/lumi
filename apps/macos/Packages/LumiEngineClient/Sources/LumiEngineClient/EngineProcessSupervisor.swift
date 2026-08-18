import Foundation
import LumiProtocol
import OSLog
import Darwin
import CryptoKit
import ServiceManagement

public actor EngineProcessSupervisor {
    private static let logger = Logger(
        subsystem: Bundle.main.bundleIdentifier ?? "co.victorblan.tech.lumi",
        category: "EngineProcessSupervisor"
    )

    private let transport: any EngineTransport
    private let launchAgentPlistName: String?
    private var process: Process?
    private var attachedProcessID: Int32?
    private var sessionToken: String?
    private var serviceRecordURL: URL?
    private var launchAgentService: SMAppService?
    private var expectedServiceIdentity: ServiceIdentity?
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

    public init(
        transport: any EngineTransport = LoopbackEngineTransport(),
        launchAgentPlistName: String? = Bundle.main.object(
            forInfoDictionaryKey: "LumiEngineLaunchAgentPlistName"
        ) as? String
    ) {
        self.transport = transport
        self.launchAgentPlistName = launchAgentPlistName
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
            if let launchAgentPlistName,
               packagedLaunchAgentExists(named: launchAgentPlistName) {
                return try await launchUsingLaunchAgent(
                    plistName: launchAgentPlistName,
                    recordURL: recordURL,
                    serviceIdentity: serviceIdentity
                )
            }
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
        // Keep the channel engine alive across sequential UI sessions. The
        // engine itself fail-safes to Off and leaves Link whenever its
        // authenticated client disconnects, while retaining stable CoreMIDI
        // endpoints for consumers such as SoundSwitch.
        environment["LUMI_EXIT_AFTER_CLIENT_DISCONNECT"] = "0"
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
        if launchAgentService != nil,
           let serviceRecordURL,
           let expectedServiceIdentity,
           let record = readServiceRecord(at: serviceRecordURL),
           record.serviceIdentity == expectedServiceIdentity,
           processIsRunning(record.processID) {
            attachedProcessID = record.processID
            sessionToken = record.sessionToken
            return true
        }
        return process?.isRunning == true || attachedProcessID.map(processIsRunning) == true
    }

    /// Disconnects this UI session without terminating the channel engine.
    ///
    /// The Rust service owns the fail-safe transition to Off on authenticated
    /// client disconnect. Keeping the process alive prevents CoreMIDI endpoint
    /// removal/recreation while a lighting application is running.
    public func detachKeepingServiceAlive() async {
        await transport.close()
        commandSequence = 0
    }

    public func stop() async {
        await transport.close()

        if launchAgentService != nil {
            process = nil
            attachedProcessID = nil
            sessionToken = nil
            commandSequence = 0
            return
        }

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

    private func launchUsingLaunchAgent(
        plistName: String,
        recordURL: URL,
        serviceIdentity: ServiceIdentity
    ) async throws -> EngineEndpoint {
        await transport.close()
        process = nil
        attachedProcessID = nil
        commandSequence = 0
        expectedServiceIdentity = serviceIdentity

        let token = try persistentSessionToken(
            at: recordURL.deletingLastPathComponent()
                .appendingPathComponent(".engine-session-token")
        )
        sessionToken = token
        let service = SMAppService.agent(plistName: plistName)
        launchAgentService = service

        if service.status == .enabled,
           let record = readServiceRecord(at: recordURL),
           record.endpoint.protocolVersion == WireProtocol.version,
           record.sessionToken == token,
           record.serviceIdentity == serviceIdentity,
           processIsRunning(record.processID) {
            attachedProcessID = record.processID
            Self.logger.info(
                "Attached to launchd-owned Lumi engine pid \(record.processID) version \(record.productVersion, privacy: .public)"
            )
            return record.endpoint
        }

        if service.status == .enabled,
           readServiceRecord(at: recordURL) == nil,
           let endpoint = try? await waitForLaunchAgentRecord(
               at: recordURL,
               sessionToken: token,
               serviceIdentity: serviceIdentity,
               timeout: .seconds(3)
           ) {
            return endpoint
        }

        switch service.status {
        case .enabled:
            let previousProcessID = readServiceRecord(at: recordURL)?.processID
            try await service.unregister()
            if let previousProcessID {
                try await waitUntilStopped(previousProcessID)
            }
            try? FileManager.default.removeItem(at: recordURL)
            try service.register()
        case .notRegistered:
            if let previousProcessID = readServiceRecord(at: recordURL)?.processID,
               processIsRunning(previousProcessID) {
                // One-time migration from the dev-43 channel-persistent child
                // process to launchd ownership. After this handover only
                // SMAppService controls the service lifecycle.
                try await retireServiceProcess(previousProcessID)
            }
            try? FileManager.default.removeItem(at: recordURL)
            try service.register()
        case .requiresApproval:
            throw EngineClientError.serviceRequiresApproval
        case .notFound:
            // On a first install macOS can report `.notFound` because the
            // Background Task Management store has no record yet, even though
            // the bundled plist and executable are present. Registration is
            // the operation that creates that record; any malformed bundle is
            // then returned as a concrete SMAppService error.
            if let previousProcessID = readServiceRecord(at: recordURL)?.processID,
               processIsRunning(previousProcessID) {
                try await retireServiceProcess(previousProcessID)
            }
            try? FileManager.default.removeItem(at: recordURL)
            do {
                try service.register()
            } catch {
                Self.logger.error(
                    "Unable to register bundled Lumi engine service: \(error.localizedDescription, privacy: .public)"
                )
                throw EngineClientError.serviceRegistrationFailed
            }
        @unknown default:
            throw EngineClientError.serviceRegistrationFailed
        }

        if service.status == .requiresApproval {
            throw EngineClientError.serviceRequiresApproval
        }
        do {
            return try await waitForLaunchAgentRecord(
                at: recordURL,
                sessionToken: token,
                serviceIdentity: serviceIdentity,
                timeout: .seconds(5)
            )
        } catch EngineClientError.startupTimedOut {
            // An ad-hoc signed local update has no stable Team ID. macOS 26
            // can accept the re-registration, reject the first spawn against
            // the cached launch constraint, and invalidate that BTM item a
            // moment later. Retry once after that bounded invalidation window
            // so unsigned Dev installs do not require a second app launch.
            Self.logger.notice(
                "LaunchAgent produced no discovery record; retrying registration once"
            )
            if service.status == .enabled {
                try await service.unregister()
            }
            try await Task.sleep(for: .milliseconds(250))
            let retryService = SMAppService.agent(plistName: plistName)
            launchAgentService = retryService
            if retryService.status == .requiresApproval {
                throw EngineClientError.serviceRequiresApproval
            }
            if retryService.status == .enabled {
                try await retryService.unregister()
                try await Task.sleep(for: .milliseconds(250))
            }
            try retryService.register()
            return try await waitForLaunchAgentRecord(
                at: recordURL,
                sessionToken: token,
                serviceIdentity: serviceIdentity
            )
        }
    }

    private func waitForLaunchAgentRecord(
        at url: URL,
        sessionToken: String,
        serviceIdentity: ServiceIdentity,
        timeout: Duration = .seconds(15)
    ) async throws -> EngineEndpoint {
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: timeout)
        while clock.now < deadline {
            if let record = readServiceRecord(at: url),
               record.endpoint.protocolVersion == WireProtocol.version,
               record.sessionToken == sessionToken,
               record.serviceIdentity == serviceIdentity,
               processIsRunning(record.processID) {
                try validate(endpoint: record.endpoint)
                attachedProcessID = record.processID
                Self.logger.info(
                    "launchd started Lumi engine pid \(record.processID) version \(record.productVersion, privacy: .public)"
                )
                return record.endpoint
            }
            try await Task.sleep(for: .milliseconds(50))
        }
        throw EngineClientError.startupTimedOut
    }

    private func waitUntilStopped(_ processID: Int32) async throws {
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .seconds(10))
        while processIsRunning(processID), clock.now < deadline {
            try await Task.sleep(for: .milliseconds(50))
        }
        guard !processIsRunning(processID) else {
            throw EngineClientError.serviceHandoverTimedOut
        }
    }

    private func persistentSessionToken(at url: URL) throws -> String {
        if let token = try? String(contentsOf: url, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines),
           (32...256).contains(token.count) {
            return token
        }
        let token = try SessionTokenGenerator.generate()
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data(token.utf8).write(to: url, options: [.atomic])
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: url.path
        )
        return token
    }

    private func packagedLaunchAgentExists(named plistName: String) -> Bool {
        let url = Bundle.main.bundleURL
            .appendingPathComponent("Contents")
            .appendingPathComponent("Library")
            .appendingPathComponent("LaunchAgents")
            .appendingPathComponent(plistName)
        return FileManager.default.fileExists(atPath: url.path)
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
