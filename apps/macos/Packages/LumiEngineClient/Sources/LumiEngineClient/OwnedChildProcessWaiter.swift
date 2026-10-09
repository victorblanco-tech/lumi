import Foundation
import Darwin

public enum OwnedChildProcessWaiterError: Error, Sendable {
    case timedOut
    case terminationFailed(OwnedChildProcessWaiter)
}

/// Waits only for a Process launched and retained by this application. Cancel
/// and timeout both reap it before returning; no process-name/global killing.
public final class OwnedChildProcessWaiter: @unchecked Sendable {
    private let process: Process
    private let lock = NSLock()
    private var reaped = false

    public var hasExited: Bool { lock.withLock { reaped } }

    public init(process: Process) { self.process = process }

    public func wait(timeout: Duration) async throws -> Int32 {
        // Foundation's run-loop notification can lag on detached workers.
        // waitUntilExit, not isRunning, is the completion authority.
        DispatchQueue.global(qos: .utility).async { [self] in
            process.waitUntilExit()
            lock.withLock { reaped = true }
        }
        let deadline = ContinuousClock.now.advanced(by: timeout)
        do {
            while !hasExited {
                try Task.checkCancellation()
                guard ContinuousClock.now < deadline else { throw OwnedChildProcessWaiterError.timedOut }
                try await Task.sleep(for: .milliseconds(20))
            }
            try Task.checkCancellation()
            return process.terminationStatus
        } catch {
            // Cleanup must not inherit the cancellation that brought us here.
            try await Task.detached(priority: .utility) { [self] in
                guard !hasExited else { return }
                _ = Darwin.kill(process.processIdentifier, SIGTERM)
                let gracefulDeadline = ContinuousClock.now.advanced(by: .milliseconds(250))
                while !hasExited, ContinuousClock.now < gracefulDeadline {
                    try await Task.sleep(for: .milliseconds(10))
                }
                if !hasExited { _ = Darwin.kill(process.processIdentifier, SIGKILL) }
                let forcedDeadline = ContinuousClock.now.advanced(by: .seconds(2))
                while !hasExited, ContinuousClock.now < forcedDeadline {
                    try await Task.sleep(for: .milliseconds(10))
                }
                guard hasExited else { throw OwnedChildProcessWaiterError.terminationFailed(self) }
            }.value
            throw error
        }
    }
}
