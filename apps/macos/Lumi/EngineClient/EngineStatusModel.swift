import Foundation
import LumiEngineClient
import LumiProtocol

struct EngineReadyViewState: Equatable {
    let endpoint: String
    let engineVersion: String
    let protocolVersion: Int
    let snapshotSequence: UInt64
    let stateRevision: UInt64
    let runtimeCore: EngineRuntimeCoreViewState
    let deckSource: EngineDeckSourceViewState
    let leaderDeckID: UInt64
    let decks: [EngineDeckViewState]
    let nextPlan: EnginePlanViewState?
}

struct EngineRuntimeCoreViewState: Equatable {
    let model: String
    let health: String
    let queueCapacity: UInt64
    let queueDepth: UInt64
    let processedEvents: UInt64
    let lastDecision: String
}

struct EngineDeckSourceViewState: Equatable {
    let providerKind: String
    let status: String
}

struct EngineDeckViewState: Equatable, Identifiable {
    let deckID: UInt64
    let trackLoadID: UInt64
    let title: String
    let artist: String
    let bpmMilli: UInt64
    let pitchClass: String
    let keyMode: String
    let beat: UInt64
    let phraseIndex: UInt64?

    var id: UInt64 { deckID }
}

struct EnginePlanViewState: Equatable {
    let deckID: UInt64
    let trackLoadID: UInt64
    let trackDurationBeats: UInt64
    let revision: UInt64
    let configurationRevision: UInt64
    let status: String
    let cues: [EnginePlanCueViewState]
}

struct EnginePlanCueViewState: Equatable, Identifiable {
    let phraseIndex: UInt64
    let startBeat: UInt64
    let endBeat: UInt64
    let origin: String
    let reason: EnginePlanReasonViewState
    let action: EnginePlanActionViewState

    var id: UInt64 { phraseIndex }
}

enum EnginePlanReasonViewState: Equatable {
    case phraseCategoryMatched(phraseKind: String, category: String)
    case missingPhraseAnalysis
}

enum EnginePlanActionViewState: Equatable {
    case applyLook(
        themeName: String,
        sceneName: String,
        category: String,
        loopBank: UInt64,
        loopSlot: UInt64
    )
    case holdCurrentLook
}

enum EngineHealthState: Equatable {
    case stopped
    case starting
    case connecting(String)
    case ready(EngineReadyViewState)
    case disconnected
    case failed(String)
}

@MainActor
final class EngineStatusModel: ObservableObject {
    @Published private(set) var state: EngineHealthState = .stopped

    private let supervisor = EngineProcessSupervisor()
    private var monitoringTask: Task<Void, Never>?

    func start() async {
        guard state == .stopped || state == .disconnected || isFailed else {
            return
        }

        monitoringTask?.cancel()
        state = .starting

        do {
            let executable = try engineExecutable()
            let endpoint = try await supervisor.launch(engineExecutable: executable)
            let endpointDescription = "\(endpoint.host):\(endpoint.port)"
            state = .connecting(endpointDescription)

            let snapshot = try await supervisor.connect(to: endpoint)
            let readyState = try mapReadyState(snapshot, endpoint: endpoint)
            state = .ready(readyState)
            startMonitoring()
        } catch {
            await supervisor.stop()
            state = .failed((error as? LocalizedError)?.errorDescription ?? "Unknown engine error")
        }
    }

    func restart() async {
        await stop()
        await start()
    }

    func stop() async {
        monitoringTask?.cancel()
        monitoringTask = nil
        await supervisor.stop()
        state = .stopped
    }

    private var isFailed: Bool {
        if case .failed = state {
            true
        } else {
            false
        }
    }

    private func engineExecutable() throws -> URL {
        let executable = Bundle.main.bundleURL
            .appendingPathComponent("Contents")
            .appendingPathComponent("Helpers")
            .appendingPathComponent("lumi-engine")

        guard FileManager.default.isExecutableFile(atPath: executable.path) else {
            throw EngineClientError.executableMissing
        }
        return executable
    }

    private func mapReadyState(
        _ snapshot: MessageEnvelope,
        endpoint: EngineEndpoint
    ) throws -> EngineReadyViewState {
        guard snapshot.messageType == .snapshot,
              snapshot.payload["kind"] == .string("stateSnapshot"),
              case let .string(engineVersion) = snapshot.payload["engineVersion"],
              let stateRevision = unsignedInteger(snapshot.payload["stateRevision"]),
              case let .object(runtimePayload) = snapshot.payload["runtimeCore"],
              case let .string(runtimeModel) = runtimePayload["model"],
              case let .string(runtimeHealth) = runtimePayload["health"],
              let queueCapacity = unsignedInteger(runtimePayload["queueCapacity"]),
              let queueDepth = unsignedInteger(runtimePayload["queueDepth"]),
              let processedEvents = unsignedInteger(runtimePayload["processedEvents"]),
              case let .string(lastDecision) = runtimePayload["lastDecision"],
              case let .object(deckSourcePayload) = snapshot.payload["deckSource"],
              case let .string(providerKind) = deckSourcePayload["providerKind"],
              case let .string(deckSourceStatus) = deckSourcePayload["status"],
              let leaderDeckID = unsignedInteger(snapshot.payload["leaderDeckId"]),
              case let .array(deckPayloads) = snapshot.payload["decks"] else {
            throw EngineClientError.invalidInitialSnapshot
        }

        let decks = try deckPayloads.map(mapDeck)
        let deckIDs = Set(decks.map(\.deckID))
        guard decks.count == 2,
              deckIDs.count == decks.count,
              deckIDs.contains(leaderDeckID) else {
            throw EngineClientError.invalidInitialSnapshot
        }
        let nextPlan: EnginePlanViewState?
        if snapshot.payload["nextPlan"] == .null {
            nextPlan = nil
        } else {
            nextPlan = try mapPlan(snapshot.payload["nextPlan"])
            guard let nextDeck = decks.first(where: { $0.deckID != leaderDeckID }),
                  nextPlan?.deckID == nextDeck.deckID,
                  nextPlan?.trackLoadID == nextDeck.trackLoadID else {
                throw EngineClientError.invalidInitialSnapshot
            }
        }

        return EngineReadyViewState(
            endpoint: "\(endpoint.host):\(endpoint.port)",
            engineVersion: engineVersion,
            protocolVersion: endpoint.protocolVersion,
            snapshotSequence: snapshot.sequence,
            stateRevision: stateRevision,
            runtimeCore: EngineRuntimeCoreViewState(
                model: runtimeModel,
                health: runtimeHealth,
                queueCapacity: queueCapacity,
                queueDepth: queueDepth,
                processedEvents: processedEvents,
                lastDecision: lastDecision
            ),
            deckSource: EngineDeckSourceViewState(
                providerKind: providerKind,
                status: deckSourceStatus
            ),
            leaderDeckID: leaderDeckID,
            decks: decks,
            nextPlan: nextPlan
        )
    }

    private func mapPlan(_ value: JSONValue?) throws -> EnginePlanViewState {
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
            throw EngineClientError.invalidInitialSnapshot
        }
        let cues = try cuePayloads.map(mapPlanCue)
        guard !cues.isEmpty,
              cues.enumerated().allSatisfy({ offset, cue in
                  cue.phraseIndex == UInt64(offset)
              }) else {
            throw EngineClientError.invalidInitialSnapshot
        }
        var previousEnd: UInt64 = 0
        for cue in cues {
            guard cue.startBeat == previousEnd else {
                throw EngineClientError.invalidInitialSnapshot
            }
            previousEnd = cue.endBeat
        }
        guard previousEnd == trackDurationBeats else {
            throw EngineClientError.invalidInitialSnapshot
        }
        return EnginePlanViewState(
            deckID: deckID,
            trackLoadID: trackLoadID,
            trackDurationBeats: trackDurationBeats,
            revision: revision,
            configurationRevision: configurationRevision,
            status: status,
            cues: cues
        )
    }

    private func mapPlanCue(_ value: JSONValue) throws -> EnginePlanCueViewState {
        guard case let .object(cue) = value,
              let phraseIndex = unsignedInteger(cue["phraseIndex"]),
              let startBeat = unsignedInteger(cue["startBeat"]),
              let endBeat = unsignedInteger(cue["endBeat"]),
              endBeat > startBeat,
              case let .string(origin) = cue["origin"],
              ["automatic", "fallback", "user"].contains(origin),
              case let .object(reasonPayload) = cue["reason"],
              case let .object(actionPayload) = cue["action"] else {
            throw EngineClientError.invalidInitialSnapshot
        }
        return EnginePlanCueViewState(
            phraseIndex: phraseIndex,
            startBeat: startBeat,
            endBeat: endBeat,
            origin: origin,
            reason: try mapPlanReason(reasonPayload),
            action: try mapPlanAction(actionPayload)
        )
    }

    private func mapPlanReason(
        _ payload: [String: JSONValue]
    ) throws -> EnginePlanReasonViewState {
        guard case let .string(kind) = payload["kind"] else {
            throw EngineClientError.invalidInitialSnapshot
        }
        switch kind {
        case "phraseCategoryMatched":
            guard case let .string(phraseKind) = payload["phraseKind"],
                  case let .string(category) = payload["category"] else {
                throw EngineClientError.invalidInitialSnapshot
            }
            return .phraseCategoryMatched(phraseKind: phraseKind, category: category)
        case "missingPhraseAnalysis":
            return .missingPhraseAnalysis
        default:
            throw EngineClientError.invalidInitialSnapshot
        }
    }

    private func mapPlanAction(
        _ payload: [String: JSONValue]
    ) throws -> EnginePlanActionViewState {
        guard case let .string(kind) = payload["kind"] else {
            throw EngineClientError.invalidInitialSnapshot
        }
        switch kind {
        case "applyLook":
            guard case let .string(themeName) = payload["themeName"],
                  case let .string(sceneName) = payload["sceneName"],
                  case let .string(category) = payload["category"],
                  let loopBank = unsignedInteger(payload["loopBank"]),
                  let loopSlot = unsignedInteger(payload["loopSlot"]) else {
                throw EngineClientError.invalidInitialSnapshot
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
            throw EngineClientError.invalidInitialSnapshot
        }
    }

    private func mapDeck(_ value: JSONValue) throws -> EngineDeckViewState {
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
            throw EngineClientError.invalidInitialSnapshot
        }

        let phraseIndex: UInt64?
        if deck["phraseIndex"] == .null {
            phraseIndex = nil
        } else {
            guard let value = unsignedInteger(deck["phraseIndex"]) else {
                throw EngineClientError.invalidInitialSnapshot
            }
            phraseIndex = value
        }

        return EngineDeckViewState(
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

    private func startMonitoring() {
        monitoringTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled, let self else {
                    return
                }
                if await !self.supervisor.isRunning() {
                    self.state = .disconnected
                    return
                }
            }
        }
    }
}
