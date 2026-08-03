import Foundation
import LumiProtocol

public enum EngineSnapshotDecodingError: Error, Equatable {
    case invalidSnapshot
}

public struct EngineSnapshotDecoder: Sendable {
    public init() {}

    public func decode(
        _ envelope: MessageEnvelope,
        endpointDescription: String,
        protocolVersion: Int
    ) throws -> EngineSnapshot {
        guard envelope.messageType == .snapshot,
              envelope.payload["kind"] == .string("stateSnapshot"),
              case let .string(engineVersion) = envelope.payload["engineVersion"],
              let stateRevision = unsignedInteger(envelope.payload["stateRevision"]),
              case let .string(operationState) = envelope.payload["operationState"],
              ["off", "armed", "live", "paused"].contains(operationState),
              case let .object(runtimePayload) = envelope.payload["runtimeCore"],
              case let .string(runtimeModel) = runtimePayload["model"],
              case let .string(runtimeHealth) = runtimePayload["health"],
              let queueCapacity = unsignedInteger(runtimePayload["queueCapacity"]),
              let queueDepth = unsignedInteger(runtimePayload["queueDepth"]),
              let processedEvents = unsignedInteger(runtimePayload["processedEvents"]),
              case let .string(lastDecision) = runtimePayload["lastDecision"],
              case let .object(sourcePayload) = envelope.payload["deckSource"],
              case let .string(providerKind) = sourcePayload["providerKind"],
              case let .string(sourceStatus) = sourcePayload["status"],
              case let .object(simulationPayload) = envelope.payload["simulation"],
              let simulationSpeed = unsignedInteger(simulationPayload["speed"]),
              [1, 4, 16, 64].contains(simulationSpeed),
              case let .boolean(simulationPaused) = simulationPayload["paused"],
              case let .object(outputPayload) = envelope.payload["outputProvider"],
              case let .string(outputProviderKind) = outputPayload["providerKind"],
              case let .string(outputStatus) = outputPayload["status"],
              let outputRecordCount = unsignedInteger(outputPayload["recordCount"]),
              let leaderDeckID = unsignedInteger(envelope.payload["leaderDeckId"]),
              case let .array(deckPayloads) = envelope.payload["decks"],
              case let .object(optionsPayload) = envelope.payload["planningOptions"],
              case let .array(timelinePayloads) = envelope.payload["timeline"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let decks = try deckPayloads.map(decodeDeck)
        let timeline = try timelinePayloads.map(decodeTimelineEntry)
        let deckIDs = Set(decks.map(\.deckID))
        guard decks.count == 2,
              deckIDs.count == decks.count,
              deckIDs.contains(leaderDeckID) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        guard timeline.count <= 256,
              timeline.enumerated().allSatisfy({ index, entry in
                  index == 0 || timeline[index - 1].sequence < entry.sequence
              }) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let nextPlan: PlanSnapshot?
        if envelope.payload["nextPlan"] == .null {
            nextPlan = nil
        } else {
            nextPlan = try decodePlan(envelope.payload["nextPlan"])
            guard let nextDeck = decks.first(where: { $0.deckID != leaderDeckID }),
                  nextPlan?.deckID == nextDeck.deckID,
                  nextPlan?.trackLoadID == nextDeck.trackLoadID else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
        }

        return EngineSnapshot(
            endpoint: endpointDescription,
            engineVersion: engineVersion,
            protocolVersion: protocolVersion,
            snapshotSequence: envelope.sequence,
            stateRevision: stateRevision,
            operationState: operationState,
            runtime: RuntimeSnapshot(
                model: runtimeModel,
                health: runtimeHealth,
                queueCapacity: queueCapacity,
                queueDepth: queueDepth,
                processedEvents: processedEvents,
                lastDecision: lastDecision
            ),
            deckSource: DeckSourceSnapshot(
                providerKind: providerKind,
                status: sourceStatus
            ),
            simulation: SimulationSnapshot(
                speed: simulationSpeed,
                paused: simulationPaused
            ),
            outputProvider: OutputProviderSnapshot(
                providerKind: outputProviderKind,
                status: outputStatus,
                recordCount: outputRecordCount
            ),
            leaderDeckID: leaderDeckID,
            decks: decks,
            nextPlan: nextPlan,
            planningOptions: try decodePlanningOptions(optionsPayload),
            timeline: timeline
        )
    }

    private func decodeTimelineEntry(_ value: JSONValue) throws -> TimelineEntrySnapshot {
        guard case let .object(entry) = value,
              let sequence = unsignedInteger(entry["sequence"]),
              let occurredAt = unsignedInteger(entry["occurredAt"]),
              case let .string(source) = entry["source"],
              ["runtime", "deckSource", "operation", "planner", "output"].contains(source),
              case let .string(type) = entry["type"],
              !type.isEmpty,
              case let .string(result) = entry["result"],
              [
                  "accepted", "ignored", "scheduled", "simulated", "rejected",
                  "skipped", "completed"
              ].contains(result),
              case let .string(reason) = entry["reason"],
              !reason.isEmpty else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return TimelineEntrySnapshot(
            sequence: sequence,
            occurredAt: occurredAt,
            source: source,
            type: type,
            result: result,
            reason: reason
        )
    }

    private func decodePlan(_ value: JSONValue?) throws -> PlanSnapshot {
        guard case let .object(plan) = value,
              case let .string(planID) = plan["planId"],
              !planID.isEmpty,
              let deckID = unsignedInteger(plan["deckId"]),
              let trackLoadID = unsignedInteger(plan["trackLoadId"]),
              let trackDurationBeats = unsignedInteger(plan["trackDurationBeats"]),
              let revision = unsignedInteger(plan["revision"]),
              let configurationRevision = unsignedInteger(plan["configurationRevision"]),
              case let .string(status) = plan["status"],
              ["ready", "fallback"].contains(status),
              revision > 0,
              configurationRevision > 0,
              trackDurationBeats > 0,
              case let .array(cuePayloads) = plan["cues"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let cues = try cuePayloads.map(decodePlanCue)
        guard !cues.isEmpty,
              cues.enumerated().allSatisfy({ offset, cue in
                  cue.phraseIndex == UInt64(offset)
              }) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        var previousEnd: UInt64 = 0
        for cue in cues {
            guard cue.startBeat == previousEnd else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            previousEnd = cue.endBeat
        }
        guard previousEnd == trackDurationBeats else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        return PlanSnapshot(
            planID: planID,
            deckID: deckID,
            trackLoadID: trackLoadID,
            trackDurationBeats: trackDurationBeats,
            revision: revision,
            configurationRevision: configurationRevision,
            status: status,
            cues: cues
        )
    }

    private func decodePlanCue(_ value: JSONValue) throws -> PlanCueSnapshot {
        guard case let .object(cue) = value,
              let phraseIndex = unsignedInteger(cue["phraseIndex"]),
              let startBeat = unsignedInteger(cue["startBeat"]),
              let endBeat = unsignedInteger(cue["endBeat"]),
              endBeat > startBeat,
              case let .string(origin) = cue["origin"],
              ["automatic", "fallback", "user"].contains(origin),
              case let .boolean(locked) = cue["locked"],
              case let .object(reasonPayload) = cue["reason"],
              case let .object(actionPayload) = cue["action"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return PlanCueSnapshot(
            phraseIndex: phraseIndex,
            startBeat: startBeat,
            endBeat: endBeat,
            origin: origin,
            locked: locked,
            reason: try decodeReason(reasonPayload),
            action: try decodeAction(actionPayload)
        )
    }

    private func decodeReason(_ payload: [String: JSONValue]) throws -> PlanReasonSnapshot {
        guard case let .string(kind) = payload["kind"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        switch kind {
        case "phraseCategoryMatched":
            guard case let .string(phraseKind) = payload["phraseKind"],
                  case let .string(category) = payload["category"] else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return .phraseCategoryMatched(phraseKind: phraseKind, category: category)
        case "missingPhraseAnalysis":
            return .missingPhraseAnalysis
        default:
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
    }

    private func decodeAction(_ payload: [String: JSONValue]) throws -> PlanActionSnapshot {
        guard case let .string(kind) = payload["kind"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        switch kind {
        case "applyLook":
            guard let themeID = unsignedInteger(payload["themeId"]),
                  case let .string(themeName) = payload["themeName"],
                  let sceneID = unsignedInteger(payload["sceneId"]),
                  case let .string(sceneName) = payload["sceneName"],
                  case let .string(category) = payload["category"],
                  let loopBank = unsignedInteger(payload["loopBank"]),
                  let loopSlot = unsignedInteger(payload["loopSlot"]) else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return .applyLook(
                themeID: themeID,
                themeName: themeName,
                sceneID: sceneID,
                sceneName: sceneName,
                category: category,
                loopBank: loopBank,
                loopSlot: loopSlot
            )
        case "holdCurrentLook":
            return .holdCurrentLook
        default:
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
    }

    private func decodePlanningOptions(
        _ payload: [String: JSONValue]
    ) throws -> PlanningOptionsSnapshot {
        guard case let .array(themePayloads) = payload["themes"],
              case let .array(scenePayloads) = payload["scenes"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let themes = try themePayloads.map { value in
            guard case let .object(theme) = value,
                  let id = unsignedInteger(theme["id"]),
                  case let .string(name) = theme["name"],
                  !name.isEmpty else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return ThemeOptionSnapshot(id: id, name: name)
        }
        let scenes = try scenePayloads.map { value in
            guard case let .object(scene) = value,
                  let id = unsignedInteger(scene["id"]),
                  case let .string(name) = scene["name"],
                  case let .string(category) = scene["category"],
                  let loopBank = unsignedInteger(scene["loopBank"]),
                  let loopSlot = unsignedInteger(scene["loopSlot"]),
                  !name.isEmpty else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return SceneOptionSnapshot(
                id: id,
                name: name,
                category: category,
                loopBank: loopBank,
                loopSlot: loopSlot
            )
        }
        guard !themes.isEmpty, !scenes.isEmpty,
              Set(themes.map(\.id)).count == themes.count,
              Set(scenes.map(\.id)).count == scenes.count else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return PlanningOptionsSnapshot(themes: themes, scenes: scenes)
    }

    private func decodeDeck(_ value: JSONValue) throws -> DeckSnapshot {
        guard case let .object(deck) = value,
              let deckID = unsignedInteger(deck["deckId"]),
              let trackLoadID = unsignedInteger(deck["trackLoadId"]),
              let beat = unsignedInteger(deck["beat"]),
              case let .object(track) = deck["track"],
              case let .string(title) = track["title"],
              case let .string(artist) = track["artist"],
              let bpmMilli = unsignedInteger(track["bpmMilli"]),
              case let .object(key) = track["key"],
              case let .string(pitchClass) = key["pitchClass"],
              case let .string(keyMode) = key["mode"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let phraseIndex: UInt64?
        if deck["phraseIndex"] == .null {
            phraseIndex = nil
        } else {
            guard let value = unsignedInteger(deck["phraseIndex"]) else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            phraseIndex = value
        }

        return DeckSnapshot(
            deckID: deckID,
            trackLoadID: trackLoadID,
            title: title,
            artist: artist,
            bpmMilli: bpmMilli,
            pitchClass: pitchClass,
            keyMode: keyMode,
            beat: beat,
            phraseIndex: phraseIndex
        )
    }

    private func unsignedInteger(_ value: JSONValue?) -> UInt64? {
        guard case let .number(number) = value,
              number.isFinite,
              number >= 0,
              number.rounded(.towardZero) == number,
              number <= Double(UInt64.max) else {
            return nil
        }
        return UInt64(number)
    }
}
