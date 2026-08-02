import Foundation
import LumiEngineClient
import LumiProtocol

struct EngineReadyViewState: Equatable {
    let endpoint: String
    let engineVersion: String
    let protocolVersion: Int
    let snapshotSequence: UInt64
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
              case let .string(engineVersion) = snapshot.payload["engineVersion"] else {
            throw EngineClientError.invalidInitialSnapshot
        }

        return EngineReadyViewState(
            endpoint: "\(endpoint.host):\(endpoint.port)",
            engineVersion: engineVersion,
            protocolVersion: endpoint.protocolVersion,
            snapshotSequence: snapshot.sequence
        )
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
