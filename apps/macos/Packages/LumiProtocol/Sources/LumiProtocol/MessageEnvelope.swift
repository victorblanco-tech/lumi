import Foundation

public enum MessageType: String, Codable, Equatable, Sendable {
    case command
    case snapshot
    case event
    case error
}

/// Transport-independent protocol v1 envelope.
///
/// Client boundaries map this DTO into presentation state instead of using it
/// as authoritative application state.
public struct MessageEnvelope: Codable, Equatable, Sendable {
    public let protocolVersion: Int
    public let messageType: MessageType
    public let messageId: String
    public let sequence: UInt64
    public let correlationId: String
    public let sentAt: String
    public let payload: [String: JSONValue]

    public init(
        protocolVersion: Int,
        messageType: MessageType,
        messageId: String,
        sequence: UInt64,
        correlationId: String,
        sentAt: String,
        payload: [String: JSONValue]
    ) {
        self.protocolVersion = protocolVersion
        self.messageType = messageType
        self.messageId = messageId
        self.sequence = sequence
        self.correlationId = correlationId
        self.sentAt = sentAt
        self.payload = payload
    }
}
