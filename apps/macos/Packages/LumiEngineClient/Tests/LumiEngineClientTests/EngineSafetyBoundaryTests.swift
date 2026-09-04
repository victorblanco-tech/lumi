import Darwin
import Dispatch
import Foundation
import LumiProtocol
import Testing
@testable import LumiEngineClient

@Suite("Engine safety boundaries")
struct EngineSafetyBoundaryTests {
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
