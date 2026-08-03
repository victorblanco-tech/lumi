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
    private var latestSnapshot: EngineSnapshot?
    private var endpointDescription: String?
    private var protocolVersion: Int?

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
            self.endpointDescription = endpointDescription
            protocolVersion = endpoint.protocolVersion
            lifecycle = .connecting
            workspaceState = LiveWorkspacePresenter.connecting(to: endpointDescription)

            let envelope = try await supervisor.connect(to: endpoint)
            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: endpoint.protocolVersion
            )
            lifecycle = .ready
            latestSnapshot = snapshot
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
        latestSnapshot = nil
        endpointDescription = nil
        protocolVersion = nil
        workspaceState = LiveWorkspacePresenter.stopped()
    }

    func mutatePlan(_ request: PlanMutationRequest) async {
        guard lifecycle == .ready,
              let current = latestSnapshot,
              let endpointDescription,
              let protocolVersion else {
            return
        }

        workspaceState = LiveWorkspacePresenter.ready(
            current,
            planInteraction: .submitting
        )

        do {
            let envelope = try await supervisor.send(engineCommand(for: request))
            if let failure = EngineCommandFailure(envelope) {
                if failure.kind == "revisionConflict" {
                    try await refreshAfterConflict(
                        message: "Plan changed elsewhere. Lumi refreshed the latest revision.",
                        endpointDescription: endpointDescription,
                        protocolVersion: protocolVersion
                    )
                } else {
                    workspaceState = LiveWorkspacePresenter.ready(
                        current,
                        planInteraction: .rejected(failure.message)
                    )
                }
                return
            }

            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(
                snapshot,
                planInteraction: .succeeded(
                    "Plan revision \(snapshot.nextPlan?.revision ?? 0) saved."
                )
            )
        } catch {
            workspaceState = LiveWorkspacePresenter.ready(
                latestSnapshot ?? current,
                planInteraction: .rejected(
                    (error as? LocalizedError)?.errorDescription
                        ?? "The plan change could not be saved."
                )
            )
        }
    }

    private func refreshAfterConflict(
        message: String,
        endpointDescription: String,
        protocolVersion: Int
    ) async throws {
        let envelope = try await supervisor.getSnapshot()
        let snapshot = try snapshotDecoder.decode(
            envelope,
            endpointDescription: endpointDescription,
            protocolVersion: protocolVersion
        )
        latestSnapshot = snapshot
        workspaceState = LiveWorkspacePresenter.ready(
            snapshot,
            planInteraction: .rejected(message)
        )
    }

    private func engineCommand(for request: PlanMutationRequest) -> EnginePlanCommand {
        switch request {
        case let .selectTheme(context, themeID):
            .selectTheme(context: engineContext(context), themeID: themeID)
        case let .selectScene(context, phraseIndex, sceneID):
            .selectScene(
                context: engineContext(context),
                phraseIndex: phraseIndex,
                sceneID: sceneID
            )
        case let .setCueLock(context, phraseIndex, locked):
            .setCueLock(
                context: engineContext(context),
                phraseIndex: phraseIndex,
                locked: locked
            )
        case let .regeneratePlan(context):
            .regeneratePlan(context: engineContext(context))
        }
    }

    private func engineContext(
        _ context: PlanMutationContext
    ) -> EnginePlanCommandContext {
        EnginePlanCommandContext(
            planID: context.planID,
            trackLoadID: context.trackLoadID,
            expectedPlanRevision: context.expectedPlanRevision
        )
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
