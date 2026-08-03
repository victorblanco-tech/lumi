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

public enum EngineTimelineEdit: Equatable, Sendable {
    case create(startBar: UInt32, endBar: UInt32, roleID: String)
    case split(phraseIndex: UInt16, atBar: UInt32)
    case mergePrevious(phraseIndex: UInt16)
    case mergeNext(phraseIndex: UInt16)
    case moveBoundary(afterPhraseIndex: UInt16, toBar: UInt32)
    case deleteAbsorbPrevious(phraseIndex: UInt16)
    case deleteAbsorbNext(phraseIndex: UInt16)
    case changeRole(phraseIndex: UInt16, roleID: String)

    fileprivate var payload: [String: JSONValue] {
        switch self {
        case let .create(startBar, endBar, roleID):
            return [
                "operation": .string("create"),
                "startBar": .number(Double(startBar)),
                "endBar": .number(Double(endBar)),
                "roleId": .string(roleID)
            ]
        case let .split(phraseIndex, atBar):
            return [
                "operation": .string("split"),
                "phraseIndex": .number(Double(phraseIndex)),
                "atBar": .number(Double(atBar))
            ]
        case let .mergePrevious(phraseIndex):
            return indexed("mergePrevious", phraseIndex: phraseIndex)
        case let .mergeNext(phraseIndex):
            return indexed("mergeNext", phraseIndex: phraseIndex)
        case let .moveBoundary(phraseIndex, toBar):
            return [
                "operation": .string("moveBoundary"),
                "phraseIndex": .number(Double(phraseIndex)),
                "toBar": .number(Double(toBar))
            ]
        case let .deleteAbsorbPrevious(phraseIndex):
            return indexed("deleteAbsorbPrevious", phraseIndex: phraseIndex)
        case let .deleteAbsorbNext(phraseIndex):
            return indexed("deleteAbsorbNext", phraseIndex: phraseIndex)
        case let .changeRole(phraseIndex, roleID):
            return [
                "operation": .string("changeRole"),
                "phraseIndex": .number(Double(phraseIndex)),
                "roleId": .string(roleID)
            ]
        }
    }

    private func indexed(_ operation: String, phraseIndex: UInt16) -> [String: JSONValue] {
        [
            "operation": .string(operation),
            "phraseIndex": .number(Double(phraseIndex))
        ]
    }
}

public enum EnginePhraseRoleMutation: Equatable, Sendable {
    case add(displayName: String)
    case rename(roleID: String, displayName: String)
    case moveEarlier(roleID: String)
    case moveLater(roleID: String)
    case archive(roleID: String)
    case restore(roleID: String)
    case setSourceMapping(providerKind: String, rawLabel: String, roleID: String)

    fileprivate var payload: [String: JSONValue] {
        switch self {
        case let .add(displayName):
            ["operation": .string("add"), "displayName": .string(displayName)]
        case let .rename(roleID, displayName):
            [
                "operation": .string("rename"),
                "roleId": .string(roleID),
                "displayName": .string(displayName)
            ]
        case let .moveEarlier(roleID):
            rolePayload("moveEarlier", roleID: roleID)
        case let .moveLater(roleID):
            rolePayload("moveLater", roleID: roleID)
        case let .archive(roleID):
            rolePayload("archive", roleID: roleID)
        case let .restore(roleID):
            rolePayload("restore", roleID: roleID)
        case let .setSourceMapping(providerKind, rawLabel, roleID):
            [
                "operation": .string("setSourceMapping"),
                "providerKind": .string(providerKind),
                "rawLabel": .string(rawLabel),
                "roleId": .string(roleID)
            ]
        }
    }

    private func rolePayload(_ operation: String, roleID: String) -> [String: JSONValue] {
        ["operation": .string(operation), "roleId": .string(roleID)]
    }
}

public enum EngineCommand: Equatable, Sendable {
    case queryLibrary(search: String, playlistID: UInt64?, offset: UInt32, limit: UInt16)
    case openLibraryTrackEditor(trackID: UInt64)
    case closeLibraryTrackEditor
    case editLibraryTimeline(
        trackID: UInt64,
        expectedTimelineRevision: UInt64,
        edit: EngineTimelineEdit
    )
    case undoLibraryTimeline(trackID: UInt64, expectedTimelineRevision: UInt64)
    case redoLibraryTimeline(trackID: UInt64, expectedTimelineRevision: UInt64)
    case restoreLibraryTimelineRevision(
        trackID: UInt64,
        expectedTimelineRevision: UInt64,
        targetTimelineRevision: UInt64
    )
    case mutatePhraseRoleCatalog(
        expectedPhraseRoleRevision: UInt64,
        mutation: EnginePhraseRoleMutation
    )
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
        case let .openLibraryTrackEditor(trackID):
            return [
                "kind": .string("openLibraryTrackEditor"),
                "trackId": .number(Double(trackID))
            ]
        case .closeLibraryTrackEditor:
            return ["kind": .string("closeLibraryTrackEditor")]
        case let .editLibraryTimeline(trackID, expectedRevision, edit):
            var payload = edit.payload
            payload["kind"] = .string("editLibraryTimeline")
            payload["trackId"] = .number(Double(trackID))
            payload["expectedTimelineRevision"] = .number(Double(expectedRevision))
            return payload
        case let .undoLibraryTimeline(trackID, expectedRevision):
            return timelinePayload(
                "undoLibraryTimeline",
                trackID: trackID,
                expectedRevision: expectedRevision
            )
        case let .redoLibraryTimeline(trackID, expectedRevision):
            return timelinePayload(
                "redoLibraryTimeline",
                trackID: trackID,
                expectedRevision: expectedRevision
            )
        case let .restoreLibraryTimelineRevision(trackID, expectedRevision, targetRevision):
            var payload = timelinePayload(
                "restoreLibraryTimelineRevision",
                trackID: trackID,
                expectedRevision: expectedRevision
            )
            payload["targetTimelineRevision"] = .number(Double(targetRevision))
            return payload
        case let .mutatePhraseRoleCatalog(expectedRevision, mutation):
            var payload = mutation.payload
            payload["kind"] = .string("mutatePhraseRoleCatalog")
            payload["expectedPhraseRoleRevision"] = .number(Double(expectedRevision))
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

    private func timelinePayload(
        _ kind: String,
        trackID: UInt64,
        expectedRevision: UInt64
    ) -> [String: JSONValue] {
        [
            "kind": .string(kind),
            "trackId": .number(Double(trackID)),
            "expectedTimelineRevision": .number(Double(expectedRevision))
        ]
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
