import AVFoundation
import Combine
import Foundation
import LumiEngineClient
import LumiLibraryWorkspace
import LumiLiveWorkspace
import LumiProtocol
import OSLog

@MainActor
final class EngineStatusModel: ObservableObject {
    private static let logger = Logger(
        subsystem: "nl.blancoservices.lumi",
        category: "EngineStatusModel"
    )

    private enum LibraryCommandFailurePresentation {
        case library
        case timeline
        case localPlayback
    }

    @Published private(set) var workspaceState = LiveWorkspacePresenter.stopped()
    @Published private(set) var libraryState = LibraryWorkspaceState.importing()
    @Published private(set) var timelineEditFeedback: String?
    @Published private(set) var phraseRoleFeedback: String?
    @Published private(set) var autoloopCatalogFeedback: String?
    @Published private(set) var midiIntegrationFeedback: String?
    @Published private(set) var localPlaybackFeedback: String?
    @Published private(set) var localPlaybackFeedbackIsError = false
    @Published private(set) var localPlaybackVisualClocks: [
        UInt64: LocalPlaybackVisualClockSnapshot
    ] = [:]
    @Published private(set) var localPlaybackWaveforms: [
        UInt64: DeckWaveformPreviewSnapshot
    ] = [:]
    @Published private(set) var sourceImportFeedback: String?
    @Published private(set) var sourceImportFeedbackIsError = false

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
    private var localAudioControllers: [UInt64: LocalDeckAudioController] = [:]
    private var pendingLocalTransports: [UInt64: LocalDeckTransportSnapshot] = [:]
    private var pendingLocalTransportDecks: [UInt64] = []
    private var localTransportDrainTask: Task<Void, Never>?
    private var isExchangingCommand = false
    private var pendingInteractiveExchanges = 0
    private var pendingLibraryQuery: (generation: UInt64, request: LibraryQueryRequest)?
    private var libraryQueryGeneration: UInt64 = 0
    private var isDrainingLibraryQueries = false
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
        phraseRoleFeedback = nil
        autoloopCatalogFeedback = nil
        midiIntegrationFeedback = nil
        localPlaybackFeedback = nil
        localPlaybackFeedbackIsError = false
        sourceImportFeedback = nil
        sourceImportFeedbackIsError = false

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
            synchronizeLocalAudio(with: snapshot)
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
        localTransportDrainTask?.cancel()
        localTransportDrainTask = nil
        pendingLocalTransports.removeAll()
        pendingLocalTransportDecks.removeAll()
        pendingLibraryQuery = nil
        libraryQueryGeneration &+= 1
        isDrainingLibraryQueries = false
        localAudioControllers.values.forEach { $0.shutdown() }
        localAudioControllers.removeAll()
        localPlaybackVisualClocks = [:]
        isExchangingCommand = false
        await supervisor.stop()
        lifecycle = .stopped
        latestSnapshot = nil
        endpointDescription = nil
        protocolVersion = nil
        workspaceState = LiveWorkspacePresenter.stopped()
        libraryState = .importing()
        phraseRoleFeedback = nil
        autoloopCatalogFeedback = nil
        midiIntegrationFeedback = nil
        localPlaybackFeedback = nil
        localPlaybackFeedbackIsError = false
        sourceImportFeedback = nil
        sourceImportFeedbackIsError = false
    }

    func queryLibrary(_ request: LibraryQueryRequest) async {
        libraryQueryGeneration &+= 1
        pendingLibraryQuery = (libraryQueryGeneration, request)
        guard !isDrainingLibraryQueries else { return }

        isDrainingLibraryQueries = true
        defer { isDrainingLibraryQueries = false }

        while let pending = pendingLibraryQuery, lifecycle == .ready {
            pendingLibraryQuery = nil
            await performLibraryQuery(
                pending.request,
                generation: pending.generation
            )
        }
    }

    private func performLibraryQuery(
        _ request: LibraryQueryRequest,
        generation: UInt64
    ) async {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return
        }
        var exchangeHeld = true
        defer {
            if exchangeHeld { isExchangingCommand = false }
        }
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
                if generation == libraryQueryGeneration {
                    libraryState = .failed(failure.message)
                }
                return
            }
            isExchangingCommand = false
            exchangeHeld = false
            let snapshotDecoder = snapshotDecoder
            let libraryDecoder = libraryDecoder
            let decoded = try await Task.detached(priority: .userInitiated) {
                (
                    try snapshotDecoder.decode(
                        envelope,
                        endpointDescription: endpointDescription,
                        protocolVersion: protocolVersion
                    ),
                    try libraryDecoder.decode(envelope)
                )
            }.value
            let snapshot = decoded.0
            guard generation == libraryQueryGeneration else { return }
            guard snapshot.snapshotSequence >= (latestSnapshot?.snapshotSequence ?? 0) else {
                return
            }
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = decoded.1
        } catch {
            guard generation == libraryQueryGeneration else { return }
            libraryState = .failed(
                (error as? LocalizedError)?.errorDescription
                    ?? "The library query could not be completed."
            )
        }
    }

    func openLibraryTrackEditor(trackID: UInt64) async {
        timelineEditFeedback = nil
        await exchangeLibraryCommand(
            .openLibraryTrackEditor(trackID: trackID),
            failurePresentation: .timeline
        )
    }

    func closeLibraryTrackEditor() async {
        guard libraryState.editor != nil else { return }
        let closed = await exchangeLibraryCommand(
            .closeLibraryTrackEditor,
            failurePresentation: .timeline
        )
        if closed {
            timelineEditFeedback = nil
        }
    }

    func loadLibraryTrackOnLocalDeck(_ request: LibraryDeckLoadRequest) async {
        guard var snapshot = latestSnapshot else { return }
        localPlaybackFeedback = nil
        localPlaybackFeedbackIsError = false
        var waveforms = localPlaybackWaveforms
        waveforms.removeValue(forKey: request.deckID)
        localPlaybackWaveforms = waveforms

        if snapshot.deckSource.mode != "localPlayback" {
            let switched = await exchangeLibraryCommand(
                .selectDeckSourceMode(
                    "localPlayback",
                    expectedStateRevision: snapshot.stateRevision
                ),
                failurePresentation: .localPlayback,
                retryOnStateRevisionConflict: { actualRevision in
                    .selectDeckSourceMode(
                        "localPlayback",
                        expectedStateRevision: actualRevision
                    )
                }
            )
            guard switched, let refreshed = latestSnapshot else { return }
            snapshot = refreshed
        }

        var loadedTimelineRevision = request.expectedTimelineRevision
        let loaded = await exchangeLibraryCommand(
            .loadLibraryTrackOnLocalDeck(
                trackID: request.trackID,
                deckID: request.deckID,
                expectedTimelineRevision: request.expectedTimelineRevision,
                expectedStateRevision: snapshot.stateRevision
            ),
            failurePresentation: .localPlayback,
            retryOnStateRevisionConflict: { actualRevision in
                .loadLibraryTrackOnLocalDeck(
                    trackID: request.trackID,
                    deckID: request.deckID,
                    expectedTimelineRevision: request.expectedTimelineRevision,
                    expectedStateRevision: actualRevision
                )
            },
            retryOnTimelineRevisionConflict: { actualTimelineRevision, actualStateRevision in
                loadedTimelineRevision = actualTimelineRevision
                return .loadLibraryTrackOnLocalDeck(
                    trackID: request.trackID,
                    deckID: request.deckID,
                    expectedTimelineRevision: actualTimelineRevision,
                    expectedStateRevision: actualStateRevision
                )
            }
        )
        if loaded {
            localPlaybackFeedback = "Loaded exact Lumi timeline r\(loadedTimelineRevision) on Local Deck \(request.deckID)."
            localPlaybackFeedbackIsError = false
            await fetchLocalPlaybackWaveform(
                trackID: request.trackID,
                deckID: request.deckID
            )
        }
    }

    private func fetchLocalPlaybackWaveform(trackID: UInt64, deckID: UInt64) async {
        guard lifecycle == .ready,
              await acquireInteractiveExchange() else {
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .getLibraryTrackWaveform(trackID: trackID)
            )
            guard EngineCommandFailure(envelope) == nil else { return }
            let decoder = snapshotDecoder
            let detail = try await Task.detached(priority: .userInitiated) {
                try decoder.decodeWaveformDetail(envelope)
            }.value
            guard detail.trackID == trackID else { return }
            var waveforms = localPlaybackWaveforms
            waveforms[deckID] = detail.preview
            localPlaybackWaveforms = waveforms
        } catch {
            // The bounded preview remains available if detail retrieval fails.
        }
    }

    func editLibraryTimeline(_ request: TrackTimelineEditRequest) async {
        guard let editor = libraryState.editor else { return }
        let command: EngineCommand
        let success: String
        switch request {
        case let .setLoopStrategy(phraseIndex, strategy):
            guard let catalog = libraryState.autoloopCatalog else { return }
            command = .setLibraryPhraseLoopStrategy(
                trackID: editor.track.id,
                phraseIndex: phraseIndex,
                expectedTimelineRevision: editor.timeline.revision,
                expectedAutoloopCatalogRevision: catalog.revision,
                strategy: engineLoopStrategy(strategy)
            )
            success = "Loop strategy saved."
        default:
            command = .editLibraryTimeline(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                edit: engineTimelineEdit(request)
            )
            success = "Phrase timeline saved."
        }
        await exchangeTimelineCommand(command, success: success)
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

    func publishMidiSource() async {
        await exchangeMidiCommand(
            .publishMidiSource,
            success: "Lumi Virtual MIDI is published. No MIDI was sent."
        )
    }

    func stopMidiSource() async {
        await exchangeMidiCommand(
            .stopMidiSource,
            success: "Lumi Virtual MIDI stopped."
        )
    }

    func sendMidiLearnPulse() async {
        await exchangeMidiCommand(
            .sendMidiLearnPulse,
            success: "Learn pulse sent on Channel 16, Note 60, with Note Off."
        )
    }

    func sendMidiAddressLearnPulse(targetKind: String, targetNumber: UInt16) async {
        let label = targetKind == "bank" ? "Bank" : "AutoLoop"
        let note = targetKind == "bank" ? 59 + targetNumber : 63 + targetNumber
        await exchangeMidiCommand(
            .sendMidiAddressLearnPulse(
                targetKind: targetKind,
                targetNumber: targetNumber
            ),
            success: "\(label) \(targetNumber) learn pulse sent on Channel 16, Note \(note)."
        )
    }

    func triggerMidiAutoloop(bankNumber: UInt16, autoloopNumber: UInt16) async {
        await exchangeMidiCommand(
            .triggerMidiAutoloop(
                bankNumber: bankNumber,
                autoloopNumber: autoloopNumber
            ),
            success: "Triggered Bank \(bankNumber) → AutoLoop \(autoloopNumber) with a 50 ms settle delay."
        )
    }

    func reconcileLibrarySource(_ request: TrackSourceReconcileRequest) async {
        guard let editor = libraryState.editor else { return }
        let command: EngineCommand
        let success: String
        switch request {
        case .previewDemoChanges:
            command = .previewDemoSourceRefresh
            success = "Source changes compared. Nothing was written."
        case .keepLumi:
            command = .reconcileLibrarySource(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                strategy: .keepLumi
            )
            success = "Source refreshed; Lumi phrases kept."
        case .rebase:
            command = .reconcileLibrarySource(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                strategy: .rebase
            )
            success = "Lumi Phrase Points rebased to whole beats."
        case let .merge(choices):
            command = .reconcileLibrarySource(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                strategy: .merge(choices.map { choice in
                    EngineSourceConflictChoice(
                        phraseIndex: choice.phraseIndex,
                        side: choice.side.rawValue
                    )
                })
            )
            success = "Source conflicts merged as a new revision."
        case .replaceWithSource:
            command = .reconcileLibrarySource(
                trackID: editor.track.id,
                expectedTimelineRevision: editor.timeline.revision,
                strategy: .replaceWithSource
            )
            success = "Source phrases adopted; the previous Lumi revision remains recoverable."
        }
        await exchangeTimelineCommand(command, success: success)
    }

    func previewRekordboxXMLSync(_ request: RekordboxXMLSyncPreviewRequest) async {
        sourceImportFeedback = nil
        sourceImportFeedbackIsError = false
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            sourceImportFeedback = "The sync preview could not start because the engine is not ready."
            sourceImportFeedbackIsError = true
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .previewRekordboxXMLSync(
                    folder: request.folderPath,
                    followedPaths: request.followedPaths,
                    includeFutureChildPlaylists: request.includeFutureChildPlaylists
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                sourceImportFeedback = failure.message
                sourceImportFeedbackIsError = true
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "Rekordbox sync preview"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            guard let preview = libraryState.rekordboxSyncPreview else {
                sourceImportFeedback = "The engine returned no Rekordbox sync preview."
                sourceImportFeedbackIsError = true
                return
            }
            sourceImportFeedback = "Preview ready. No library data was changed. \(preview.uniqueTrackCount) unique tracks in \(preview.followedPlaylistCount) playlists."
        } catch {
            sourceImportFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The Rekordbox sync preview could not be completed."
            sourceImportFeedbackIsError = true
        }
    }

    func applyRekordboxXMLSync(
        _ request: RekordboxXMLSyncPreviewRequest,
        expectedContentSHA256: String
    ) async {
        sourceImportFeedback = nil
        sourceImportFeedbackIsError = false
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            sourceImportFeedback = "Apply Sync could not start because the engine is not ready."
            sourceImportFeedbackIsError = true
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .applyRekordboxXMLSync(
                    folder: request.folderPath,
                    followedPaths: request.followedPaths,
                    includeFutureChildPlaylists: request.includeFutureChildPlaylists,
                    expectedContentSHA256: expectedContentSHA256
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                sourceImportFeedback = failure.message
                sourceImportFeedbackIsError = true
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "Rekordbox sync"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            guard let mirror = libraryState.rekordboxMirror else {
                sourceImportFeedback = "The engine applied the sync but returned no mirror status."
                sourceImportFeedbackIsError = true
                return
            }
            sourceImportFeedback = "Sync applied safely. \(mirror.activeTracks) active tracks in \(mirror.playlists) playlists; \(mirror.archivedTracks) archived tracks retained."
        } catch {
            sourceImportFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The Rekordbox sync could not be applied."
            sourceImportFeedbackIsError = true
        }
    }

    func importRekordboxAnalysis(
        _ request: RekordboxXMLSyncPreviewRequest,
        expectedContentSHA256: String
    ) async {
        sourceImportFeedback = "Importing beatgrids, RGB waveforms and phrases from the closed Rekordbox library…"
        sourceImportFeedbackIsError = false
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            sourceImportFeedback = "Analysis import could not start because the engine is not ready."
            sourceImportFeedbackIsError = true
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .importRekordboxAnalysis(
                    folder: request.folderPath,
                    followedPaths: request.followedPaths,
                    includeFutureChildPlaylists: request.includeFutureChildPlaylists,
                    expectedContentSHA256: expectedContentSHA256
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                sourceImportFeedback = failure.message
                sourceImportFeedbackIsError = true
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "Rekordbox analysis import"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            sourceImportFeedback = "Rekordbox analysis imported. \(libraryState.collectionTotal) tracks are now available in Tracks."
        } catch {
            sourceImportFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The Rekordbox analysis could not be imported."
            sourceImportFeedbackIsError = true
        }
    }

    func mutatePhraseRoles(_ request: PhraseRoleMutationRequest) async {
        guard let settings = libraryState.phraseRoleSettings,
              lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .mutatePhraseRoleCatalog(
                    expectedPhraseRoleRevision: settings.revision,
                    mutation: enginePhraseRoleMutation(request)
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                if failure.code == "phraseRoleRevisionMismatch" {
                    let refreshed = try await supervisor.getSnapshot()
                    libraryState = try libraryDecoder.decode(refreshed)
                    phraseRoleFeedback = "Phrase roles changed elsewhere. Lumi refreshed the latest revision."
                } else {
                    phraseRoleFeedback = failure.message
                }
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "phrase-role mutation"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            if let revision = libraryState.phraseRoleSettings?.revision {
                phraseRoleFeedback = "Phrase-role settings saved. Revision \(revision)."
            } else {
                phraseRoleFeedback = "Phrase-role settings saved."
            }
        } catch {
            phraseRoleFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The phrase-role change could not be saved."
        }
    }

    func mutateAutoloopCatalog(_ request: AutoloopCatalogMutationRequest) async {
        guard let catalog = libraryState.autoloopCatalog,
              lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(
                .mutateAutoloopCatalog(
                    expectedAutoloopCatalogRevision: catalog.revision,
                    mutation: engineAutoloopCatalogMutation(request)
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                if failure.code == "autoloopCatalogRevisionMismatch" {
                    let refreshed = try await supervisor.getSnapshot()
                    libraryState = try libraryDecoder.decode(refreshed)
                    autoloopCatalogFeedback = "Autoloop catalog changed elsewhere. Lumi refreshed the latest revision."
                } else {
                    autoloopCatalogFeedback = failure.message
                }
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "Autoloop catalog mutation"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            if let revision = libraryState.autoloopCatalog?.revision {
                autoloopCatalogFeedback = "Autoloop catalog saved. Revision \(revision)."
            } else {
                autoloopCatalogFeedback = "Autoloop catalog saved."
            }
        } catch {
            autoloopCatalogFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The Autoloop catalog change could not be saved."
        }
    }

    private func engineAutoloopCatalogMutation(
        _ request: AutoloopCatalogMutationRequest
    ) -> EngineAutoloopCatalogMutation {
        switch request {
        case let .renameTheme(themeID, displayName):
            .renameTheme(themeID: themeID, displayName: displayName)
        case let .addVariant(roleID, displayName):
            .addVariant(roleID: roleID, displayName: displayName)
        case let .renameVariant(roleID, variantID, displayName):
            .renameVariant(roleID: roleID, variantID: variantID, displayName: displayName)
        case let .moveVariantEarlier(roleID, variantID):
            .moveVariantEarlier(roleID: roleID, variantID: variantID)
        case let .moveVariantLater(roleID, variantID):
            .moveVariantLater(roleID: roleID, variantID: variantID)
        case let .archiveVariant(roleID, variantID):
            .archiveVariant(roleID: roleID, variantID: variantID)
        case let .restoreVariant(roleID, variantID):
            .restoreVariant(roleID: roleID, variantID: variantID)
        case let .setCell(themeID, roleID, variantID, displayName):
            .setCell(
                themeID: themeID,
                roleID: roleID,
                variantID: variantID,
                displayName: displayName
            )
        case let .setButton(themeID, buttonNumber, roleID, displayName):
            .setButton(
                themeID: themeID,
                buttonNumber: buttonNumber,
                roleID: roleID,
                displayName: displayName
            )
        case let .clearButton(themeID, buttonNumber):
            .clearButton(themeID: themeID, buttonNumber: buttonNumber)
        }
    }

    private func enginePhraseRoleMutation(
        _ request: PhraseRoleMutationRequest
    ) -> EnginePhraseRoleMutation {
        switch request {
        case let .add(displayName):
            .add(displayName: displayName)
        case let .rename(roleID, displayName):
            .rename(roleID: roleID, displayName: displayName)
        case let .moveEarlier(roleID):
            .moveEarlier(roleID: roleID)
        case let .moveLater(roleID):
            .moveLater(roleID: roleID)
        case let .archive(roleID):
            .archive(roleID: roleID)
        case let .restore(roleID):
            .restore(roleID: roleID)
        case let .setSourceMapping(providerKind, rawLabel, roleID):
            .setSourceMapping(providerKind: providerKind, rawLabel: rawLabel, roleID: roleID)
        }
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
                    timelineEditFeedback = failure.code == "autoloopCatalogRevisionMismatch"
                        ? "Autoloop catalog changed elsewhere. Lumi refreshed the latest revision."
                        : "Timeline changed elsewhere. Lumi refreshed the latest revision."
                } else {
                    timelineEditFeedback = failure.message
                }
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "timeline mutation"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            let revision = libraryState.editor?.timeline.revision
            timelineEditFeedback = revision.map { "\(success) Revision \($0)." } ?? success
        } catch {
            timelineEditFeedback = (error as? LocalizedError)?.errorDescription
                ?? "The phrase timeline edit could not be saved."
        }
    }

    private func engineTimelineEdit(_ request: TrackTimelineEditRequest) -> EngineTimelineEdit {
        switch request {
        case let .create(startBeat, endBeat, roleID):
            .create(startBeat: startBeat, endBeat: endBeat, roleID: roleID)
        case let .split(phraseIndex, atBeat):
            .split(phraseIndex: phraseIndex, atBeat: atBeat)
        case let .mergePrevious(phraseIndex):
            .mergePrevious(phraseIndex: phraseIndex)
        case let .mergeNext(phraseIndex):
            .mergeNext(phraseIndex: phraseIndex)
        case let .moveBoundary(phraseIndex, toBeat):
            .moveBoundary(afterPhraseIndex: phraseIndex, toBeat: toBeat)
        case let .deleteAbsorbPrevious(phraseIndex):
            .deleteAbsorbPrevious(phraseIndex: phraseIndex)
        case let .deleteAbsorbNext(phraseIndex):
            .deleteAbsorbNext(phraseIndex: phraseIndex)
        case let .changeRole(phraseIndex, roleID):
            .changeRole(phraseIndex: phraseIndex, roleID: roleID)
        case .setLoopStrategy:
            preconditionFailure("Loop strategies use their dedicated revision-safe command")
        }
    }

    private func engineLoopStrategy(
        _ request: TrackLoopStrategyRequest
    ) -> EnginePhraseLoopStrategy {
        switch request {
        case .automatic:
            .automatic
        case let .fixedVariant(variantID):
            .fixedVariant(variantID)
        case let .themeSpecificExact(overrides):
            .themeSpecificExact(overrides.map { value in
                EngineThemeVariantOverride(themeID: value.themeID, variantID: value.variantID)
            })
        }
    }

    @discardableResult
    private func exchangeLibraryCommand(
        _ command: EngineCommand,
        failurePresentation: LibraryCommandFailurePresentation = .library,
        retryOnStateRevisionConflict: ((UInt64) -> EngineCommand)? = nil,
        retryOnTimelineRevisionConflict: ((UInt64, UInt64) -> EngineCommand)? = nil
    ) async -> Bool {
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            return false
        }
        defer { isExchangingCommand = false }
        do {
            var envelope = try await supervisor.send(command)
            var effectiveStateRevision = latestSnapshot?.stateRevision
            if let failure = EngineCommandFailure(envelope),
               failure.kind == "revisionConflict",
               let actualRevision = failure.actualStateRevision,
               let retryOnStateRevisionConflict {
                effectiveStateRevision = actualRevision
                envelope = try await supervisor.send(
                    retryOnStateRevisionConflict(actualRevision)
                )
            }
            if let failure = EngineCommandFailure(envelope),
               failure.kind == "revisionConflict",
               let actualTimelineRevision = failure.actualTimelineRevision,
               let effectiveStateRevision,
               let retryOnTimelineRevisionConflict {
                envelope = try await supervisor.send(
                    retryOnTimelineRevisionConflict(
                        actualTimelineRevision,
                        effectiveStateRevision
                    )
                )
            }
            if let failure = EngineCommandFailure(envelope) {
                presentLibraryCommandFailure(
                    failure.message,
                    as: failurePresentation
                )
                return false
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "library command"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            synchronizeLocalAudio(with: snapshot)
            return true
        } catch {
            let message = (error as? LocalizedError)?.errorDescription
                ?? "The track editor could not be updated."
            presentLibraryCommandFailure(message, as: failurePresentation)
            return false
        }
    }

    private func decodeSnapshotWithRecovery(
        _ envelope: MessageEnvelope,
        endpointDescription: String,
        protocolVersion: Int,
        context: String
    ) async throws -> (EngineSnapshot, MessageEnvelope) {
        let decoder = snapshotDecoder
        do {
            let snapshot = try await Task.detached(priority: .userInitiated) {
                try decoder.decode(
                    envelope,
                    endpointDescription: endpointDescription,
                    protocolVersion: protocolVersion
                )
            }.value
            return (snapshot, envelope)
        } catch {
            Self.logger.error(
                "Snapshot decode failed after \(context, privacy: .public); requesting authoritative recovery snapshot: \(error.localizedDescription, privacy: .public)"
            )
            let recoveryEnvelope = try await supervisor.getSnapshot()
            let snapshot = try await Task.detached(priority: .userInitiated) {
                try decoder.decode(
                    recoveryEnvelope,
                    endpointDescription: endpointDescription,
                    protocolVersion: protocolVersion
                )
            }.value
            Self.logger.info(
                "Recovered authoritative engine snapshot after \(context, privacy: .public)"
            )
            return (snapshot, recoveryEnvelope)
        }
    }

    private func presentLibraryCommandFailure(
        _ message: String,
        as presentation: LibraryCommandFailurePresentation
    ) {
        switch presentation {
        case .library:
            libraryState = .failed(message)
        case .timeline:
            timelineEditFeedback = message
        case .localPlayback:
            localPlaybackFeedback = message
            localPlaybackFeedbackIsError = true
        }
    }

    private func exchangeMidiCommand(_ command: EngineCommand, success: String) async {
        midiIntegrationFeedback = nil
        guard lifecycle == .ready,
              let endpointDescription,
              let protocolVersion,
              await acquireInteractiveExchange() else {
            midiIntegrationFeedback = "The MIDI command could not run because the engine is not ready."
            return
        }
        defer { isExchangingCommand = false }
        do {
            let envelope = try await supervisor.send(command)
            if let failure = EngineCommandFailure(envelope) {
                midiIntegrationFeedback = "The MIDI command could not run: \(failure.message)"
                return
            }
            let (snapshot, snapshotEnvelope) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "MIDI command"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(snapshot)
            libraryState = try libraryDecoder.decode(snapshotEnvelope)
            midiIntegrationFeedback = success
        } catch {
            midiIntegrationFeedback = "The MIDI command could not run: \((error as? LocalizedError)?.errorDescription ?? error.localizedDescription)"
        }
    }

    func setLightingTimingOffset(_ millis: Int) async {
        let clamped = max(-250, min(250, millis))
        await exchangeMidiCommand(
            .setOutputTimingOffset(millis: Int16(clamped)),
            success: "Lighting timing set to \(String(format: "%+d ms", clamped))."
        )
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

            let (snapshot, _) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "plan mutation"
            )
            latestSnapshot = snapshot
            let savedRevision = [snapshot.livePlan, snapshot.nextPlan]
                .compactMap { $0 }
                .first(where: { $0.planID == request.context.planID })?
                .revision
            workspaceState = LiveWorkspacePresenter.ready(
                snapshot,
                planInteraction: .succeeded(
                    "Plan revision \(savedRevision ?? 0) saved."
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
        let presentationSnapshot = switch request {
        case let .setLocalPlaybackLeader(deckID, _):
            current.optimisticallySettingLocalPlaybackLeader(deckID)
        default:
            current
        }
        workspaceState = LiveWorkspacePresenter.ready(
            presentationSnapshot,
            sessionInteraction: .submitting
        )
        do {
            var envelope = try await supervisor.send(engineCommand(for: request))
            if let failure = EngineCommandFailure(envelope),
               failure.kind == "revisionConflict",
               let actualRevision = failure.actualStateRevision {
                envelope = try await supervisor.send(
                    engineCommand(for: request.withStateRevision(actualRevision))
                )
            }
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
            let (snapshot, _) = try await decodeSnapshotWithRecovery(
                envelope,
                endpointDescription: endpointDescription,
                protocolVersion: protocolVersion,
                context: "session command"
            )
            latestSnapshot = snapshot
            workspaceState = LiveWorkspacePresenter.ready(
                snapshot,
                sessionInteraction: .succeeded(sessionSuccessMessage(request))
            )
            synchronizeLocalAudio(with: snapshot)
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
        case let .selectThemeFromPhrase(context, phraseIndex, themeID):
            .selectThemeFromPhrase(
                context: engineContext(context),
                phraseIndex: phraseIndex,
                themeID: themeID
            )
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
        case let .setOperationState(state, expectedRevision):
            .setOperationState(state, expectedStateRevision: expectedRevision)
        case let .setLocalPlaybackLeader(deckID, expectedRevision):
            .setLocalPlaybackLeader(
                deckID: deckID,
                expectedStateRevision: expectedRevision
            )
        case let .selectDeckSourceMode(mode, expectedRevision):
            .selectDeckSourceMode(mode, expectedStateRevision: expectedRevision)
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
        case let .setOperationState(state, _): "Operation state is now \(state.uppercased())."
        case let .setLocalPlaybackLeader(deckID, _): "Local Deck \(deckID) is now Live."
        case let .selectDeckSourceMode(mode, _):
            mode == "localPlayback" ? "Local Playback selected." : "Live Decks selected."
        }
    }

    func runLocalPlayback(_ request: LocalPlaybackRequest) {
        let deckID: UInt64
        switch request {
        case let .togglePlayback(value), let .stop(value), let .seek(value, _):
            deckID = value
        }
        guard latestSnapshot?.deckSource.mode == "localPlayback",
              let controller = localAudioControllers[deckID] else {
            return
        }
        switch request {
        case .togglePlayback:
            controller.togglePlayback()
        case .stop:
            controller.stop()
        case let .seek(_, progress):
            controller.seek(progress: progress)
            discardPendingLocalTransport(for: deckID)
        }
        updateLocalPlaybackVisualClock(controller.snapshot)
        publishLocalTransport(controller.snapshot)
    }

    private func updateLocalPlaybackVisualClock(_ transport: LocalDeckTransportSnapshot) {
        var clocks = localPlaybackVisualClocks
        clocks[transport.deckID] = LocalPlaybackVisualClockSnapshot(
            trackLoadID: transport.trackLoadID,
            positionMillis: transport.positionMillis,
            durationMillis: transport.durationMillis,
            playing: transport.playing,
            anchoredAtReferenceTime: Date.timeIntervalSinceReferenceDate
        )
        localPlaybackVisualClocks = clocks
    }

    private func discardPendingLocalTransport(for deckID: UInt64) {
        pendingLocalTransports.removeValue(forKey: deckID)
        pendingLocalTransportDecks.removeAll(where: { $0 == deckID })
    }

    private func synchronizeLocalAudio(with snapshot: EngineSnapshot) {
        guard snapshot.deckSource.mode == "localPlayback" else {
            localAudioControllers.values.forEach { $0.shutdown() }
            localAudioControllers.removeAll()
            localPlaybackVisualClocks = [:]
            localPlaybackWaveforms = [:]
            return
        }
        let expectedDecks = Set(snapshot.decks.compactMap { deck in
            deck.localPlayback == nil ? nil : deck.deckID
        })
        for deckID in Array(localAudioControllers.keys) where !expectedDecks.contains(deckID) {
            localAudioControllers.removeValue(forKey: deckID)?.shutdown()
            var clocks = localPlaybackVisualClocks
            clocks.removeValue(forKey: deckID)
            localPlaybackVisualClocks = clocks
            var waveforms = localPlaybackWaveforms
            waveforms.removeValue(forKey: deckID)
            localPlaybackWaveforms = waveforms
        }
        for deck in snapshot.decks {
            guard let playback = deck.localPlayback else { continue }
            if let existing = localAudioControllers[deck.deckID],
               existing.trackLoadID == deck.trackLoadID {
                continue
            }
            localAudioControllers.removeValue(forKey: deck.deckID)?.shutdown()
            let controller = LocalDeckAudioController(
                deckID: deck.deckID,
                trackLoadID: deck.trackLoadID,
                audioURI: playback.audioURI,
                durationMillis: playback.durationMillis
            )
            controller.onTransport = { [weak self, weak controller] in
                guard let self, let controller else { return }
                let transport = controller.snapshot
                if !transport.playing {
                    self.updateLocalPlaybackVisualClock(transport)
                }
                self.publishLocalTransport(transport)
            }
            localAudioControllers[deck.deckID] = controller
            updateLocalPlaybackVisualClock(controller.snapshot)
        }
    }

    private func publishLocalTransport(_ transport: LocalDeckTransportSnapshot) {
        if pendingLocalTransports[transport.deckID] == nil {
            pendingLocalTransportDecks.append(transport.deckID)
        }
        pendingLocalTransports[transport.deckID] = transport
        guard localTransportDrainTask == nil else { return }
        localTransportDrainTask = Task { [weak self] in
            await self?.drainLocalTransports()
        }
    }

    private func drainLocalTransports() async {
        defer { localTransportDrainTask = nil }
        while lifecycle == .ready, !Task.isCancelled {
            while isExchangingCommand || pendingInteractiveExchanges > 0 {
                do {
                    try await Task.sleep(for: .milliseconds(5))
                } catch {
                    return
                }
                guard lifecycle == .ready else { return }
            }
            guard let transport = dequeuePendingLocalTransport() else { return }
            await exchangeLocalTransport(transport)
        }
    }

    private func dequeuePendingLocalTransport() -> LocalDeckTransportSnapshot? {
        while !pendingLocalTransportDecks.isEmpty {
            let deckID = pendingLocalTransportDecks.removeFirst()
            if let transport = pendingLocalTransports.removeValue(forKey: deckID) {
                return transport
            }
        }
        return nil
    }

    private func exchangeLocalTransport(_ transport: LocalDeckTransportSnapshot) async {
        guard lifecycle == .ready,
              !isExchangingCommand,
              pendingInteractiveExchanges == 0 else {
            return
        }
        isExchangingCommand = true
        defer { isExchangingCommand = false }
        await sendLocalTransport(transport)
    }

    private func sendLocalTransport(_ transport: LocalDeckTransportSnapshot) async {
        do {
            let envelope = try await supervisor.send(
                .updateLocalPlaybackTransport(
                    deckID: transport.deckID,
                    trackLoadID: transport.trackLoadID,
                    positionMillis: transport.positionMillis,
                    playing: transport.playing
                )
            )
            if let failure = EngineCommandFailure(envelope) {
                Self.logger.error(
                    "Local transport update rejected for deck \(transport.deckID): \(failure.message, privacy: .public)"
                )
                return
            }
            guard localAudioControllers[transport.deckID]?.snapshot.discontinuityRevision
                    == transport.discontinuityRevision else {
                if let current = localAudioControllers[transport.deckID]?.snapshot {
                    publishLocalTransport(current)
                }
                return
            }
        } catch {
            Self.logger.error(
                "Local transport exchange failed for deck \(transport.deckID): \(error.localizedDescription, privacy: .public)"
            )
            // Audio remains local and the next transport sample retries safely.
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
        // Preserve phrase-boundary timing when Library/UI commands are queued:
        // flush at most one latest sample per deck before granting the lane.
        for _ in 0..<2 {
            guard let transport = dequeuePendingLocalTransport() else { break }
            await sendLocalTransport(transport)
        }
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
            var healthTick = 0
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(250))
                guard !Task.isCancelled, let self else {
                    return
                }
                healthTick = (healthTick + 1) % 4
                if healthTick == 0, await !self.supervisor.isRunning() {
                    self.lifecycle = .disconnected
                    self.workspaceState = LiveWorkspacePresenter.disconnected()
                    self.libraryState = .failed("The local Lumi engine disconnected.")
                    return
                }
                guard self.lifecycle == .ready,
                      let endpointDescription = self.endpointDescription,
                      let protocolVersion = self.protocolVersion else {
                    continue
                }
                let connectedDecks = self.latestSnapshot?.deckSource.mode == "connectedDecks"
                guard connectedDecks || healthTick == 0 else { continue }
                guard await self.acquireInteractiveExchange() else { continue }
                do {
                    let envelope = try await self.supervisor.getSnapshot()
                    self.isExchangingCommand = false
                    let snapshotDecoder = self.snapshotDecoder
                    let libraryDecoder = self.libraryDecoder
                    let decodeLibrary = healthTick == 0
                    let decoded = try await Task.detached(priority: .utility) {
                        (
                            try snapshotDecoder.decode(
                                envelope,
                                endpointDescription: endpointDescription,
                                protocolVersion: protocolVersion
                            ),
                            decodeLibrary ? try libraryDecoder.decode(envelope) : nil
                        )
                    }.value
                    let snapshot = decoded.0
                    guard snapshot.snapshotSequence >= (self.latestSnapshot?.snapshotSequence ?? 0)
                    else { continue }
                    self.latestSnapshot = snapshot
                    let nextWorkspaceState = LiveWorkspacePresenter.ready(snapshot)
                    if nextWorkspaceState != self.workspaceState {
                        self.workspaceState = nextWorkspaceState
                    }
                    if healthTick == 0 {
                        guard let nextLibraryState = decoded.1 else { continue }
                        if nextLibraryState != self.libraryState {
                            self.libraryState = nextLibraryState
                        }
                    }
                } catch {
                    self.isExchangingCommand = false
                    Self.logger.error(
                        "Engine snapshot monitor failed: \(error.localizedDescription, privacy: .public)"
                    )
                    // The one-second process health check owns disconnect state. A
                    // single missed polling frame must not disturb Live UI.
                }
            }
        }
    }
}

private extension SessionCommandRequest {
    func withStateRevision(_ revision: UInt64) -> Self {
        switch self {
        case let .setOperationState(state, _):
            .setOperationState(state, expectedStateRevision: revision)
        case let .setLocalPlaybackLeader(deckID, _):
            .setLocalPlaybackLeader(deckID, expectedStateRevision: revision)
        case let .selectDeckSourceMode(mode, _):
            .selectDeckSourceMode(mode, expectedStateRevision: revision)
        }
    }
}

private struct LocalDeckTransportSnapshot: Equatable, Sendable {
    let deckID: UInt64
    let trackLoadID: UInt64
    let positionMillis: UInt64
    let durationMillis: UInt64
    let playing: Bool
    let discontinuityRevision: UInt64
}

@MainActor
private final class LocalDeckAudioController {
    private enum AudioSource {
        case file(AVAudioFile)
        case buffer(AVAudioPCMBuffer)

        var format: AVAudioFormat {
            switch self {
            case let .file(file): file.processingFormat
            case let .buffer(buffer): buffer.format
            }
        }

        var frameLength: AVAudioFramePosition {
            switch self {
            case let .file(file): file.length
            case let .buffer(buffer): AVAudioFramePosition(buffer.frameLength)
            }
        }
    }

    let deckID: UInt64
    let trackLoadID: UInt64
    var onTransport: (() -> Void)?

    private let durationMillis: UInt64
    private var positionMillis: UInt64 = 0
    private var isPlaying = false
    private var engine: AVAudioEngine?
    private var player: AVAudioPlayerNode?
    private var source: AudioSource?
    private var updateTask: Task<Void, Never>?
    private var scheduledFrameCount: AVAudioFrameCount = 0
    private var scheduledStartMillis: UInt64 = 0
    private var playbackStartedAt: ContinuousClock.Instant?
    private var generation: UInt64 = 0
    private var discontinuityRevision: UInt64 = 0

    var snapshot: LocalDeckTransportSnapshot {
        LocalDeckTransportSnapshot(
            deckID: deckID,
            trackLoadID: trackLoadID,
            positionMillis: positionMillis,
            durationMillis: durationMillis,
            playing: isPlaying,
            discontinuityRevision: discontinuityRevision
        )
    }

    init(
        deckID: UInt64,
        trackLoadID: UInt64,
        audioURI: String,
        durationMillis: UInt64
    ) {
        self.deckID = deckID
        self.trackLoadID = trackLoadID
        self.durationMillis = durationMillis
        source = try? Self.loadSource(audioURI: audioURI, durationMillis: durationMillis)
    }

    func togglePlayback() {
        isPlaying ? pause() : play()
    }

    func play() {
        guard let source else { return }
        do {
            let (engine, player) = prepareEngine(format: source.format)
            if !engine.isRunning { try engine.start() }
            generation &+= 1
            let activeGeneration = generation
            player.stop()
            if positionMillis >= durationMillis { positionMillis = 0 }
            let sampleRate = source.format.sampleRate
            let requestedFrame = AVAudioFramePosition(
                Double(positionMillis) / 1_000 * sampleRate
            )
            let startFrame = min(requestedFrame, max(0, source.frameLength - 1))
            let remainingFrames = source.frameLength - startFrame
            guard remainingFrames > 0,
                  remainingFrames <= AVAudioFramePosition(UInt32.max) else { return }
            scheduledFrameCount = AVAudioFrameCount(remainingFrames)
            scheduledStartMillis = UInt64(
                (Double(startFrame) / sampleRate * 1_000).rounded()
            )
            positionMillis = scheduledStartMillis
            let completion: @Sendable () -> Void = { [weak self] in
                Task { @MainActor [weak self] in
                    guard let self, self.generation == activeGeneration else { return }
                    self.positionMillis = self.durationMillis
                    self.isPlaying = false
                    self.playbackStartedAt = nil
                    self.updateTask?.cancel()
                    self.updateTask = nil
                    self.onTransport?()
                }
            }
            switch source {
            case let .file(file):
                player.scheduleSegment(
                    file,
                    startingFrame: startFrame,
                    frameCount: scheduledFrameCount,
                    at: nil,
                    completionHandler: completion
                )
            case let .buffer(buffer):
                guard let slice = Self.slice(
                    buffer,
                    start: startFrame,
                    end: source.frameLength
                ) else { return }
                player.scheduleBuffer(slice, completionHandler: completion)
            }
            player.play()
            playbackStartedAt = ContinuousClock.now
            isPlaying = true
            startUpdates()
        } catch {
            isPlaying = false
        }
    }

    func pause() {
        refreshPosition()
        generation &+= 1
        player?.pause()
        isPlaying = false
        playbackStartedAt = nil
        updateTask?.cancel()
        updateTask = nil
    }

    func stop() {
        discontinuityRevision &+= 1
        generation &+= 1
        player?.stop()
        positionMillis = 0
        isPlaying = false
        playbackStartedAt = nil
        updateTask?.cancel()
        updateTask = nil
    }

    func seek(progress: Double) {
        let shouldResume = isPlaying
        discontinuityRevision &+= 1
        generation &+= 1
        player?.stop()
        positionMillis = UInt64(
            (min(max(0, progress), 1) * Double(durationMillis)).rounded()
        )
        isPlaying = false
        playbackStartedAt = nil
        updateTask?.cancel()
        updateTask = nil
        if shouldResume { play() }
    }

    func shutdown() {
        stop()
        engine?.stop()
        if let player { engine?.detach(player) }
        player = nil
        engine = nil
        source = nil
    }

    private func prepareEngine(format: AVAudioFormat) -> (AVAudioEngine, AVAudioPlayerNode) {
        if let engine, let player { return (engine, player) }
        let engine = AVAudioEngine()
        let player = AVAudioPlayerNode()
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: format)
        player.volume = 0.75
        self.engine = engine
        self.player = player
        return (engine, player)
    }

    private func startUpdates() {
        updateTask?.cancel()
        let activeGeneration = generation
        updateTask = Task { [weak self] in
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: .milliseconds(10))
                } catch {
                    return
                }
                guard let self,
                      self.isPlaying,
                      self.generation == activeGeneration else { return }
                self.refreshPosition()
                self.onTransport?()
            }
        }
    }

    private func refreshPosition() {
        guard let playbackStartedAt else { return }
        let elapsed = playbackStartedAt.duration(to: ContinuousClock.now).components
        let elapsedMillis = max(
            0,
            Double(elapsed.seconds) * 1_000
                + Double(elapsed.attoseconds) / 1_000_000_000_000_000
        )
        positionMillis = min(
            durationMillis,
            scheduledStartMillis + UInt64(elapsedMillis.rounded())
        )
    }

    private static func loadSource(
        audioURI: String,
        durationMillis: UInt64
    ) throws -> AudioSource {
        if audioURI.hasPrefix("lumi-demo://") {
            return .buffer(
                try syntheticBuffer(seed: audioURI, durationMillis: durationMillis)
            )
        }
        let url: URL
        if audioURI.hasPrefix("/") {
            url = URL(fileURLWithPath: audioURI)
        } else if let candidate = URL(string: audioURI), candidate.isFileURL {
            url = candidate
        } else {
            throw CocoaError(.fileReadUnsupportedScheme)
        }
        let file = try AVAudioFile(forReading: url)
        guard file.length > 0,
              file.length <= AVAudioFramePosition(UInt32.max) else {
            throw CocoaError(.fileReadCorruptFile)
        }
        return .file(file)
    }

    private static func syntheticBuffer(
        seed: String,
        durationMillis: UInt64
    ) throws -> AVAudioPCMBuffer {
        let sampleRate = 44_100.0
        let frameCount64 = durationMillis * 44_100 / 1_000
        guard frameCount64 > 0,
              frameCount64 <= UInt64(UInt32.max),
              let format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1),
              let buffer = AVAudioPCMBuffer(
                pcmFormat: format,
                frameCapacity: AVAudioFrameCount(frameCount64)
              ),
              let samples = buffer.floatChannelData?[0] else {
            throw CocoaError(.fileReadCorruptFile)
        }
        buffer.frameLength = AVAudioFrameCount(frameCount64)
        let hash = seed.utf8.reduce(UInt32(2_166_136_261)) { ($0 ^ UInt32($1)) &* 16_777_619 }
        let frequency = 110.0 + Double(hash % 220)
        for frame in 0..<Int(frameCount64) {
            let time = Double(frame) / sampleRate
            let saw = Float((time * frequency).truncatingRemainder(dividingBy: 1) * 2 - 1)
            let pulse = Float(exp(-18 * time.truncatingRemainder(dividingBy: 0.5)))
            samples[frame] = saw * 0.12 + pulse * 0.08
        }
        return buffer
    }

    private static func slice(
        _ source: AVAudioPCMBuffer,
        start: AVAudioFramePosition,
        end: AVAudioFramePosition
    ) -> AVAudioPCMBuffer? {
        let count = AVAudioFrameCount(end - start)
        guard count > 0,
              let result = AVAudioPCMBuffer(pcmFormat: source.format, frameCapacity: count),
              let sourceChannels = source.floatChannelData,
              let resultChannels = result.floatChannelData else { return nil }
        result.frameLength = count
        for channel in 0..<Int(source.format.channelCount) {
            resultChannels[channel].update(
                from: sourceChannels[channel].advanced(by: Int(start)),
                count: Int(count)
            )
        }
        return result
    }
}
