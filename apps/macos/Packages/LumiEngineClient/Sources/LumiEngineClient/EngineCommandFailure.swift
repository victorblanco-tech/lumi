import LumiProtocol

public struct EngineCommandFailure: Error, Equatable, Sendable {
    public let kind: String
    public let code: String
    public let message: String
    public let retryable: Bool
    public let actualPlanRevision: UInt64?
    public let actualStateRevision: UInt64?
    public let actualTimelineRevision: UInt64?
    public let actualPhraseRoleRevision: UInt64?
    public let actualAutoloopCatalogRevision: UInt64?

    public init?(_ envelope: MessageEnvelope) {
        guard envelope.messageType == .error,
              case let .string(kind) = envelope.payload["kind"],
              case let .string(code) = envelope.payload["code"],
              case let .string(message) = envelope.payload["message"],
              case let .boolean(retryable) = envelope.payload["retryable"] else {
            return nil
        }
        self.kind = kind
        self.code = code
        self.message = message
        self.retryable = retryable
        if case let .number(revision) = envelope.payload["actualPlanRevision"],
           revision >= 0,
           revision.rounded(.towardZero) == revision,
           revision <= Double(UInt64.max) {
            actualPlanRevision = UInt64(revision)
        } else {
            actualPlanRevision = nil
        }
        if case let .number(revision) = envelope.payload["actualStateRevision"],
           revision >= 0,
           revision.rounded(.towardZero) == revision,
           revision <= Double(UInt64.max) {
            actualStateRevision = UInt64(revision)
        } else {
            actualStateRevision = nil
        }
        if case let .number(revision) = envelope.payload["actualTimelineRevision"],
           revision >= 0,
           revision.rounded(.towardZero) == revision,
           revision <= Double(UInt64.max) {
            actualTimelineRevision = UInt64(revision)
        } else {
            actualTimelineRevision = nil
        }
        if case let .number(revision) = envelope.payload["actualPhraseRoleRevision"],
           revision >= 0,
           revision.rounded(.towardZero) == revision,
           revision <= Double(UInt64.max) {
            actualPhraseRoleRevision = UInt64(revision)
        } else {
            actualPhraseRoleRevision = nil
        }
        if case let .number(revision) = envelope.payload["actualAutoloopCatalogRevision"],
           revision >= 0,
           revision.rounded(.towardZero) == revision,
           revision <= Double(UInt64.max) {
            actualAutoloopCatalogRevision = UInt64(revision)
        } else {
            actualAutoloopCatalogRevision = nil
        }
    }
}
