import Foundation

public enum WireProtocol {
    public static let version = 1
    public static let maximumMessageBytes = 65_536
}

public enum ProtocolDecodingError: Error, Equatable, Sendable {
    case oversized(actual: Int, maximum: Int)
    case malformed
    case unsupportedProtocolVersion(Int)
    case invalidField(String)
}

public enum ProtocolMessageDecoder {
    public static func decode(_ data: Data) throws -> MessageEnvelope {
        guard data.count <= WireProtocol.maximumMessageBytes else {
            throw ProtocolDecodingError.oversized(
                actual: data.count,
                maximum: WireProtocol.maximumMessageBytes
            )
        }

        let envelope: MessageEnvelope
        do {
            envelope = try JSONDecoder().decode(MessageEnvelope.self, from: data)
        } catch {
            throw ProtocolDecodingError.malformed
        }

        guard envelope.protocolVersion == WireProtocol.version else {
            throw ProtocolDecodingError.unsupportedProtocolVersion(envelope.protocolVersion)
        }
        guard (1...128).contains(envelope.messageId.utf8.count) else {
            throw ProtocolDecodingError.invalidField("messageId")
        }
        guard (1...128).contains(envelope.correlationId.utf8.count) else {
            throw ProtocolDecodingError.invalidField("correlationId")
        }
        guard !envelope.sentAt.isEmpty else {
            throw ProtocolDecodingError.invalidField("sentAt")
        }

        return envelope
    }
}
