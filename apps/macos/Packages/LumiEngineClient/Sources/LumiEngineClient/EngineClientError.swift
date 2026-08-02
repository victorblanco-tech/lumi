import Foundation

public enum EngineClientError: Error, Equatable, LocalizedError, Sendable {
    case executableMissing
    case processLaunchFailed
    case startupTimedOut
    case invalidStartupRecord
    case nonLoopbackEndpoint
    case protocolMismatch(expected: Int, received: Int)
    case secureTokenGenerationFailed
    case connectionFailed
    case authenticationFailed
    case connectionClosed
    case invalidInitialSnapshot

    public var errorDescription: String? {
        switch self {
        case .executableMissing:
            "The bundled Lumi engine could not be found."
        case .processLaunchFailed:
            "The Lumi engine could not be started."
        case .startupTimedOut:
            "The Lumi engine did not become ready in time."
        case .invalidStartupRecord:
            "The Lumi engine returned invalid startup information."
        case .nonLoopbackEndpoint:
            "The Lumi engine refused a non-local endpoint."
        case let .protocolMismatch(expected, received):
            "Protocol mismatch: expected v\(expected), received v\(received)."
        case .secureTokenGenerationFailed:
            "A secure local session could not be created."
        case .connectionFailed:
            "The app could not connect to the local Lumi engine."
        case .authenticationFailed:
            "The local Lumi engine rejected session authentication."
        case .connectionClosed:
            "The local Lumi engine connection closed unexpectedly."
        case .invalidInitialSnapshot:
            "The Lumi engine did not provide a valid initial snapshot."
        }
    }
}
