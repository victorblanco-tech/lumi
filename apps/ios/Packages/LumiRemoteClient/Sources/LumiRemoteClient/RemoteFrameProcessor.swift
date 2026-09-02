import Foundation
import LumiProtocol

public enum RemoteFrameProcessingDecision: Equatable, Sendable {
    case applied
    case duplicateIgnored
    case snapshotRequired(expected: UInt64, received: UInt64)
    case unrelated
}

@MainActor
public final class RemoteFrameProcessor {
    private let model: RemoteSessionModel
    private let decoder: RemoteFrameDecoder
    private var sequenceTracker = SequenceTracker()
    private var awaitsSnapshot = true
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
    }

    public func process(_ data: Data) throws -> RemoteFrameProcessingDecision {
        let frame = try decoder.decodeFrame(data)

        if awaitsSnapshot {
            guard frame.frameKind == .snapshot else { return .unrelated }
            sequenceTracker.reset()
            _ = sequenceTracker.observe(frame.sequence)
            let projection = try decoder.decodeProjection(frame)
            model.replaceWithSnapshot(projection, from: macName)
            awaitsSnapshot = false
            return .applied
        }

        switch sequenceTracker.observe(frame.sequence) {
        case .duplicate:
            return .duplicateIgnored
        case let .requestSnapshot(expected, received):
            awaitsSnapshot = true
            model.reconnecting(to: macName)
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
            switch result.status {
            case .accepted, .duplicate:
                model.acknowledgeCommand(result.commandID)
            case .conflict:
                model.rejectCommand(
                    result.commandID,
                    reason: "The show changed on the Mac. Refresh before trying again."
                )
            case .rejected:
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
