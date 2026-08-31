import Darwin
import Dispatch
import Foundation
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
                    error: .requestTimedOut,
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
}
