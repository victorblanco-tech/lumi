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
            decks: decks
        )
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
