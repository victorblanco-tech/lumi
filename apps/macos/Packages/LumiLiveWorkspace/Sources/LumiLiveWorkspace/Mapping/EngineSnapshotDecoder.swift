import Foundation
import LumiProtocol

public enum EngineSnapshotDecodingError: Error, Equatable {
    case invalidSnapshot
}

public struct EngineSnapshotDecoder: Sendable {
    public init() {}

    public func decodeWaveformDetail(
        _ envelope: MessageEnvelope
    ) throws -> LibraryWaveformDetailSnapshot {
        guard envelope.messageType == .snapshot,
              case let .object(detail) = envelope.payload["waveformDetail"],
              let trackID = unsignedInteger(detail["trackId"]),
              trackID > 0,
              case let .string(source) = detail["source"],
              !source.isEmpty,
              detail["style"] == .string("rgb"),
              case let .array(pointPayloads) = detail["points"],
              (16...16_384).contains(pointPayloads.count) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let points = try pointPayloads.map { value in
            guard case let .array(channels) = value,
                  channels.count == 3,
                  let low = unsignedInteger(channels[0]),
                  let mid = unsignedInteger(channels[1]),
                  let high = unsignedInteger(channels[2]),
                  low <= UInt8.max,
                  mid <= UInt8.max,
                  high <= UInt8.max else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return DeckWaveformPointSnapshot(
                low: UInt8(low),
                mid: UInt8(mid),
                high: UInt8(high)
            )
        }
        return LibraryWaveformDetailSnapshot(
            trackID: trackID,
            preview: DeckWaveformPreviewSnapshot(
                source: source,
                style: "rgb",
                points: points
            )
        )
    }

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
              case let .string(sourceMode) = sourcePayload["mode"],
              case let .string(sourceDisplayName) = sourcePayload["displayName"],
              case let .string(sourceStatus) = sourcePayload["status"],
              case let .object(outputPayload) = envelope.payload["outputProvider"],
              case let .string(outputProviderKind) = outputPayload["providerKind"],
              case let .string(outputStatus) = outputPayload["status"],
              let outputRecordCount = unsignedInteger(outputPayload["recordCount"]),
              case let .array(deckPayloads) = envelope.payload["decks"],
              case let .object(optionsPayload) = envelope.payload["planningOptions"],
              case let .array(timelinePayloads) = envelope.payload["timeline"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let leaderDeckID: UInt64?
        if envelope.payload["leaderDeckId"] == .null {
            leaderDeckID = nil
        } else {
            guard let value = unsignedInteger(envelope.payload["leaderDeckId"]) else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            leaderDeckID = value
        }
        let simulation: SimulationSnapshot?
        if envelope.payload["simulation"] == nil || envelope.payload["simulation"] == .null {
            simulation = nil
        } else {
            guard case let .object(payload) = envelope.payload["simulation"],
                  let speed = unsignedInteger(payload["speed"]),
                  [1, 4, 16, 64].contains(speed),
                  case let .boolean(paused) = payload["paused"] else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            simulation = SimulationSnapshot(speed: speed, paused: paused)
        }
        let deckInputIntegration = try decodeDeckInputIntegration(
            envelope.payload["deckInputIntegration"]
        )
        let midiIntegration = try decodeMidiIntegration(envelope.payload["midiIntegration"])
        let midiClockIntegration = try decodeMidiClockIntegration(
            envelope.payload["midiClockIntegration"]
        )
        let abletonLinkIntegration = try decodeAbletonLinkIntegration(
            envelope.payload["abletonLinkIntegration"]
        )
        let decks = try deckPayloads.map(decodeDeck)
        let timeline = try timelinePayloads.map(decodeTimelineEntry)
        let deckIDs = Set(decks.map(\.deckID))
        guard decks.count <= 2,
              deckIDs.count == decks.count,
              deckIDs.allSatisfy({ [1, 2].contains($0) }),
              leaderDeckID.map(deckIDs.contains) ?? true else {
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
            guard let nextPlan,
                  let nextDeck = decks.first(where: { $0.deckID == nextPlan.deckID }),
                  nextPlan.deckID == nextDeck.deckID,
                  nextPlan.trackLoadID == nextDeck.trackLoadID,
                  leaderDeckID.map({ nextDeck.deckID != $0 }) ?? true else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
        }

        let livePlan: PlanSnapshot?
        if envelope.payload["livePlan"] == nil || envelope.payload["livePlan"] == .null {
            livePlan = nil
        } else {
            livePlan = try decodePlan(envelope.payload["livePlan"])
            guard let leaderDeckID,
                  let liveDeck = decks.first(where: { $0.deckID == leaderDeckID }),
                  livePlan?.deckID == liveDeck.deckID,
                  livePlan?.trackLoadID == liveDeck.trackLoadID else {
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
                mode: sourceMode,
                displayName: sourceDisplayName,
                status: sourceStatus
            ),
            deckInputIntegration: deckInputIntegration,
            midiIntegration: midiIntegration,
            midiClockIntegration: midiClockIntegration,
            abletonLinkIntegration: abletonLinkIntegration,
            simulation: simulation,
            outputProvider: OutputProviderSnapshot(
                providerKind: outputProviderKind,
                status: outputStatus,
                recordCount: outputRecordCount
            ),
            leaderDeckID: leaderDeckID,
            decks: decks,
            livePlan: livePlan,
            nextPlan: nextPlan,
            planningOptions: try decodePlanningOptions(optionsPayload),
            timeline: timeline
        )
    }

    private func decodeMidiIntegration(
        _ value: JSONValue?
    ) throws -> MidiOutputIntegrationSnapshot? {
        if value == nil || value == .null { return nil }
        guard case let .object(midi) = value,
              case let .string(state) = midi["state"],
              ["stopped", "ready"].contains(state),
              case let .string(sourceName) = midi["sourceName"],
              !sourceName.isEmpty,
              case let .string(protocolName) = midi["protocol"],
              !protocolName.isEmpty,
              let sentPulseCount = unsignedInteger(midi["sentPulseCount"]),
              case let .boolean(autoPublishEnabled) = midi["autoPublishEnabled"],
              let timingOffsetMillis = signedInteger(midi["timingOffsetMillis"]),
              (-250...250).contains(timingOffsetMillis),
              let bankPreRollMillis = unsignedInteger(midi["bankPreRollMillis"]),
              bankPreRollMillis <= 250 else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let activeBank = try optionalUnsignedInteger(midi["activeBank"])
        let pendingTimingOffsetMillis = try optionalSignedInteger(
            midi["pendingTimingOffsetMillis"]
        )
        guard activeBank.map({ (1...4).contains($0) }) ?? true else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        guard pendingTimingOffsetMillis.map({ (-250...250).contains($0) }) ?? true else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return MidiOutputIntegrationSnapshot(
            state: state,
            sourceName: sourceName,
            protocolName: protocolName,
            sentPulseCount: sentPulseCount,
            lastEvent: try optionalString(midi["lastEvent"]),
            lastError: try optionalString(midi["lastError"]),
            activeBank: activeBank,
            autoPublishEnabled: autoPublishEnabled,
            timingOffsetMillis: timingOffsetMillis,
            pendingTimingOffsetMillis: pendingTimingOffsetMillis,
            bankPreRollMillis: bankPreRollMillis,
            realtimeLane: try decodeRealtimeMidiLane(midi["realtimeScheduler"])
        )
    }

    private func decodeRealtimeMidiLane(
        _ value: JSONValue?
    ) throws -> RealtimeMidiOutputLaneSnapshot? {
        guard let value, value != .null else { return nil }
        guard case let .object(scheduler) = value,
              case let .object(lane)? = scheduler["lane"],
              let queueCapacity = unsignedInteger(lane["queueCapacity"]),
              let queueDepth = unsignedInteger(lane["queueDepth"]),
              let queueHighWater = unsignedInteger(lane["queueHighWater"]),
              let saturationCount = unsignedInteger(lane["saturationCount"]),
              let latencySampleCount = unsignedInteger(lane["latencySampleCount"]),
              let latencyP95Micros = unsignedInteger(lane["latencyP95Micros"]),
              queueDepth <= queueCapacity,
              queueHighWater <= queueCapacity else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return RealtimeMidiOutputLaneSnapshot(
            queueCapacity: queueCapacity,
            queueDepth: queueDepth,
            queueHighWater: queueHighWater,
            saturationCount: saturationCount,
            latencySampleCount: latencySampleCount,
            latencyP95Micros: latencyP95Micros,
            lastDispatchLatenessMicros: unsignedInteger(
                lane["lastDispatchLatenessMicros"]
            ) ?? 0,
            lateDispatchCount: unsignedInteger(lane["lateDispatchCount"]) ?? 0
        )
    }

    private func decodeMidiClockIntegration(
        _ value: JSONValue?
    ) throws -> MidiClockIntegrationSnapshot? {
        if value == nil || value == .null { return nil }
        guard case let .object(clock) = value,
              case let .string(state) = clock["state"],
              ["stopped", "ready", "running"].contains(state),
              case let .string(sourceName) = clock["sourceName"],
              !sourceName.isEmpty,
              case let .string(protocolName) = clock["protocol"],
              !protocolName.isEmpty,
              let sentTickCount = unsignedInteger(clock["sentTickCount"]) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return MidiClockIntegrationSnapshot(
            state: state,
            sourceName: sourceName,
            protocolName: protocolName,
            bpmMilli: try optionalUnsignedInteger(clock["bpmMilli"]),
            sentTickCount: sentTickCount,
            lastEvent: try optionalString(clock["lastEvent"]),
            lastError: try optionalString(clock["lastError"])
        )
    }

    private func decodeAbletonLinkIntegration(
        _ value: JSONValue?
    ) throws -> AbletonLinkIntegrationSnapshot? {
        if value == nil || value == .null { return nil }
        guard case let .object(link) = value,
              case let .string(state) = link["state"],
              ["stopped", "starting", "ready", "running", "degraded"].contains(state),
              case let .string(provider) = link["provider"],
              !provider.isEmpty,
              case let .boolean(enabled) = link["enabled"],
              let peers = unsignedInteger(link["peers"]),
              case let .boolean(playing) = link["playing"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return AbletonLinkIntegrationSnapshot(
            enabled: enabled,
            state: state,
            provider: provider,
            helperVersion: try optionalString(link["helperVersion"]),
            peers: peers,
            source: try optionalString(link["source"]),
            deckNumber: try optionalUnsignedInteger(link["deckNumber"]),
            bpmMilli: try optionalUnsignedInteger(link["bpmMilli"]),
            beatWithinBar: try optionalUnsignedInteger(link["beatWithinBar"]),
            playing: playing,
            generation: try optionalUnsignedInteger(link["generation"]),
            lastBeatAgeMillis: try optionalUnsignedInteger(link["lastBeatAgeMillis"]),
            phaseErrorMicros: try optionalSignedInteger(link["phaseErrorMicros"]),
            lastReanchor: try optionalString(link["lastReanchor"]),
            lastEvent: try optionalString(link["lastEvent"]),
            lastError: try optionalString(link["lastError"])
        )
    }

    private func decodeDeckInputIntegration(
        _ value: JSONValue?
    ) throws -> DeckInputIntegrationSnapshot? {
        if value == nil || value == .null {
            return nil
        }
        guard case let .object(input) = value,
              case let .string(state) = input["state"],
              ["stopped", "ready"].contains(state),
              case let .string(protocolName) = input["protocol"],
              !protocolName.isEmpty,
              let protocolVersion = unsignedInteger(input["protocolVersion"]),
              protocolVersion > 0,
              let receivedMessageCount = unsignedInteger(input["receivedMessageCount"]),
              let invalidWordCount = unsignedInteger(input["invalidWordCount"]),
              let committedFrameCount = unsignedInteger(input["committedFrameCount"]),
              let ignoredMessageCount = unsignedInteger(input["ignoredMessageCount"]),
              let duplicateFrameCount = unsignedInteger(input["duplicateFrameCount"]) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let destinationName = try optionalString(input["destinationName"])
        let lastDeckID = try optionalUnsignedInteger(input["lastDeckId"])
        let lastFrameSequence = try optionalUnsignedInteger(input["lastFrameSequence"])
        let isBLTMIDI = protocolName == "BLT MIDI Deck Frame"
        guard destinationName?.isEmpty != true,
              lastDeckID.map({ (1...4).contains($0) }) ?? true,
              lastFrameSequence.map({ !isBLTMIDI || $0 <= 127 }) ?? true else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return DeckInputIntegrationSnapshot(
            state: state,
            destinationName: destinationName,
            protocolName: protocolName,
            protocolVersion: protocolVersion,
            receivedMessageCount: receivedMessageCount,
            invalidWordCount: invalidWordCount,
            committedFrameCount: committedFrameCount,
            ignoredMessageCount: ignoredMessageCount,
            duplicateFrameCount: duplicateFrameCount,
            lastDeckID: lastDeckID,
            lastFrameSequence: lastFrameSequence
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
        let themeDecision: ThemeDecisionSnapshot?
        if plan["themeDecision"] == .null {
            themeDecision = nil
        } else {
            themeDecision = try decodeThemeDecision(plan["themeDecision"])
        }
        let libraryTrack: PlanLibraryTrackSnapshot?
        if plan["libraryTrack"] == .null || plan["libraryTrack"] == nil {
            libraryTrack = nil
        } else {
            libraryTrack = try decodePlanLibraryTrack(plan["libraryTrack"])
        }
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
        guard (status == "ready") == (themeDecision != nil) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        if let themeDecision {
            guard cues.first.map({ cue in
                if case let .applyLook(themeID, themeName, _, _, _, _, _) = cue.action {
                    return themeID == themeDecision.themeID
                        && themeName == themeDecision.themeName
                }
                return false
            }) == true else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
        }
        guard libraryTrack == nil || cues.allSatisfy({ $0.libraryResolution != nil }) else {
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
            themeDecision: themeDecision,
            libraryTrack: libraryTrack,
            cues: cues
        )
    }

    private func decodePlanLibraryTrack(_ value: JSONValue?) throws -> PlanLibraryTrackSnapshot {
        guard case let .object(identity) = value,
              identity["matchStatus"] == .string("exact"),
              case let .string(providerKind) = identity["providerKind"],
              !providerKind.isEmpty,
              case let .string(sourceID) = identity["sourceId"],
              !sourceID.isEmpty,
              case let .string(sourceName) = identity["sourceName"],
              !sourceName.isEmpty,
              case let .string(sourceTrackID) = identity["sourceTrackId"],
              !sourceTrackID.isEmpty,
              case let .string(analysisRevision) = identity["analysisRevision"],
              !analysisRevision.isEmpty,
              let timelineRevision = unsignedInteger(identity["timelineRevision"]),
              timelineRevision > 0 else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return PlanLibraryTrackSnapshot(
            providerKind: providerKind,
            sourceID: sourceID,
            sourceName: sourceName,
            sourceTrackID: sourceTrackID,
            analysisRevision: analysisRevision,
            timelineRevision: timelineRevision
        )
    }

    private func decodeThemeDecision(_ value: JSONValue?) throws -> ThemeDecisionSnapshot {
        guard case let .object(decision) = value,
              let themeID = unsignedInteger(decision["themeId"]),
              case let .string(themeName) = decision["themeName"],
              !themeName.isEmpty,
              case let .string(reason) = decision["reason"],
              [
                  "globalLock", "planInstanceUserChoice", "colorForce", "colorPrefer",
                  "rotation", "defaultTheme"
              ].contains(reason) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let matchedColorRGB: UInt64?
        if decision["matchedColorRgb"] == .null {
            matchedColorRGB = nil
        } else {
            guard let value = unsignedInteger(decision["matchedColorRgb"]),
                  value <= 0x00FF_FFFF else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            matchedColorRGB = value
        }
        guard ["colorForce", "colorPrefer"].contains(reason) == (matchedColorRGB != nil) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return ThemeDecisionSnapshot(
            themeID: themeID,
            themeName: themeName,
            reason: reason,
            matchedColorRGB: matchedColorRGB
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
            action: try decodeAction(actionPayload),
            libraryResolution: try decodeLibraryResolution(cue["libraryResolution"])
        )
    }

    private func decodeLibraryResolution(
        _ value: JSONValue?
    ) throws -> PlanCueLibraryResolutionSnapshot? {
        if value == nil || value == .null {
            return nil
        }
        guard case let .object(resolution) = value,
              case let .string(roleID) = resolution["roleId"],
              !roleID.isEmpty,
              case let .string(roleName) = resolution["roleName"],
              !roleName.isEmpty,
              case let .string(strategy) = resolution["strategy"],
              ["auto", "fixedVariant", "themeSpecificExact", "planOverride"].contains(strategy),
              case let .string(variantID) = resolution["variantId"],
              !variantID.isEmpty,
              let catalogRevision = unsignedInteger(resolution["catalogRevision"]),
              catalogRevision > 0,
              case let .string(resolutionReason) = resolution["resolutionReason"],
              !resolutionReason.isEmpty,
              case let .object(entry) = resolution["dryRunEntry"],
              case let .string(entryID) = entry["id"],
              !entryID.isEmpty,
              case let .string(entryName) = entry["name"],
              !entryName.isEmpty else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let choices: [PlanAutoloopChoiceSnapshot]
        if resolution["choices"] == nil || resolution["choices"] == .null {
            choices = []
        } else {
            guard case let .array(values) = resolution["choices"] else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            choices = try values.map { value in
                guard case let .object(choice) = value,
                      let id = unsignedInteger(choice["id"]),
                      (1...32).contains(id),
                      case let .string(name) = choice["name"],
                      !name.isEmpty,
                      case let .string(variantID) = choice["variantId"],
                      !variantID.isEmpty,
                      let bankNumber = unsignedInteger(choice["bankNumber"]),
                      (1...4).contains(bankNumber) else {
                    throw EngineSnapshotDecodingError.invalidSnapshot
                }
                return PlanAutoloopChoiceSnapshot(
                    id: id,
                    name: name,
                    variantID: variantID,
                    bankNumber: bankNumber
                )
            }
        }
        return PlanCueLibraryResolutionSnapshot(
            roleID: roleID,
            roleName: roleName,
            strategy: strategy,
            variantID: variantID,
            catalogRevision: catalogRevision,
            resolutionReason: resolutionReason,
            entryID: entryID,
            entryName: entryName,
            bankNumber: unsignedInteger(resolution["bankNumber"]),
            autoloopNumber: unsignedInteger(resolution["autoloopNumber"]),
            choices: choices
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
              case let .boolean(playing) = deck["playing"],
              case let .object(track) = deck["track"],
              case let .string(title) = track["title"],
              case let .string(artist) = track["artist"],
              let trackBPMMilli = unsignedInteger(track["bpmMilli"]),
              let durationBeats = unsignedInteger(track["durationBeats"]),
              durationBeats > 0,
              case let .array(phrasePayloads) = track["phrases"],
              case let .object(key) = track["key"],
              case let .string(pitchClass) = key["pitchClass"],
              case let .string(keyMode) = key["mode"] else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let bpmMilli: UInt64
        if deck["effectiveBpmMilli"] == nil {
            bpmMilli = trackBPMMilli
        } else {
            guard let value = unsignedInteger(deck["effectiveBpmMilli"]),
                  (20_000...300_000).contains(value) else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            bpmMilli = value
        }

        let colorRGB: UInt64?
        if track["colorRgb"] == nil || track["colorRgb"] == .null {
            colorRGB = nil
        } else {
            guard let value = unsignedInteger(track["colorRgb"]), value <= 0x00FF_FFFF else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            colorRGB = value
        }
        guard case let .string(planEligibilityValue) = deck["planEligibility"],
              let planEligibility = DeckPlanEligibility(rawValue: planEligibilityValue) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let localPlayback: LocalPlaybackTrackSnapshot?
        if deck["localPlayback"] == nil || deck["localPlayback"] == .null {
            localPlayback = nil
        } else {
            guard case let .object(payload) = deck["localPlayback"],
                  case let .string(audioURI) = payload["audioUri"],
                  !audioURI.isEmpty,
                  let durationMillis = unsignedInteger(payload["durationMillis"]),
                  durationMillis > 0 else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            localPlayback = LocalPlaybackTrackSnapshot(
                audioURI: audioURI,
                durationMillis: durationMillis
            )
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

        let playbackPositionMillis: UInt64?
        if deck["playbackPositionMillis"] == nil
            || deck["playbackPositionMillis"] == .null {
            playbackPositionMillis = nil
        } else {
            playbackPositionMillis = unsignedInteger(deck["playbackPositionMillis"])
            guard playbackPositionMillis != nil else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
        }
        let transportRevision = unsignedInteger(deck["transportRevision"]) ?? 0

        let phrases = try phrasePayloads.map(decodeDeckPhrase)
        let phraseTimelineIsValid = phrases.isEmpty
            ? planEligibility == .autoHeld && phraseIndex == nil
            : phrases.count <= 512
                && phrases.first?.startBeat == 0
                && phrases.last?.endBeat == durationBeats
                && phrases.enumerated().allSatisfy({ offset, phrase in
                    phrase.index == UInt64(offset)
                        && phrase.startBeat < phrase.endBeat
                        && phrase.endBeat <= durationBeats
                        && (offset == 0 || phrases[offset - 1].endBeat == phrase.startBeat)
                })
                && (phraseIndex.map { index in
                    phrases.contains { $0.index == index }
                } ?? true)
        guard phraseTimelineIsValid else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }

        let waveformPreview: DeckWaveformPreviewSnapshot?
        if track["waveformPreview"] == nil || track["waveformPreview"] == .null {
            waveformPreview = nil
        } else {
            waveformPreview = try decodeWaveformPreview(track["waveformPreview"])
        }

        let beatGrid: DeckBeatGridSnapshot?
        if track["beatGrid"] == nil || track["beatGrid"] == .null {
            beatGrid = nil
        } else {
            beatGrid = try decodeDeckBeatGrid(
                track["beatGrid"],
                durationBeats: durationBeats
            )
        }
        let hotCues = try decodeHotCues(
            track["hotCues"],
            durationMillis: beatGrid?.durationMillis
        )

        let keyKnown: Bool
        if key["known"] == nil {
            keyKnown = true
        } else {
            guard case let .boolean(value) = key["known"] else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            keyKnown = value
        }

        return DeckSnapshot(
            deckID: deckID,
            trackLoadID: trackLoadID,
            title: title,
            artist: artist,
            bpmMilli: bpmMilli,
            colorRGB: colorRGB,
            pitchClass: pitchClass,
            keyMode: keyMode,
            keyKnown: keyKnown,
            beat: beat,
            playing: playing,
            playbackPositionMillis: playbackPositionMillis,
            transportRevision: transportRevision,
            phraseIndex: phraseIndex,
            durationBeats: durationBeats,
            beatGrid: beatGrid,
            phrases: phrases,
            waveformPreview: waveformPreview,
            hotCues: hotCues,
            planEligibility: planEligibility,
            localPlayback: localPlayback
        )
    }

    private func decodeHotCues(
        _ value: JSONValue?,
        durationMillis: UInt64?
    ) throws -> [DeckHotCueSnapshot] {
        guard let value, value != .null else { return [] }
        guard case let .array(values) = value, values.count <= 26 else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let cues = try values.map { value -> DeckHotCueSnapshot in
            guard case let .object(cue) = value,
                  let indexValue = unsignedInteger(cue["index"]),
                  let index = UInt8(exactly: indexValue),
                  (1...26).contains(index),
                  let timeMillis = unsignedInteger(cue["timeMillis"]),
                  case let .string(name) = cue["name"],
                  let colorValue = unsignedInteger(cue["colorRgb"]),
                  let colorRGB = UInt32(exactly: colorValue),
                  colorRGB <= 0x00ff_ffff else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            let loopEndMillis: UInt64?
            if cue["loopEndMillis"] == nil || cue["loopEndMillis"] == .null {
                loopEndMillis = nil
            } else {
                guard let value = unsignedInteger(cue["loopEndMillis"]), value > timeMillis else {
                    throw EngineSnapshotDecodingError.invalidSnapshot
                }
                loopEndMillis = value
            }
            guard durationMillis.map({ duration in
                timeMillis < duration && (loopEndMillis.map { $0 <= duration } ?? true)
            }) ?? true else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return DeckHotCueSnapshot(
                index: index,
                timeMillis: timeMillis,
                loopEndMillis: loopEndMillis,
                name: name,
                colorRGB: colorRGB
            )
        }
        guard Set(cues.map(\.index)).count == cues.count else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return cues.sorted { $0.index < $1.index }
    }

    private func decodeDeckBeatGrid(
        _ value: JSONValue?,
        durationBeats: UInt64
    ) throws -> DeckBeatGridSnapshot {
        guard let value,
              case let .object(grid) = value,
              let beatsPerBarValue = unsignedInteger(grid["beatsPerBar"]),
              (1...16).contains(beatsPerBarValue),
              let durationMillis = unsignedInteger(grid["durationMillis"]),
              durationMillis > 0,
              case let .array(timePayloads) = grid["timesMillis"],
              timePayloads.count >= 2,
              timePayloads.count <= 100_000 else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        guard UInt64(timePayloads.count) <= durationBeats else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let timesMillis = try timePayloads.map { value in
            guard let timeMillis = unsignedInteger(value) else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return timeMillis
        }
        guard timesMillis.enumerated().allSatisfy({ offset, timeMillis in
            offset == 0 || timesMillis[offset - 1] < timeMillis
        }) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return DeckBeatGridSnapshot(
            beatsPerBar: UInt8(beatsPerBarValue),
            durationMillis: durationMillis,
            timesMillis: timesMillis
        )
    }

    private func decodeDeckPhrase(_ value: JSONValue) throws -> DeckPhraseSnapshot {
        guard case let .object(phrase) = value,
              let index = unsignedInteger(phrase["index"]),
              let startBeat = unsignedInteger(phrase["startBeat"]),
              let endBeat = unsignedInteger(phrase["endBeat"]),
              case let .string(kind) = phrase["kind"],
              ["intro", "verse", "breakdown", "build", "drop", "outro"].contains(kind) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let roleID: String?
        let roleName: String?
        if phrase["role"] == nil || phrase["role"] == .null {
            roleID = nil
            roleName = nil
        } else {
            guard case let .object(role) = phrase["role"],
                  case let .string(decodedRoleID) = role["roleId"],
                  !decodedRoleID.isEmpty,
                  case let .string(decodedRoleName) = role["roleName"],
                  !decodedRoleName.isEmpty else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            roleID = decodedRoleID
            roleName = decodedRoleName
        }
        return DeckPhraseSnapshot(
            index: index,
            startBeat: startBeat,
            endBeat: endBeat,
            kind: kind,
            roleID: roleID,
            roleName: roleName
        )
    }

    private func decodeWaveformPreview(_ value: JSONValue?) throws -> DeckWaveformPreviewSnapshot {
        guard let value,
              case let .object(preview) = value,
              case let .string(source) = preview["source"],
              !source.isEmpty,
              preview["style"] == .string("rgb"),
              case let .array(pointPayloads) = preview["points"],
              (16...4_096).contains(pointPayloads.count) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        let points = try pointPayloads.map { value in
            guard case let .object(point) = value,
                  let low = unsignedInteger(point["low"]),
                  let mid = unsignedInteger(point["mid"]),
                  let high = unsignedInteger(point["high"]),
                  low <= 31,
                  mid <= 31,
                  high <= 31 else {
                throw EngineSnapshotDecodingError.invalidSnapshot
            }
            return DeckWaveformPointSnapshot(
                low: UInt8(low),
                mid: UInt8(mid),
                high: UInt8(high)
            )
        }
        return DeckWaveformPreviewSnapshot(source: source, style: "rgb", points: points)
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

    private func signedInteger(_ value: JSONValue?) -> Int? {
        guard case let .number(number) = value,
              number.isFinite,
              number.rounded(.towardZero) == number,
              number >= Double(Int.min),
              number <= Double(Int.max) else {
            return nil
        }
        return Int(number)
    }

    private func optionalUnsignedInteger(_ value: JSONValue?) throws -> UInt64? {
        if value == nil || value == .null {
            return nil
        }
        guard let decoded = unsignedInteger(value) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return decoded
    }

    private func optionalSignedInteger(_ value: JSONValue?) throws -> Int? {
        if value == nil || value == .null {
            return nil
        }
        guard let decoded = signedInteger(value) else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return decoded
    }

    private func optionalString(_ value: JSONValue?) throws -> String? {
        if value == nil || value == .null {
            return nil
        }
        guard case let .string(string) = value else {
            throw EngineSnapshotDecodingError.invalidSnapshot
        }
        return string
    }
}
