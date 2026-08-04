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

public struct EngineSourceConflictChoice: Equatable, Sendable {
    public let phraseIndex: UInt16
    public let side: String

    public init(phraseIndex: UInt16, side: String) {
        self.phraseIndex = phraseIndex
        self.side = side
    }
}

public enum EngineSourceReconcileStrategy: Equatable, Sendable {
    case keepLumi
    case rebase
    case merge([EngineSourceConflictChoice])
    case replaceWithSource

    fileprivate var payload: [String: JSONValue] {
        switch self {
        case .keepLumi:
            return ["strategy": .string("keepLumi")]
        case .rebase:
            return ["strategy": .string("rebase")]
        case let .merge(choices):
            return [
                "strategy": .string("merge"),
                "choices": .array(choices.map { choice in
                    .object([
                        "phraseIndex": .number(Double(choice.phraseIndex)),
                        "side": .string(choice.side)
                    ])
                })
            ]
        case .replaceWithSource:
            return ["strategy": .string("replaceWithSource")]
        }
    }
}

public enum EngineTimelineEdit: Equatable, Sendable {
    case create(startBeat: UInt32, endBeat: UInt32, roleID: String)
    case split(phraseIndex: UInt16, atBeat: UInt32)
    case mergePrevious(phraseIndex: UInt16)
    case mergeNext(phraseIndex: UInt16)
    case moveBoundary(afterPhraseIndex: UInt16, toBeat: UInt32)
    case deleteAbsorbPrevious(phraseIndex: UInt16)
    case deleteAbsorbNext(phraseIndex: UInt16)
    case changeRole(phraseIndex: UInt16, roleID: String)

    fileprivate var payload: [String: JSONValue] {
        switch self {
        case let .create(startBeat, endBeat, roleID):
            return [
                "operation": .string("create"),
                "startBeat": .number(Double(startBeat)),
                "endBeat": .number(Double(endBeat)),
                "roleId": .string(roleID)
            ]
        case let .split(phraseIndex, atBeat):
            return [
                "operation": .string("split"),
                "phraseIndex": .number(Double(phraseIndex)),
                "atBeat": .number(Double(atBeat))
            ]
        case let .mergePrevious(phraseIndex):
            return indexed("mergePrevious", phraseIndex: phraseIndex)
        case let .mergeNext(phraseIndex):
            return indexed("mergeNext", phraseIndex: phraseIndex)
        case let .moveBoundary(phraseIndex, toBeat):
            return [
                "operation": .string("moveBoundary"),
                "phraseIndex": .number(Double(phraseIndex)),
                "toBeat": .number(Double(toBeat))
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

public struct EngineThemeVariantOverride: Equatable, Sendable {
    public let themeID: UInt64
    public let variantID: String

    public init(themeID: UInt64, variantID: String) {
        self.themeID = themeID
        self.variantID = variantID
    }
}

public enum EnginePhraseLoopStrategy: Equatable, Sendable {
    case automatic
    case fixedVariant(String)
    case themeSpecificExact([EngineThemeVariantOverride])

    fileprivate var payload: [String: JSONValue] {
        switch self {
        case .automatic:
            ["strategy": .string("auto")]
        case let .fixedVariant(variantID):
            [
                "strategy": .string("fixedVariant"),
                "variantId": .string(variantID)
            ]
        case let .themeSpecificExact(overrides):
            [
                "strategy": .string("themeSpecificExact"),
                "themeOverrides": .array(overrides.map { value in
                    .object([
                        "themeId": .number(Double(value.themeID)),
                        "variantId": .string(value.variantID)
                    ])
                })
            ]
        }
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

public enum EngineAutoloopCatalogMutation: Equatable, Sendable {
    case renameTheme(themeID: UInt64, displayName: String)
    case addVariant(roleID: String, displayName: String)
    case renameVariant(roleID: String, variantID: String, displayName: String)
    case moveVariantEarlier(roleID: String, variantID: String)
    case moveVariantLater(roleID: String, variantID: String)
    case archiveVariant(roleID: String, variantID: String)
    case restoreVariant(roleID: String, variantID: String)
    case setCell(themeID: UInt64, roleID: String, variantID: String, displayName: String?)
    case setButton(themeID: UInt64, buttonNumber: UInt16, roleID: String, displayName: String?)

    fileprivate var payload: [String: JSONValue] {
        switch self {
        case let .renameTheme(themeID, displayName):
            [
                "operation": .string("renameTheme"),
                "themeId": .number(Double(themeID)),
                "displayName": .string(displayName)
            ]
        case let .addVariant(roleID, displayName):
            [
                "operation": .string("addVariant"),
                "roleId": .string(roleID),
                "displayName": .string(displayName)
            ]
        case let .renameVariant(roleID, variantID, displayName):
            rowPayload(
                "renameVariant",
                roleID: roleID,
                variantID: variantID,
                additional: ["displayName": .string(displayName)]
            )
        case let .moveVariantEarlier(roleID, variantID):
            rowPayload("moveVariantEarlier", roleID: roleID, variantID: variantID)
        case let .moveVariantLater(roleID, variantID):
            rowPayload("moveVariantLater", roleID: roleID, variantID: variantID)
        case let .archiveVariant(roleID, variantID):
            rowPayload("archiveVariant", roleID: roleID, variantID: variantID)
        case let .restoreVariant(roleID, variantID):
            rowPayload("restoreVariant", roleID: roleID, variantID: variantID)
        case let .setCell(themeID, roleID, variantID, displayName):
            rowPayload(
                "setCell",
                roleID: roleID,
                variantID: variantID,
                additional: [
                    "themeId": .number(Double(themeID)),
                    "displayName": displayName.map(JSONValue.string) ?? .null
                ]
            )
        case let .setButton(themeID, buttonNumber, roleID, displayName):
            [
                "operation": .string("setButton"),
                "themeId": .number(Double(themeID)),
                "buttonNumber": .number(Double(buttonNumber)),
                "roleId": .string(roleID),
                "displayName": displayName.map(JSONValue.string) ?? .null
            ]
        }
    }

    private func rowPayload(
        _ operation: String,
        roleID: String,
        variantID: String,
        additional: [String: JSONValue] = [:]
    ) -> [String: JSONValue] {
        var payload = additional
        payload["operation"] = .string(operation)
        payload["roleId"] = .string(roleID)
        payload["variantId"] = .string(variantID)
        return payload
    }
}

public enum EngineCommand: Equatable, Sendable {
    case queryLibrary(search: String, playlistID: UInt64?, offset: UInt32, limit: UInt16)
    case openLibraryTrackEditor(trackID: UInt64)
    case closeLibraryTrackEditor
    case previewDemoSourceRefresh
    case reconcileLibrarySource(
        trackID: UInt64,
        expectedTimelineRevision: UInt64,
        strategy: EngineSourceReconcileStrategy
    )
    case editLibraryTimeline(
        trackID: UInt64,
        expectedTimelineRevision: UInt64,
        edit: EngineTimelineEdit
    )
    case setLibraryPhraseLoopStrategy(
        trackID: UInt64,
        phraseIndex: UInt16,
        expectedTimelineRevision: UInt64,
        expectedAutoloopCatalogRevision: UInt64,
        strategy: EnginePhraseLoopStrategy
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
    case mutateAutoloopCatalog(
        expectedAutoloopCatalogRevision: UInt64,
        mutation: EngineAutoloopCatalogMutation
    )
    case loadLibraryTrackOnSimulatorDeck(
        trackID: UInt64,
        deckID: UInt64,
        expectedTimelineRevision: UInt64,
        expectedStateRevision: UInt64
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
        case .previewDemoSourceRefresh:
            return ["kind": .string("previewDemoSourceRefresh")]
        case let .reconcileLibrarySource(trackID, expectedRevision, strategy):
            var payload = strategy.payload
            payload["kind"] = .string("reconcileLibrarySource")
            payload["trackId"] = .number(Double(trackID))
            payload["expectedTimelineRevision"] = .number(Double(expectedRevision))
            return payload
        case let .editLibraryTimeline(trackID, expectedRevision, edit):
            var payload = edit.payload
            payload["kind"] = .string("editLibraryTimeline")
            payload["trackId"] = .number(Double(trackID))
            payload["expectedTimelineRevision"] = .number(Double(expectedRevision))
            return payload
        case let .setLibraryPhraseLoopStrategy(
            trackID,
            phraseIndex,
            expectedTimelineRevision,
            expectedCatalogRevision,
            strategy
        ):
            var payload = strategy.payload
            payload["kind"] = .string("setLibraryPhraseLoopStrategy")
            payload["trackId"] = .number(Double(trackID))
            payload["phraseIndex"] = .number(Double(phraseIndex))
            payload["expectedTimelineRevision"] = .number(Double(expectedTimelineRevision))
            payload["expectedAutoloopCatalogRevision"] = .number(Double(expectedCatalogRevision))
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
        case let .mutateAutoloopCatalog(expectedRevision, mutation):
            var payload = mutation.payload
            payload["kind"] = .string("mutateAutoloopCatalog")
            payload["expectedAutoloopCatalogRevision"] = .number(Double(expectedRevision))
            return payload
        case let .loadLibraryTrackOnSimulatorDeck(
            trackID,
            deckID,
            expectedTimelineRevision,
            expectedStateRevision
        ):
            return statePayload(
                "loadLibraryTrackOnSimulatorDeck",
                expectedRevision: expectedStateRevision,
                additional: [
                    "trackId": .number(Double(trackID)),
                    "deckId": .number(Double(deckID)),
                    "expectedTimelineRevision": .number(Double(expectedTimelineRevision))
                ]
            )
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
