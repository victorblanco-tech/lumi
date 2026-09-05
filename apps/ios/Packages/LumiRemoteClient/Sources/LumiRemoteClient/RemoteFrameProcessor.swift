import Foundation
import LumiProtocol
import OSLog

public enum RemoteFrameProcessingDecision: Equatable, Sendable {
    case applied
    case duplicateIgnored
    case snapshotRequired(expected: UInt64, received: UInt64)
    case authoritativeSnapshotRequired
    case unrelated
}

@MainActor
public final class RemoteFrameProcessor {
    private let logger = Logger(subsystem: "co.victorblan.tech.lumi.remote", category: "CommandResult")
    private let model: RemoteSessionModel
    private let decoder: RemoteFrameDecoder
    private var sequenceTracker = SequenceTracker()
    private var awaitsSnapshot = true
    private var acceptsRecoveryProjection = false
    private var macName: String

    public init(
        model: RemoteSessionModel,
        macName: String,
        decoder: RemoteFrameDecoder = .init()
    ) {
        self.model = model
        self.macName = macName
        self.decoder = decoder
    }

    public func reset(for macName: String) {
        self.macName = macName
        sequenceTracker.reset()
        awaitsSnapshot = true
        acceptsRecoveryProjection = false
        model.awaitingSnapshot(from: macName)
    }

    public func process(_ data: Data) throws -> RemoteFrameProcessingDecision {
        let frame = try decoder.decodeFrame(data)

        if frame.frameKind == .error, frame.correlationID == nil,
           case let .object(payload) = frame.payload,
           case let .string(reason)? = payload["reasonCode"],
           reason == "engineUnavailable" || reason == "connectedPlayersUnavailable" {
            awaitsSnapshot = true
            acceptsRecoveryProjection = true
            model.awaitingSnapshot(from: macName)
            // Do not loop snapshot requests while the engine is unavailable.
            // Its first complete replacement projection is authoritative.
            return .applied
        }

        if awaitsSnapshot {
            guard frame.frameKind == .snapshot ||
                    (acceptsRecoveryProjection && frame.frameKind == .projection) else { return .unrelated }
            sequenceTracker.reset()
            _ = sequenceTracker.observe(frame.sequence)
            let projection = try decoder.decodeProjection(frame)
            model.replaceWithSnapshot(projection, from: macName)
            awaitsSnapshot = false
            acceptsRecoveryProjection = false
            return .applied
        }

        switch sequenceTracker.observe(frame.sequence) {
        case .duplicate:
            return .duplicateIgnored
        case let .requestSnapshot(expected, received):
            awaitsSnapshot = true
            model.awaitingSnapshot(from: macName)
            return .snapshotRequired(expected: expected, received: received)
        case .accepted:
            break
        }

        switch frame.frameKind {
        case .snapshot:
            model.replaceWithSnapshot(try decoder.decodeProjection(frame), from: macName)
            return .applied
        case .projection:
            try model.apply(try decoder.decodeProjection(frame), from: macName)
            return .applied
        case .transportAnchor:
            let update = try decoder.decodeTransportAnchor(frame)
            try model.applyTransportAnchor(
                playerNumber: update.playerNumber,
                anchor: update.anchor
            )
            return .applied
        case .commandResult:
            let result = try decoder.decodeCommandResult(frame)
            logger.notice("Command outcome: \(result.status.rawValue, privacy: .public), reason: \(result.reasonCode ?? "none", privacy: .public)")
            switch result.status {
            case .accepted:
                model.acknowledgeCommand(result.commandID)
            case .duplicate:
                // Older gateways acknowledged admission, not execution.
                model.rejectCommand(result.commandID, reason: "Confirmation is unavailable. Refreshing the show state.")
                awaitsSnapshot = true
                model.awaitingSnapshot(from: macName)
                return .authoritativeSnapshotRequired
            case .conflict:
                model.rejectCommand(
                    result.commandID,
                    reason: result.reasonCode == "timingOffsetConflict"
                        ? "Lighting timing changed on the Mac. Check the current value and try again."
                        : "The show changed on the Mac. Refresh before trying again."
                )
                awaitsSnapshot = true
                model.awaitingSnapshot(from: macName)
                return .authoritativeSnapshotRequired
            case .rejected:
                if result.reasonCode == "commandOutcomeUnknown" || result.reasonCode == "commandOutcomePending" {
                    model.rejectCommand(result.commandID, reason: "Confirmation is unavailable. Refreshing the show state.")
                    awaitsSnapshot = true
                    model.awaitingSnapshot(from: macName)
                    return .authoritativeSnapshotRequired
                }
                model.rejectCommand(
                    result.commandID,
                    reason: "The Mac rejected the requested change."
                )
            }
            return .applied
        case .error:
            if let commandID = frame.correlationID {
                model.rejectCommand(commandID, reason: "The Mac rejected the requested change.")
                return .applied
            }
            return .unrelated
        case .hello, .command:
            return .unrelated
        }
    }
}
