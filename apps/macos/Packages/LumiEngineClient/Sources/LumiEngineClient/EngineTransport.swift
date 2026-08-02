import Foundation
import LumiProtocol

public protocol EngineTransport: Sendable {
    func connect(to endpoint: EngineEndpoint) async throws
    func authenticate(sessionToken: String) async throws -> MessageEnvelope
    func close() async
}
