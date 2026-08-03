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
              let leaderDeckID = unsignedInteger(envelope.payload["leaderDeckId"]),
              case let .array(deckPayloads) = envelope.payload["decks"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let decks = try deckPayloads.map(decodeDeck)
        let deckIDs = Set(decks.map(\.deckID))
        guard decks.count == 2,
              deckIDs.count == decks.count,
              deckIDs.contains(leaderDeckID) else {
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
            leaderDeckID: leaderDeckID,
            decks: decks,
            nextPlan: nextPlan
        )
    }

    private func decodePlan(_ value: JSONValue?) throws -> PlanSnapshot {
        guard case let .object(plan) = value,
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
              case let .object(reasonPayload) = cue["reason"],
              case let .object(actionPayload) = cue["action"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return PlanCueSnapshot(
            phraseIndex: phraseIndex,
            startBeat: startBeat,
            endBeat: endBeat,
            origin: origin,
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
            guard case let .string(themeName) = payload["themeName"],
                  case let .string(sceneName) = payload["sceneName"],
                  case let .string(category) = payload["category"],
                  let loopBank = unsignedInteger(payload["loopBank"]),
                  let loopSlot = unsignedInteger(payload["loopSlot"]) else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return .applyLook(
                themeName: themeName,
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
