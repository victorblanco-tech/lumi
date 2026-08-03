import Foundation
import LumiProtocol

public struct EnginePlanCommandContext: Equatable, Sendable {
    public let planID: String
    public let trackLoadID: UInt64
    public let expectedPlanRevision: UInt64

    public init(planID: String, trackLoadID: UInt64, expectedPlanRevision: UInt64) {
        self.planID = planID
        self.trackLoadID = trackLoadID
        self.expectedPlanRevision = expectedPlanRevision
    }
}

public enum EnginePlanCommand: Equatable, Sendable {
    case selectTheme(context: EnginePlanCommandContext, themeID: UInt64)
    case selectScene(
        context: EnginePlanCommandContext,
        phraseIndex: UInt64,
        sceneID: UInt64
    )
    case setCueLock(
        context: EnginePlanCommandContext,
        phraseIndex: UInt64,
        locked: Bool
    )
    case regeneratePlan(context: EnginePlanCommandContext)

    func payload() -> [String: JSONValue] {
        var payload = contextPayload
        switch self {
        case let .selectTheme(_, themeID):
            payload["kind"] = .string("selectTheme")
            payload["themeId"] = .number(Double(themeID))
        case let .selectScene(_, phraseIndex, sceneID):
            payload["kind"] = .string("selectScene")
            payload["phraseIndex"] = .number(Double(phraseIndex))
            payload["sceneId"] = .number(Double(sceneID))
        case let .setCueLock(_, phraseIndex, locked):
            payload["kind"] = .string("setCueLock")
            payload["phraseIndex"] = .number(Double(phraseIndex))
            payload["locked"] = .boolean(locked)
        case .regeneratePlan:
            payload["kind"] = .string("regeneratePlan")
        }
        return payload
    }

    private var contextPayload: [String: JSONValue] {
        let context = switch self {
        case let .selectTheme(context, _),
             let .selectScene(context, _, _),
             let .setCueLock(context, _, _),
             let .regeneratePlan(context):
            context
        }
        return [
            "planId": .string(context.planID),
            "trackLoadId": .number(Double(context.trackLoadID)),
            "expectedPlanRevision": .number(Double(context.expectedPlanRevision))
        ]
    }
}
