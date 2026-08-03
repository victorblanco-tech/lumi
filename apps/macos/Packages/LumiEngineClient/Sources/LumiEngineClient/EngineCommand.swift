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

public enum EngineCommand: Equatable, Sendable {
    case queryLibrary(search: String, playlistID: UInt64?, offset: UInt32, limit: UInt16)
    case loadDemoSession(expectedStateRevision: UInt64)
    case setOperationState(String, expectedStateRevision: UInt64)
    case setSimulationSpeed(UInt64, expectedStateRevision: UInt64)
    case setSimulationPlayback(Bool, expectedStateRevision: UInt64)
    case advanceSimulation(elapsedTicks: UInt64, expectedStateRevision: UInt64)
    case advanceToNextTrack(expectedStateRevision: UInt64)
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
    case resetDemoSession(expectedStateRevision: UInt64)

    func payload() -> [String: JSONValue] {
        switch self {
        case let .queryLibrary(search, playlistID, offset, limit):
            var payload: [String: JSONValue] = [
                "kind": .string("queryLibrary"),
                "search": .string(search),
                "offset": .number(Double(offset)),
                "limit": .number(Double(limit))
            ]
            payload["playlistId"] = playlistID.map { .number(Double($0)) } ?? .null
            return payload
        case let .loadDemoSession(expectedRevision):
            return statePayload("loadDemoSession", expectedRevision: expectedRevision)
        case let .setOperationState(state, expectedRevision):
            return statePayload(
                "setOperationState",
                expectedRevision: expectedRevision,
                additional: ["operationState": .string(state)]
            )
        case let .setSimulationSpeed(speed, expectedRevision):
            return statePayload(
                "setSimulationSpeed",
                expectedRevision: expectedRevision,
                additional: ["speed": .number(Double(speed))]
            )
        case let .setSimulationPlayback(playing, expectedRevision):
            return statePayload(
                "setSimulationPlayback",
                expectedRevision: expectedRevision,
                additional: ["playing": .boolean(playing)]
            )
        case let .advanceSimulation(elapsedTicks, expectedRevision):
            return statePayload(
                "advanceSimulation",
                expectedRevision: expectedRevision,
                additional: ["elapsedTicks": .number(Double(elapsedTicks))]
            )
        case let .advanceToNextTrack(expectedRevision):
            return statePayload("advanceToNextTrack", expectedRevision: expectedRevision)
        case let .selectTheme(context, themeID):
            return planPayload(
                "selectTheme",
                context: context,
                additional: ["themeId": .number(Double(themeID))]
            )
        case let .selectScene(context, phraseIndex, sceneID):
            return planPayload(
                "selectScene",
                context: context,
                additional: [
                    "phraseIndex": .number(Double(phraseIndex)),
                    "sceneId": .number(Double(sceneID))
                ]
            )
        case let .setCueLock(context, phraseIndex, locked):
            return planPayload(
                "setCueLock",
                context: context,
                additional: [
                    "phraseIndex": .number(Double(phraseIndex)),
                    "locked": .boolean(locked)
                ]
            )
        case let .regeneratePlan(context):
            return planPayload("regeneratePlan", context: context)
        case let .resetDemoSession(expectedRevision):
            return statePayload("resetDemoSession", expectedRevision: expectedRevision)
        }
    }

    private func statePayload(
        _ kind: String,
        expectedRevision: UInt64,
        additional: [String: JSONValue] = [:]
    ) -> [String: JSONValue] {
        var payload = additional
        payload["kind"] = .string(kind)
        payload["expectedStateRevision"] = .number(Double(expectedRevision))
        return payload
    }

    private func planPayload(
        _ kind: String,
        context: EnginePlanCommandContext,
        additional: [String: JSONValue] = [:]
    ) -> [String: JSONValue] {
        var payload = additional
        payload["kind"] = .string(kind)
        payload["planId"] = .string(context.planID)
        payload["trackLoadId"] = .number(Double(context.trackLoadID))
        payload["expectedPlanRevision"] = .number(Double(context.expectedPlanRevision))
        return payload
    }
}
