import Foundation

public struct EngineEndpoint: Codable, Equatable, Sendable {
    public let recordType: String
    public let host: String
    public let port: UInt16
    public let protocolVersion: Int
}
