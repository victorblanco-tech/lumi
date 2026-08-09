/// The action a client takes after observing an event sequence.
public enum SequenceDecision: Equatable, Sendable {
    case accepted
    case duplicate
    case requestSnapshot(expected: UInt64, received: UInt64)
}

/// Tracks monotonic engine event sequences for one connection.
public struct SequenceTracker: Sendable {
    private var lastAcceptedSequence: UInt64?

    public init() {}

    public mutating func observe(_ sequence: UInt64) -> SequenceDecision {
        guard let lastAcceptedSequence else {
            self.lastAcceptedSequence = sequence
            return .accepted
        }

        if sequence <= lastAcceptedSequence {
            return .duplicate
        }

        let expected = lastAcceptedSequence + 1
        guard sequence == expected else {
            return .requestSnapshot(expected: expected, received: sequence)
        }

        self.lastAcceptedSequence = sequence
        return .accepted
    }

    public mutating func reset() {
        lastAcceptedSequence = nil
    }
}
