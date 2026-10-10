import Darwin
import Dispatch
import Foundation
import LumiProtocol
import Testing
@testable import LumiEngineClient

@Suite("Engine safety boundaries")
struct EngineSafetyBoundaryTests {
    @Test("Remote details reject a reused PID or a helper from another installation")
    func remoteDetailsValidateExecutable() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("gateway.json")
        let record = RemoteGatewayServiceRecord(
            endpointHost: "127.0.0.1", endpointPort: 12345,
            adminToken: String(repeating: "a", count: 32), processID: getpid(),
            productVersion: "test", installationID: String(repeating: "b", count: 32),
            certificateFingerprintSHA256: String(repeating: "c", count: 64), lanPort: 12346
        )
        try JSONEncoder().encode(record).write(to: url)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        let wrong = RemoteGatewaySupervisor(
            launchAgentPlistName: nil, expectedProductVersion: "test",
            expectedExecutableURL: directory.appendingPathComponent("another-installation/gateway")
        )
        #expect(await wrong.processDetails(recordURL: url) == "No verified Remote process")
        let actualPath = try #require(ProcessExecutableIdentity.path(processID: getpid()))
        let matching = RemoteGatewaySupervisor(
            launchAgentPlistName: nil, expectedProductVersion: "test",
            expectedExecutableURL: URL(fileURLWithPath: actualPath)
        )
        #expect(await matching.processDetails(recordURL: url).contains("PID \(getpid())"))
    }

    @Test("Cancelling or timing out an isolated worker waits until its child is gone", arguments: [false, true])
    func isolatedWorkerCleanup(cancel: Bool) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let executable = directory.appendingPathComponent("worker")
        try Data("#!/bin/sh\ntrap '' TERM\nexec sleep 30\n".utf8).write(to: executable)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: executable.path)
        let process = Process()
        process.executableURL = executable
        try process.run()
        let waiter = OwnedChildProcessWaiter(process: process)
        let task = Task.detached { try await waiter.wait(timeout: cancel ? .seconds(30) : .milliseconds(50)) }
        if cancel {
            try await Task.sleep(for: .milliseconds(50))
            task.cancel()
        }
        do {
            _ = try await task.value
            Issue.record("Sleeping worker unexpectedly completed normally")
        } catch is CancellationError {
            #expect(cancel)
        } catch OwnedChildProcessWaiterError.timedOut {
            #expect(!cancel)
        }
        #expect(!process.isRunning)
        #expect(Darwin.kill(process.processIdentifier, 0) != 0)
        #expect(errno == ESRCH)
    }

    @Test("An owned child that exits before its waiter is created retains its exit status")
    func alreadyExitedWorker() async throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", "exit 7"]
        try process.run()
        let deadline = ContinuousClock.now.advanced(by: .seconds(3))
        while process.isRunning, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(!process.isRunning)
        let waiter = OwnedChildProcessWaiter(process: process)
        #expect(try await waiter.wait(timeout: .seconds(1)) == 7)
        #expect(waiter.hasExited)
    }

    @Test("Cancelled Remote startup does not proceed to service registration")
    func cancelledRemoteStartup() async {
        let supervisor = RemoteGatewaySupervisor(launchAgentPlistName: nil)
        let task = Task {
            withUnsafeCurrentTask { $0?.cancel() }
            do {
                _ = try await supervisor.enable(recordURL: URL(fileURLWithPath: "/unused"))
                Issue.record("Cancelled startup unexpectedly succeeded")
            } catch is CancellationError {
                // Cancellation must win even before packaging/registration checks.
            } catch {
                Issue.record("Startup proceeded after cancellation: \(error)")
            }
            #expect(await supervisor.refresh(recordURL: URL(fileURLWithPath: "/unused")) == .disabled)
        }
        await task.value
    }

    @Test("Old Remote record is rejected for commands but remains readable for shutdown")
    func oldRemoteRecordShutdownValidation() throws {
        let record = RemoteGatewayServiceRecord(
            endpointHost: "127.0.0.1", endpointPort: 12345,
            adminToken: String(repeating: "a", count: 32), processID: 123,
            productVersion: "0.6.4-dev-19", installationID: String(repeating: "b", count: 32),
            certificateFingerprintSHA256: String(repeating: "c", count: 64), lanPort: 12346
        )
        #expect(throws: RemoteGatewayClientError.serviceVersionMismatch) {
            try record.validate(expectedProductVersion: "0.6.4-dev-20")
        }
        try record.validate(expectedProductVersion: nil)
    }

    @Test("Explicit shutdown terminates an owned process, including ignored TERM, and permits relaunch", arguments: [false, true])
    func explicitShutdownDoesNotParkProcess(ignoresTermination: Bool) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let executable = directory.appendingPathComponent("fake-engine")
        let script = """
        #!/bin/sh
        \(ignoresTermination ? "trap '' TERM" : "")
        printf '%s\\n' '{"recordType":"engineReady","host":"127.0.0.1","port":54321,"protocolVersion":\(WireProtocol.version)}'
        exec sleep 30
        """
        try Data(script.utf8).write(to: executable)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: executable.path)
        let supervisor = EngineProcessSupervisor(launchAgentPlistName: nil)
        _ = try await supervisor.launch(engineExecutable: executable, automaticallyPublishesMidi: false)
        #expect(await supervisor.isRunning())
        let started = ContinuousClock.now
        try await supervisor.shutdown()
        #expect(started.duration(to: .now) < .seconds(9))
        #expect(await !supervisor.isRunning())
        try await supervisor.shutdown()
        _ = try await supervisor.launch(engineExecutable: executable, automaticallyPublishesMidi: false)
        #expect(await supervisor.isRunning())
        try await supervisor.shutdown()
        #expect(await !supervisor.isRunning())
    }

    @Test("A silent transport operation reaches its bounded deadline")
    func transportDeadlineFails() async {
        let started = ContinuousClock.now
        do {
            let _: Void = try await withCheckedThrowingContinuation { continuation in
                let gate = DeadlineContinuationGate<Void>(continuation)
                gate.arm(
                    on: DispatchQueue(label: "lumi.engine-timeout-test"),
                    after: 0.02,
                    error: EngineClientError.requestTimedOut,
                    onTimeout: {}
                )
            }
            Issue.record("The silent operation unexpectedly completed")
        } catch {
            #expect(error as? EngineClientError == .requestTimedOut)
        }
        #expect(started.duration(to: .now) < .seconds(1))
    }

    @Test("An unrelated executable never matches a recorded engine PID")
    func executableIdentityIsVerified() throws {
        let processID = getpid()
        let actualPath = try #require(ProcessExecutableIdentity.path(processID: processID))

        #expect(
            ProcessExecutableIdentity.matches(
                processID: processID,
                expectedPath: actualPath
            )
        )
        #expect(
            !ProcessExecutableIdentity.matches(
                processID: processID,
                expectedPath: "/Applications/Definitely Not Lumi.app/Contents/MacOS/LumiEngine"
            )
        )
    }

    @Test("A transient local engine connection failure is retried")
    func transientConnectionIsRetried() async throws {
        let transport = ScriptedEngineTransport(connectionFailures: 2)
        let supervisor = EngineProcessSupervisor(
            transport: transport,
            launchAgentPlistName: nil,
            connectionRetryDelays: [.milliseconds(1), .milliseconds(1)],
            sessionTokenForTesting: "test-session-token"
        )
        let endpoint = EngineEndpoint(
            recordType: "engineReady",
            host: "127.0.0.1",
            port: 49_151,
            protocolVersion: WireProtocol.version
        )

        let snapshot = try await supervisor.connect(to: endpoint)
        let attempts = await transport.connectionAttempts
        let authentications = await transport.authenticationAttempts
        let closes = await transport.closeCount

        #expect(snapshot.messageType == .snapshot)
        #expect(attempts == 3)
        #expect(authentications == 1)
        #expect(closes == 2)
    }

    @Test("A non-transient authentication failure is not retried")
    func authenticationFailureIsNotRetried() async {
        let transport = ScriptedEngineTransport(
            connectionFailures: 0,
            authenticationError: .authenticationFailed
        )
        let supervisor = EngineProcessSupervisor(
            transport: transport,
            launchAgentPlistName: nil,
            connectionRetryDelays: [.milliseconds(1), .milliseconds(1)],
            sessionTokenForTesting: "test-session-token"
        )
        let endpoint = EngineEndpoint(
            recordType: "engineReady",
            host: "127.0.0.1",
            port: 49_151,
            protocolVersion: WireProtocol.version
        )

        await #expect(throws: EngineClientError.authenticationFailed) {
            try await supervisor.connect(to: endpoint)
        }
        let attempts = await transport.connectionAttempts
        let authentications = await transport.authenticationAttempts

        #expect(attempts == 1)
        #expect(authentications == 1)
    }
}

private actor ScriptedEngineTransport: EngineTransport {
    private var remainingConnectionFailures: Int
    private let authenticationError: EngineClientError?
    private(set) var connectionAttempts = 0
    private(set) var authenticationAttempts = 0
    private(set) var closeCount = 0

    init(
        connectionFailures: Int,
        authenticationError: EngineClientError? = nil
    ) {
        remainingConnectionFailures = connectionFailures
        self.authenticationError = authenticationError
    }

    func connect(to endpoint: EngineEndpoint) async throws {
        connectionAttempts += 1
        if remainingConnectionFailures > 0 {
            remainingConnectionFailures -= 1
            throw EngineClientError.connectionFailed
        }
    }

    func authenticate(sessionToken: String) async throws -> MessageEnvelope {
        authenticationAttempts += 1
        if let authenticationError {
            throw authenticationError
        }
        return MessageEnvelope(
            protocolVersion: WireProtocol.version,
            messageType: .snapshot,
            messageId: "snapshot-1",
            sequence: 1,
            correlationId: "session-bootstrap",
            sentAt: "2026-09-03T20:00:00Z",
            payload: [:]
        )
    }

    func exchange(_ envelope: MessageEnvelope) async throws -> MessageEnvelope {
        envelope
    }

    func close() async {
        closeCount += 1
    }
}
