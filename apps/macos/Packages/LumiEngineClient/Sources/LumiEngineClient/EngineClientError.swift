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
    case connectionTimedOut
    case authenticationFailed
    case requestTimedOut
    case connectionClosed
    case invalidInitialSnapshot
    case commandSequenceOverflow
    case serviceHandoverTimedOut
    case serviceRequiresApproval
    case serviceDefinitionMissing
    case serviceRegistrationFailed

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
        case .connectionTimedOut:
            "The local Lumi engine did not accept the connection in time."
        case .authenticationFailed:
            "The local Lumi engine rejected session authentication."
        case .requestTimedOut:
            "The local Lumi engine did not answer in time. Lumi will reconnect safely."
        case .connectionClosed:
            "The local Lumi engine connection closed unexpectedly."
        case .invalidInitialSnapshot:
            "The Lumi engine did not provide a valid initial snapshot."
        case .commandSequenceOverflow:
            "The local command sequence overflowed. Restart Lumi before sending more edits."
        case .serviceHandoverTimedOut:
            "The previous Lumi engine did not stop safely. Close other Lumi versions and try again."
        case .serviceRequiresApproval:
            "Lumi Engine requires approval in System Settings > General > Login Items. No administrator account is required."
        case .serviceDefinitionMissing:
            "The bundled Lumi Engine background-service definition is missing. Reinstall this Lumi build."
        case .serviceRegistrationFailed:
            "macOS could not register the Lumi Engine background service."
        }
    }
}
