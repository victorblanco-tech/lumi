import Combine
import Foundation
import LumiEngineClient
import LumiLiveWorkspace

@MainActor
final class EngineStatusModel: ObservableObject {
    @Published private(set) var workspaceState = LiveWorkspacePresenter.stopped()

    private enum Lifecycle: Equatable {
        case stopped
        case starting
        case connecting
        case ready
        case disconnected
        case failed
    }

    private let supervisor = EngineProcessSupervisor()
    private let snapshotDecoder = EngineSnapshotDecoder()
    private var lifecycle: Lifecycle = .stopped
    private var monitoringTask: Task<Void, Never>?

    func start() async {
        guard [.stopped, .disconnected, .failed].contains(lifecycle) else {
            return
        }

        monitoringTask?.cancel()
        lifecycle = .starting
        workspaceState = LiveWorkspacePresenter.starting()

        do {
            let executable = try engineExecutable()
            let endpoint = try await supervisor.launch(engineExecutable: executable)
            let endpointDescription = "\(endpoint.host):\(endpoint.port)"
            lifecycle = .connecting
            workspaceState = LiveWorkspacePresenter.connecting(to: endpointDescription)

            let envelope = try await supervisor.connect(to: endpoint)
            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: endpoint.protocolVersion
            )
            lifecycle = .ready
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            startMonitoring()
        } catch {
            await supervisor.stop()
            lifecycle = .failed
            let detail = (error as? LocalizedError)?.errorDescription
                ?? "Unknown local engine error"
            workspaceState = LiveWorkspacePresenter.failed(detail)
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
        lifecycle = .stopped
        workspaceState = LiveWorkspacePresenter.stopped()
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

    private func startMonitoring() {
        monitoringTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled, let self else {
                    return
                }
                if await !self.supervisor.isRunning() {
                    self.lifecycle = .disconnected
                    self.workspaceState = LiveWorkspacePresenter.disconnected()
                    return
                }
            }
        }
    }
}
