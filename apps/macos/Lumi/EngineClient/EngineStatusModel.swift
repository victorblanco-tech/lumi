import Combine
import Foundation
import LumiEngineClient
import LumiLibraryWorkspace
import LumiLiveWorkspace

@MainActor
final class EngineStatusModel: ObservableObject {
    @Published private(set) var workspaceState = LiveWorkspacePresenter.stopped()
    @Published private(set) var libraryState = LibraryWorkspaceState.importing()
    @Published private(set) var timelineEditFeedback: String?

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
    private let libraryDecoder = LibrarySnapshotDecoder()
    private var lifecycle: Lifecycle = .stopped
    private var monitoringTask: Task<Void, Never>?
    private var playbackTask: Task<Void, Never>?
    private var isExchangingCommand = false
    private var pendingInteractiveExchanges = 0
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
        libraryState = .importing()
        timelineEditFeedback = nil

        do {
            let executable = try engineExecutable()
            let endpoint = try await supervisor.launch(
                engineExecutable: executable,
                libraryDatabaseURL: try libraryDatabaseURL()
            )
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
            libraryState = try libraryDecoder.decode(envelope)
            lifecycle = .ready
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            startMonitoring()
            ensurePlaybackTask()
        } catch {
            await supervisor.stop()
            lifecycle = .failed
            let detail = (error as? LocalizedError)?.errorDescription
                ?? "Unknown local engine error"
            workspaceState = LiveWorkspacePresenter.failed(detail)
            libraryState = .failed(detail)
        }
    }

    private func libraryDatabaseURL() throws -> URL {
        let base = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let directory = base.appendingPathComponent("Lumi", isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true
        )
        return directory.appendingPathComponent("library.sqlite", isDirectory: false)
    }

    func restart() async {
        await stop()
        await start()
    }

    func stop() async {
        monitoringTask?.cancel()
        monitoringTask = nil
        playbackTask?.cancel()
        playbackTask = nil
        isExchangingCommand = false
        await supervisor.stop()
        lifecycle = .stopped
        latestSnapshot = nil
        endpointDescription = nil
        protocolVersion = nil
        workspaceState = LiveWorkspacePresenter.stopped()
        libraryState = .importing()
    }

    func queryLibrary(_ request: LibraryQueryRequest) async {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .queryLibrary(
                    search: request.search,
                    playlistID: request.playlistID,
                    offset: request.offset,
                    limit: request.limit
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                libraryState = .failed(failure.message)
                return
            }
            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(envelope)
        } catch {
            libraryState = .failed(
                (error as? LocalizedError)?.errorDescription
                    ?? "The library query could not be completed."
            )
        }
    }

    func openLibraryTrackEditor(trackID: UInt64) async {
        timelineEditFeedback = nil
        await exchangeLibraryCommand(.openLibraryTrackEditor(trackID: trackID))
    }

    func closeLibraryTrackEditor() async {
        guard libraryState.editor != nil else { return }
        await exchangeLibraryCommand(.closeLibraryTrackEditor)
        timelineEditFeedback = nil
    }

    func editLibraryTimeline(_ request: TrackTimelineEditRequest) async {
        guard let editor = libraryState.editor else { return }
        await exchangeTimelineCommand(
            .editLibraryTimeline(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                edit: engineTimelineEdit(request)
            ),
            success: "Phrase timeline saved."
        )
    }

    func mutateLibraryTimelineHistory(_ request: TrackTimelineHistoryRequest) async {
        guard let editor = libraryState.editor else { return }
        let command: EngineCommand
        let success: String
        switch request {
        case .undo:
            command = .undoLibraryTimeline(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision
            )
            success = "Timeline edit undone."
        case .redo:
            command = .redoLibraryTimeline(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision
            )
            success = "Timeline edit redone."
        case let .restore(revision):
            command = .restoreLibraryTimelineRevision(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                targetTimelineRevision: revision
            )
            success = "Revision \(revision) restored as a new revision."
        }
        await exchangeTimelineCommand(command, success: success)
    }

    private func exchangeTimelineCommand(_ command: EngineCommand, success: String) async {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(command)
            if let failure = EngineCommandFailure(envelope) {
                if failure.kind == "revisionConflict" {
                    let refreshed = try await supervisor.getSnapshot()
                    libraryState = try libraryDecoder.decode(refreshed)
                    timelineEditFeedback = "Timeline changed elsewhere. Lumi refreshed the latest revision."
                } else {
                    timelineEditFeedback = failure.message
                }
                return
            }
            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(envelope)
            let revision = libraryState.editor?.timeline.revision
            timelineEditFeedback = revision.map { "\(success) Revision \($0)." } ?? success
        } catch {
            timelineEditFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The phrase timeline edit could not be saved."
        }
    }

    private func engineTimelineEdit(_ request: TrackTimelineEditRequest) -> EngineTimelineEdit {
        switch request {
        case let .create(startBar, endBar, roleID):
            .create(startBar: startBar, endBar: endBar, roleID: roleID)
        case let .split(phraseIndex, atBar):
            .split(phraseIndex: phraseIndex, atBar: atBar)
        case let .mergePrevious(phraseIndex):
            .mergePrevious(phraseIndex: phraseIndex)
        case let .mergeNext(phraseIndex):
            .mergeNext(phraseIndex: phraseIndex)
        case let .moveBoundary(phraseIndex, toBar):
            .moveBoundary(afterPhraseIndex: phraseIndex, toBar: toBar)
        case let .deleteAbsorbPrevious(phraseIndex):
            .deleteAbsorbPrevious(phraseIndex: phraseIndex)
        case let .deleteAbsorbNext(phraseIndex):
            .deleteAbsorbNext(phraseIndex: phraseIndex)
        case let .changeRole(phraseIndex, roleID):
            .changeRole(phraseIndex: phraseIndex, roleID: roleID)
        }
    }

    private func exchangeLibraryCommand(_ command: EngineCommand) async {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(command)
            if let failure = EngineCommandFailure(envelope) {
                libraryState = .failed(failure.message)
                return
            }
            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(envelope)
        } catch {
            libraryState = .failed(
                (error as? LocalizedError)?.errorDescription
                    ?? "The track editor could not be updated."
            )
        }
    }

    func mutatePlan(_ request: PlanMutationRequest) async {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange(),
              let current = latestSnapshot else {
            return
        }
        defer { isExchangingCommand = false }

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

    func runSessionCommand(_ request: SessionCommandRequest) async {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange(),
              let current = latestSnapshot else {
            return
        }
        defer { isExchangingCommand = false }
        workspaceState = LiveWorkspacePresenter.ready(
            current,
            sessionInteraction: .submitting
        )
        do {
            let envelope = try await supervisor.send(engineCommand(for: request))
            if let failure = EngineCommandFailure(envelope) {
                if failure.kind == "revisionConflict" {
                    try await refreshSessionAfterConflict(
                        message: "Session changed elsewhere. Lumi refreshed the latest state.",
                        endpointDescription: endpointDescription,
                        protocolVersion: protocolVersion
                    )
                } else {
                    workspaceState = LiveWorkspacePresenter.ready(
                        current,
                        sessionInteraction: .rejected(failure.message)
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
                sessionInteraction: .succeeded(sessionSuccessMessage(request))
            )
            ensurePlaybackTask()
        } catch {
            workspaceState = LiveWorkspacePresenter.ready(
                latestSnapshot ?? current,
                sessionInteraction: .rejected(
                    (error as? LocalizedError)?.errorDescription
                        ?? "The session command could not be applied."
                )
            )
        }
    }

    private func engineCommand(for request: PlanMutationRequest) -> EngineCommand {
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

    private func refreshSessionAfterConflict(
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
            sessionInteraction: .rejected(message)
        )
    }

    private func engineCommand(for request: SessionCommandRequest) -> EngineCommand {
        switch request {
        case let .loadDemo(expectedRevision):
            .loadDemoSession(expectedStateRevision: expectedRevision)
        case let .setOperationState(state, expectedRevision):
            .setOperationState(state, expectedStateRevision: expectedRevision)
        case let .setSimulationSpeed(speed, expectedRevision):
            .setSimulationSpeed(speed, expectedStateRevision: expectedRevision)
        case let .setSimulationPlayback(playing, expectedRevision):
            .setSimulationPlayback(playing, expectedStateRevision: expectedRevision)
        case let .advanceToNextTrack(expectedRevision):
            .advanceToNextTrack(expectedStateRevision: expectedRevision)
        case let .resetDemo(expectedRevision):
            .resetDemoSession(expectedStateRevision: expectedRevision)
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

    private func sessionSuccessMessage(_ request: SessionCommandRequest) -> String {
        switch request {
        case .loadDemo: "Demo session loaded."
        case let .setOperationState(state, _): "Operation state is now \(state.uppercased())."
        case let .setSimulationSpeed(speed, _): "Simulation speed is now \(speed)×."
        case let .setSimulationPlayback(playing, _):
            playing ? "Simulation resumed." : "Simulation paused."
        case .advanceToNextTrack: "Next deck is now Live."
        case .resetDemo: "Demo session reset to its canonical start."
        }
    }

    private func ensurePlaybackTask() {
        guard playbackTask == nil else { return }
        playbackTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(250))
                guard !Task.isCancelled, let self else { return }
                await self.tickSimulation()
            }
        }
    }

    private func tickSimulation() async {
        guard lifecycle == .ready,
              !isExchangingCommand,
              pendingInteractiveExchanges == 0,
              let current = latestSnapshot,
              !current.simulation.paused,
              let endpointDescription,
              let protocolVersion else {
            return
        }
        isExchangingCommand = true
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .advanceSimulation(
                    elapsedTicks: 250,
                    expectedStateRevision: current.stateRevision
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                if failure.kind == "revisionConflict" {
                    let refreshed = try await supervisor.getSnapshot()
                    let snapshot = try snapshotDecoder.decode(
                        refreshed,
                        endpointDescription: endpointDescription,
                        protocolVersion: protocolVersion
                    )
                    latestSnapshot = snapshot
                    workspaceState = LiveWorkspacePresenter.ready(snapshot)
                }
                return
            }
            let snapshot = try snapshotDecoder.decode(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
        } catch {
            // The process monitor owns disconnect presentation; a later tick can recover.
        }
    }

    private func acquireInteractiveExchange() async -> Bool {
        pendingInteractiveExchanges += 1
        defer { pendingInteractiveExchanges -= 1 }

        while isExchangingCommand {
            do {
                try await Task.sleep(for: .milliseconds(5))
            } catch {
                return false
            }
            guard lifecycle == .ready else { return false }
        }

        guard lifecycle == .ready else { return false }
        isExchangingCommand = true
        return true
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
                    self.libraryState = .failed("The local Lumi engine disconnected.")
                    return
                }
            }
        }
    }
}
