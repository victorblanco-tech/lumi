import Foundation
import LumiProtocol
import Testing
@testable import LumiEngineClient

@Test("Typed engine command failures preserve revision conflict details")
func decodesCommandFailure() {
    let envelope = MessageEnvelope(
        protocolVersion: 1,
        messageType: .error,
        messageId: "error-1",
        sequence: 2,
        correlationId: "command-1",
        sentAt: "2026-08-03T12:00:00Z",
        payload: [
            "kind": .string("revisionConflict"),
            "code": .string("planRevisionMismatch"),
            "message": .string("The plan changed before the command was applied."),
            "retryable": .boolean(true),
            "actualPlanRevision": .number(4),
            "actualStateRevision": .number(11),
            "actualTimelineRevision": .number(7),
            "actualPhraseRoleRevision": .number(3),
            "actualAutoloopCatalogRevision": .number(9)
        ]
    )

    let failure = EngineCommandFailure(envelope)
    #expect(failure?.kind == "revisionConflict")
    #expect(failure?.actualPlanRevision == 4)
    #expect(failure?.actualStateRevision == 11)
    #expect(failure?.actualTimelineRevision == 7)
    #expect(failure?.actualPhraseRoleRevision == 3)
    #expect(failure?.actualAutoloopCatalogRevision == 9)
    #expect(failure?.retryable == true)
}

@Test("The Swift client launches and authenticates the real Rust engine")
func launchesRealEngine() async throws {
    let environment = ProcessInfo.processInfo.environment
    guard let executablePath = environment["LUMI_ENGINE_TEST_EXECUTABLE"] else {
        Issue.record("LUMI_ENGINE_TEST_EXECUTABLE is required")
        return
    }

    let supervisor = EngineProcessSupervisor()
    let databaseURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("lumi-swift-engine-\(UUID().uuidString).sqlite")
    do {
        let endpoint = try await supervisor.launch(
            engineExecutable: URL(fileURLWithPath: executablePath),
            libraryDatabaseURL: databaseURL
        )
        #expect(endpoint.host == "127.0.0.1")
        #expect(endpoint.protocolVersion == WireProtocol.version)

        var snapshot = try await supervisor.connect(to: endpoint)
        #expect(snapshot.messageType == .snapshot)
        #expect(snapshot.sequence == 1)
        #expect(libraryTrackTitles(snapshot).count == 3)
        #expect(librarySourceName(snapshot) == "Lumi Demo Library")
        #expect(phraseRoleRevision(snapshot) == 1)
        #expect(phraseRoleName(snapshot, roleID: "synth") == "Synth")
        #expect(autoloopCatalogRevision(snapshot) == 1)
        #expect(autoloopThemeNames(snapshot).count == 4)
        #expect(autoloopVariantCount(snapshot, roleID: "synth") == 1)

        let renamedRole = try await supervisor.send(
            .mutatePhraseRoleCatalog(
                expectedPhraseRoleRevision: 1,
                mutation: .rename(roleID: "synth", displayName: "Lead Synth")
            ),
            messageID: "swift-rename-phrase-role"
        )
        #expect(phraseRoleRevision(renamedRole) == 2)
        #expect(phraseRoleName(renamedRole, roleID: "synth") == "Lead Synth")
        #expect(stateRevision(renamedRole) == stateRevision(snapshot))

        let staleRole = try await supervisor.send(
            .mutatePhraseRoleCatalog(
                expectedPhraseRoleRevision: 1,
                mutation: .rename(roleID: "synth", displayName: "Stale Name")
            ),
            messageID: "swift-stale-phrase-role"
        )
        #expect(EngineCommandFailure(staleRole)?.code == "phraseRoleRevisionMismatch")
        #expect(EngineCommandFailure(staleRole)?.actualPhraseRoleRevision == 2)

        snapshot = try await supervisor.send(
            .mutatePhraseRoleCatalog(
                expectedPhraseRoleRevision: 2,
                mutation: .setSourceMapping(
                    providerKind: "demo",
                    rawLabel: "Intro",
                    roleID: "synth"
                )
            ),
            messageID: "swift-map-source-phrase"
        )
        #expect(phraseRoleRevision(snapshot) == 3)

        let renamedTheme = try await supervisor.send(
            .mutateAutoloopCatalog(
                expectedAutoloopCatalogRevision: 1,
                mutation: .renameTheme(themeID: 1, displayName: "Electric Garden")
            ),
            messageID: "swift-rename-autoloop-theme"
        )
        #expect(autoloopCatalogRevision(renamedTheme) == 2)
        #expect(autoloopThemeNames(renamedTheme).first == "Electric Garden")

        snapshot = try await supervisor.send(
            .mutateAutoloopCatalog(
                expectedAutoloopCatalogRevision: 2,
                mutation: .setButton(
                    themeID: 1,
                    buttonNumber: 1,
                    roleID: "drop",
                    displayName: "DROP BLUE PINK - NEW 1"
                )
            ),
            messageID: "swift-map-autoloop-button"
        )
        #expect(autoloopCatalogRevision(snapshot) == 3)
        #expect(autoloopButtonName(snapshot, themeID: 1, buttonNumber: 1) == "DROP BLUE PINK - NEW 1")
        #expect(autoloopButtonRole(snapshot, themeID: 1, buttonNumber: 1) == "drop")
        #expect(autoloopMissingCellCount(snapshot) == 0)

        let staleAutoloop = try await supervisor.send(
            .mutateAutoloopCatalog(
                expectedAutoloopCatalogRevision: 1,
                mutation: .renameTheme(themeID: 2, displayName: "Stale Theme")
            ),
            messageID: "swift-stale-autoloop-catalog"
        )
        #expect(EngineCommandFailure(staleAutoloop)?.code == "autoloopCatalogRevisionMismatch")
        #expect(EngineCommandFailure(staleAutoloop)?.actualAutoloopCatalogRevision == 3)

        let searchedLibrary = try await supervisor.send(
            .queryLibrary(search: "Northern", playlistID: nil, offset: 0, limit: 50),
            messageID: "swift-library-search"
        )
        #expect(libraryTrackTitles(searchedLibrary) == ["Northern Pulse"])

        let editorRevision = stateRevision(searchedLibrary)
        let openedEditor = try await supervisor.send(
            .openLibraryTrackEditor(trackID: requiredFirstLibraryTrackID(snapshot)),
            messageID: "swift-open-library-editor"
        )
        #expect(stateRevision(openedEditor) == editorRevision)
        #expect(libraryEditorTitle(openedEditor) == libraryTrackTitles(snapshot).first)
        #expect(libraryEditorAudioURI(openedEditor)?.hasPrefix("lumi-demo://") == true)
        #expect(libraryEditorArrayCount(openedEditor, field: "waveform") ?? 0 > 0)
        #expect(libraryEditorArrayCount(openedEditor, field: "phrases") ?? 0 > 0)
        #expect(libraryEditorArrayCount(openedEditor, field: "sourcePhrases") ?? 0 > 0)
        #expect(libraryEditorFirstRoleID(openedEditor) == "intro-outro")
        #expect(libraryEditorBeatCount(openedEditor) ?? 0 > 0)
        #expect(libraryEditorTimelineRevision(openedEditor) == 1)
        let comparedSource = try await supervisor.send(
            .previewDemoSourceRefresh,
            messageID: "swift-preview-source-refresh"
        )
        #expect(libraryEditorReconciliation(comparedSource)?["metadataOnly"] == .boolean(true))
        #expect(librarySourceRefreshChangeCount(comparedSource) == 3)
        let refreshedMetadata = try await supervisor.send(
            .reconcileLibrarySource(
                trackID: requiredFirstLibraryTrackID(snapshot),
                expectedTimelineRevision: 1,
                strategy: .keepLumi
            ),
            messageID: "swift-keep-lumi-metadata-refresh"
        )
        #expect(libraryEditorTitle(refreshedMetadata) == "Afterglow Drive (Extended)")
        #expect(libraryEditorTimelineRevision(refreshedMetadata) == 1)
        #expect(librarySourceRefreshChangeCount(refreshedMetadata) == 2)
        let editedTimeline = try await supervisor.send(
            .editLibraryTimeline(
                trackID: requiredFirstLibraryTrackID(snapshot),
                expectedTimelineRevision: 1,
                edit: .split(phraseIndex: 0, atBeat: 16)
            ),
            messageID: "swift-split-library-timeline"
        )
        #expect(stateRevision(editedTimeline) == editorRevision)
        #expect(planRevision(editedTimeline) == planRevision(openedEditor))
        #expect(outputRecordCount(editedTimeline) == 0)
        #expect(libraryEditorTimelineRevision(editedTimeline) == 2)
        #expect(libraryEditorArrayCount(editedTimeline, field: "phrases") == 5)

        let staleTimeline = try await supervisor.send(
            .editLibraryTimeline(
                trackID: requiredFirstLibraryTrackID(snapshot),
                expectedTimelineRevision: 1,
                edit: .changeRole(phraseIndex: 0, roleID: "synth")
            ),
            messageID: "swift-stale-library-timeline"
        )
        #expect(EngineCommandFailure(staleTimeline)?.kind == "revisionConflict")
        #expect(EngineCommandFailure(staleTimeline)?.actualTimelineRevision == 2)

        let undoneTimeline = try await supervisor.send(
            .undoLibraryTimeline(
                trackID: requiredFirstLibraryTrackID(snapshot),
                expectedTimelineRevision: 2
            ),
            messageID: "swift-undo-library-timeline"
        )
        #expect(libraryEditorTimelineRevision(undoneTimeline) == 3)
        #expect(libraryEditorCanRedo(undoneTimeline) == true)
        #expect(libraryEditorArrayCount(undoneTimeline, field: "phrases") == 4)
        let closedEditor = try await supervisor.send(
            .closeLibraryTrackEditor,
            messageID: "swift-close-library-editor"
        )
        #expect(stateRevision(closedEditor) == editorRevision)
        #expect(libraryEditorIsClosed(closedEditor))

        await supervisor.stop()
        let restartedEndpoint = try await supervisor.launch(
            engineExecutable: URL(fileURLWithPath: executablePath),
            libraryDatabaseURL: databaseURL
        )
        snapshot = try await supervisor.connect(to: restartedEndpoint)
        let libraryTitlesBeforeThemeOverride = libraryTrackTitles(snapshot)
        #expect(phraseRoleRevision(snapshot) == 3)
        #expect(phraseRoleName(snapshot, roleID: "synth") == "Lead Synth")
        #expect(autoloopCatalogRevision(snapshot) == 3)
        #expect(autoloopThemeNames(snapshot).first == "Electric Garden")
        #expect(autoloopButtonName(snapshot, themeID: 1, buttonNumber: 1) == "DROP BLUE PINK - NEW 1")
        #expect(autoloopButtonRole(snapshot, themeID: 1, buttonNumber: 1) == "drop")
        #expect(autoloopMissingCellCount(snapshot) == 0)
        let reopenedEditor = try await supervisor.send(
            .openLibraryTrackEditor(trackID: requiredFirstLibraryTrackID(snapshot)),
            messageID: "swift-reopen-library-editor"
        )
        #expect(libraryEditorTimelineRevision(reopenedEditor) == 3)
        #expect(libraryEditorCanRedo(reopenedEditor) == true)
        let redoneTimeline = try await supervisor.send(
            .redoLibraryTimeline(
                trackID: requiredFirstLibraryTrackID(snapshot),
                expectedTimelineRevision: 3
            ),
            messageID: "swift-redo-library-timeline"
        )
        #expect(libraryEditorTimelineRevision(redoneTimeline) == 4)
        #expect(libraryEditorArrayCount(redoneTimeline, field: "phrases") == 5)
        let fixedLoop = try await supervisor.send(
            .setLibraryPhraseLoopStrategy(
                trackID: requiredFirstLibraryTrackID(snapshot),
                phraseIndex: 0,
                expectedTimelineRevision: 4,
                expectedAutoloopCatalogRevision: 3,
                strategy: .fixedVariant("mapping-1")
            ),
            messageID: "swift-fix-phrase-loop-variant"
        )
        #expect(libraryEditorTimelineRevision(fixedLoop) == 5)
        #expect(libraryEditorFirstLoopStrategyKind(fixedLoop) == "fixedVariant")
        #expect(libraryEditorFirstFixedVariantID(fixedLoop) == "mapping-1")

        let staleLoopCatalog = try await supervisor.send(
            .setLibraryPhraseLoopStrategy(
                trackID: requiredFirstLibraryTrackID(snapshot),
                phraseIndex: 0,
                expectedTimelineRevision: 5,
                expectedAutoloopCatalogRevision: 2,
                strategy: .automatic
            ),
            messageID: "swift-stale-phrase-loop-catalog"
        )
        #expect(EngineCommandFailure(staleLoopCatalog)?.code == "autoloopCatalogRevisionMismatch")
        #expect(EngineCommandFailure(staleLoopCatalog)?.actualAutoloopCatalogRevision == 3)

        let automaticLoop = try await supervisor.send(
            .setLibraryPhraseLoopStrategy(
                trackID: requiredFirstLibraryTrackID(snapshot),
                phraseIndex: 0,
                expectedTimelineRevision: 5,
                expectedAutoloopCatalogRevision: 3,
                strategy: .automatic
            ),
            messageID: "swift-reset-phrase-loop-auto"
        )
        #expect(libraryEditorTimelineRevision(automaticLoop) == 6)
        #expect(libraryEditorFirstLoopStrategyKind(automaticLoop) == "auto")
        _ = try await supervisor.send(
            .closeLibraryTrackEditor,
            messageID: "swift-close-reopened-library-editor"
        )

        let unknownEditor = try await supervisor.send(
            .openLibraryTrackEditor(trackID: 999_999),
            messageID: "swift-open-unknown-library-editor"
        )
        #expect(EngineCommandFailure(unknownEditor)?.kind == "commandFailed")

        #expect(planThemeName(snapshot) == "Deep Ocean")
        #expect(planThemeReason(snapshot) == "colorPrefer")
        #expect(planMatchedColorRGB(snapshot) == 4_747_469)
        #expect(planCueThemeIDs(snapshot) == [2])

        guard case let .object(plan) = snapshot.payload["nextPlan"],
              case let .string(planID) = plan["planId"] else {
            Issue.record("Initial snapshot must contain a plan ID")
            await supervisor.stop()
            return
        }
        let context = EnginePlanCommandContext(
            planID: planID,
            trackLoadID: 2_001,
            expectedPlanRevision: 1
        )
        let command = EngineCommand.selectTheme(context: context, themeID: 1)
        let revised = try await supervisor.send(command, messageID: "swift-theme-1")
        #expect(planRevision(revised) == 2)
        #expect(planThemeName(revised) == "Electric Bloom")
        #expect(planThemeReason(revised) == "planInstanceUserChoice")
        #expect(planMatchedColorRGB(revised) == nil)
        #expect(planCueThemeIDs(revised) == [1])
        #expect(libraryTrackTitles(revised) == libraryTitlesBeforeThemeOverride)
        #expect(libraryEditorIsClosed(revised))

        let duplicate = try await supervisor.send(command, messageID: "swift-theme-1")
        #expect(planRevision(duplicate) == 2)

        let conflict = try await supervisor.send(
            command,
            messageID: "swift-stale-theme"
        )
        #expect(EngineCommandFailure(conflict)?.kind == "revisionConflict")
        #expect(EngineCommandFailure(conflict)?.actualPlanRevision == 2)

        let refreshed = try await supervisor.getSnapshot()
        #expect(planRevision(refreshed) == 2)

        guard let initialStateRevision = stateRevision(refreshed) else {
            Issue.record("Initial snapshot must contain a state revision")
            await supervisor.stop()
            return
        }
        let speed = try await supervisor.send(
            .setSimulationSpeed(64, expectedStateRevision: initialStateRevision)
        )
        #expect(simulationSpeed(speed) == 64)
        let armed = try await supervisor.send(
            .setOperationState(
                "armed",
                expectedStateRevision: requiredStateRevision(speed)
            )
        )
        #expect(operationState(armed) == "armed")
        let live = try await supervisor.send(
            .setOperationState(
                "live",
                expectedStateRevision: requiredStateRevision(armed)
            )
        )
        #expect(operationState(live) == "live")
        let leaderAdvanced = try await supervisor.send(
            .advanceToNextTrack(
                expectedStateRevision: requiredStateRevision(live)
            )
        )
        let played = try await supervisor.send(
            .advanceSimulation(
                elapsedTicks: 1_000,
                expectedStateRevision: requiredStateRevision(leaderAdvanced)
            )
        )
        #expect(outputRecordCount(played) == 4)
        #expect(timelineContainsSimulatedOutput(played))

        let staleState = try await supervisor.send(
            .setOperationState("armed", expectedStateRevision: 0)
        )
        #expect(EngineCommandFailure(staleState)?.code == "stateRevisionMismatch")
        #expect(
            EngineCommandFailure(staleState)?.actualStateRevision
                == requiredStateRevision(played)
        )

        let reset = try await supervisor.send(
            .resetDemoSession(
                expectedStateRevision: requiredStateRevision(played)
            )
        )
        #expect(operationState(reset) == "off")
        #expect(simulationSpeed(reset) == 1)
        #expect(outputRecordCount(reset) == 0)
        #expect(await supervisor.isRunning())
        await supervisor.stop()
        #expect(await !supervisor.isRunning())
        await #expect(throws: EngineClientError.connectionClosed) {
            try await supervisor.getSnapshot()
        }
        try? FileManager.default.removeItem(at: databaseURL)
        try? FileManager.default.removeItem(atPath: databaseURL.path + "-wal")
        try? FileManager.default.removeItem(atPath: databaseURL.path + "-shm")
    } catch {
        await supervisor.stop()
        try? FileManager.default.removeItem(at: databaseURL)
        try? FileManager.default.removeItem(atPath: databaseURL.path + "-wal")
        try? FileManager.default.removeItem(atPath: databaseURL.path + "-shm")
        throw error
    }
}

@Test("A library track keeps its exact Lumi identity through Next and dry-run output")
func loadsLibraryTrackIntoSimulator() async throws {
    let environment = ProcessInfo.processInfo.environment
    guard let executablePath = environment["LUMI_ENGINE_TEST_EXECUTABLE"] else {
        Issue.record("LUMI_ENGINE_TEST_EXECUTABLE is required")
        return
    }
    let supervisor = EngineProcessSupervisor()
    let databaseURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("lumi-swift-library-simulator-\(UUID().uuidString).sqlite")
    defer {
        try? FileManager.default.removeItem(at: databaseURL)
        try? FileManager.default.removeItem(atPath: databaseURL.path + "-wal")
        try? FileManager.default.removeItem(atPath: databaseURL.path + "-shm")
    }
    do {
        let endpoint = try await supervisor.launch(
            engineExecutable: URL(fileURLWithPath: executablePath),
            libraryDatabaseURL: databaseURL
        )
        let initial = try await supervisor.connect(to: endpoint)
        let loaded = try await supervisor.send(
            .loadLibraryTrackOnSimulatorDeck(
                trackID: requiredFirstLibraryTrackID(initial),
                deckID: 2,
                expectedTimelineRevision: 1,
                expectedStateRevision: requiredStateRevision(initial)
            ),
            messageID: "swift-load-library-track-deck-2"
        )
        #expect(nextPlanLibraryTimelineRevision(loaded) == 1)
        #expect(nextPlanLibraryProvider(loaded) == "demo")
        #expect(nextPlanResolutionCount(loaded) == nextPlanCueCount(loaded))
        #expect(nextPlanResolutionFieldsAreComplete(loaded))

        let stale = try await supervisor.send(
            .loadLibraryTrackOnSimulatorDeck(
                trackID: requiredFirstLibraryTrackID(initial),
                deckID: 1,
                expectedTimelineRevision: 99,
                expectedStateRevision: requiredStateRevision(loaded)
            ),
            messageID: "swift-load-stale-library-track"
        )
        #expect(EngineCommandFailure(stale)?.code == "timelineRevisionMismatch")
        #expect(EngineCommandFailure(stale)?.actualTimelineRevision == 1)

        let armed = try await supervisor.send(
            .setOperationState("armed", expectedStateRevision: requiredStateRevision(loaded))
        )
        let live = try await supervisor.send(
            .setOperationState("live", expectedStateRevision: requiredStateRevision(armed))
        )
        let activated = try await supervisor.send(
            .advanceToNextTrack(expectedStateRevision: requiredStateRevision(live))
        )
        let accelerated = try await supervisor.send(
            .setSimulationSpeed(64, expectedStateRevision: requiredStateRevision(activated))
        )
        let completed = try await supervisor.send(
            .advanceSimulation(
                elapsedTicks: 1_000,
                expectedStateRevision: requiredStateRevision(accelerated)
            )
        )
        #expect(outputRecordCount(completed) == UInt64(nextPlanCueCount(loaded)))
        #expect(outputResolutionsAreComplete(completed))
        await supervisor.stop()
    } catch {
        await supervisor.stop()
        throw error
    }
}

private func stateRevision(_ envelope: MessageEnvelope) -> UInt64? {
    guard case let .number(revision) = envelope.payload["stateRevision"] else {
        return nil
    }
    return UInt64(revision)
}

private func nextPlan(_ envelope: MessageEnvelope) -> [String: JSONValue]? {
    guard case let .object(plan) = envelope.payload["nextPlan"] else { return nil }
    return plan
}

private func nextPlanCueCount(_ envelope: MessageEnvelope) -> Int {
    guard let plan = nextPlan(envelope), case let .array(cues) = plan["cues"] else { return 0 }
    return cues.count
}

private func nextPlanLibraryTimelineRevision(_ envelope: MessageEnvelope) -> UInt64? {
    guard let plan = nextPlan(envelope),
          case let .object(identity) = plan["libraryTrack"],
          case let .number(revision) = identity["timelineRevision"] else { return nil }
    return UInt64(revision)
}

private func nextPlanLibraryProvider(_ envelope: MessageEnvelope) -> String? {
    guard let plan = nextPlan(envelope),
          case let .object(identity) = plan["libraryTrack"],
          case let .string(provider) = identity["providerKind"] else { return nil }
    return provider
}

private func nextPlanResolutionCount(_ envelope: MessageEnvelope) -> Int {
    guard let plan = nextPlan(envelope), case let .array(cues) = plan["cues"] else { return 0 }
    return cues.filter { value in
        guard case let .object(cue) = value,
              case .object = cue["libraryResolution"] else { return false }
        return true
    }.count
}

private func nextPlanResolutionFieldsAreComplete(_ envelope: MessageEnvelope) -> Bool {
    guard let plan = nextPlan(envelope), case let .array(cues) = plan["cues"] else { return false }
    return !cues.isEmpty && cues.allSatisfy { value in
        guard case let .object(cue) = value,
              case let .object(resolution) = cue["libraryResolution"],
              case .string = resolution["roleId"],
              case .string = resolution["strategy"],
              case .string = resolution["variantId"],
              case let .object(entry) = resolution["dryRunEntry"],
              case .string = entry["id"] else { return false }
        return true
    }
}

private func outputResolutionsAreComplete(_ envelope: MessageEnvelope) -> Bool {
    guard case let .array(effects) = envelope.payload["outputEffects"] else { return false }
    return !effects.isEmpty && effects.allSatisfy { value in
        guard case let .object(effect) = value,
              effect["status"] == .string("simulated"),
              case let .object(resolution) = effect["libraryResolution"],
              case let .object(entry) = resolution["dryRunEntry"],
              case .string = entry["id"] else { return false }
        return true
    }
}

private func libraryTrackTitles(_ envelope: MessageEnvelope) -> [String] {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(page) = library["page"],
          case let .array(tracks) = page["tracks"] else {
        return []
    }
    return tracks.compactMap { value in
        guard case let .object(track) = value,
              case let .string(title) = track["title"] else { return nil }
        return title
    }
}

private func librarySourceName(_ envelope: MessageEnvelope) -> String? {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(source) = library["source"],
          case let .string(name) = source["name"] else { return nil }
    return name
}

private func phraseRoleRevision(_ envelope: MessageEnvelope) -> UInt64? {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(settings) = library["phraseRoleSettings"],
          case let .number(revision) = settings["revision"] else { return nil }
    return UInt64(revision)
}

private func phraseRoleName(_ envelope: MessageEnvelope, roleID: String) -> String? {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(settings) = library["phraseRoleSettings"],
          case let .array(roles) = settings["roles"] else { return nil }
    for roleValue in roles {
        guard case let .object(role) = roleValue,
              role["id"] == .string(roleID),
              case let .string(name) = role["name"] else { continue }
        return name
    }
    return nil
}

private func autoloopCatalog(_ envelope: MessageEnvelope) -> [String: JSONValue]? {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(catalog) = library["autoloopCatalog"] else { return nil }
    return catalog
}

private func autoloopCatalogRevision(_ envelope: MessageEnvelope) -> UInt64? {
    guard let catalog = autoloopCatalog(envelope),
          case let .number(revision) = catalog["revision"] else { return nil }
    return UInt64(revision)
}

private func autoloopThemeNames(_ envelope: MessageEnvelope) -> [String] {
    guard let catalog = autoloopCatalog(envelope),
          case let .array(themes) = catalog["themes"] else { return [] }
    return themes.compactMap { value in
        guard case let .object(theme) = value,
              case let .string(name) = theme["name"] else { return nil }
        return name
    }
}

private func autoloopVariantCount(_ envelope: MessageEnvelope, roleID: String) -> Int? {
    guard let catalog = autoloopCatalog(envelope),
          case let .array(roles) = catalog["roles"] else { return nil }
    for value in roles {
        guard case let .object(role) = value,
              role["id"] == .string(roleID),
              case let .array(variants) = role["variants"] else { continue }
        return variants.count
    }
    return nil
}

private func autoloopMissingCellCount(_ envelope: MessageEnvelope) -> UInt64? {
    guard let catalog = autoloopCatalog(envelope),
          case let .object(preflight) = catalog["preflight"],
          case let .number(count) = preflight["missingCellCount"] else { return nil }
    return UInt64(count)
}

private func autoloopButtonName(
    _ envelope: MessageEnvelope,
    themeID: UInt64,
    buttonNumber: UInt16
) -> String? {
    autoloopButton(envelope, themeID: themeID, buttonNumber: buttonNumber)?.name
}

private func autoloopButtonRole(
    _ envelope: MessageEnvelope,
    themeID: UInt64,
    buttonNumber: UInt16
) -> String? {
    autoloopButton(envelope, themeID: themeID, buttonNumber: buttonNumber)?.roleID
}

private func autoloopButton(
    _ envelope: MessageEnvelope,
    themeID: UInt64,
    buttonNumber: UInt16
) -> (roleID: String, name: String)? {
    guard let catalog = autoloopCatalog(envelope),
          case let .array(roles) = catalog["roles"] else { return nil }
    for roleValue in roles {
        guard case let .object(role) = roleValue,
              case let .string(roleID) = role["id"],
              case let .array(variants) = role["variants"] else { continue }
        for variantValue in variants {
            guard case let .object(variant) = variantValue,
                  case let .array(cells) = variant["cells"] else { continue }
            for cellValue in cells {
                guard case let .object(cell) = cellValue,
                      cell["themeId"] == .number(Double(themeID)),
                      cell["buttonNumber"] == .number(Double(buttonNumber)),
                      case let .string(name) = cell["name"] else { continue }
                return (roleID, name)
            }
        }
    }
    return nil
}

private func requiredFirstLibraryTrackID(_ envelope: MessageEnvelope) -> UInt64 {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(page) = library["page"],
          case let .array(tracks) = page["tracks"],
          case let .object(track)? = tracks.first,
          case let .number(id) = track["id"] else { return 0 }
    return UInt64(id)
}

private func libraryEditor(_ envelope: MessageEnvelope) -> [String: JSONValue]? {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(editor) = library["editor"] else { return nil }
    return editor
}

private func libraryEditorReconciliation(
    _ envelope: MessageEnvelope
) -> [String: JSONValue]? {
    guard let editor = libraryEditor(envelope),
          case let .object(reconciliation) = editor["sourceReconciliation"] else { return nil }
    return reconciliation
}

private func librarySourceRefreshChangeCount(_ envelope: MessageEnvelope) -> UInt64? {
    guard case let .object(library) = envelope.payload["library"],
          case let .object(refresh) = library["sourceRefresh"],
          case let .number(count) = refresh["changeCount"] else { return nil }
    return UInt64(count)
}

private func libraryEditorTitle(_ envelope: MessageEnvelope) -> String? {
    guard let editor = libraryEditor(envelope),
          case let .object(track) = editor["track"],
          case let .string(title) = track["title"] else { return nil }
    return title
}

private func libraryEditorAudioURI(_ envelope: MessageEnvelope) -> String? {
    guard let editor = libraryEditor(envelope),
          case let .string(uri) = editor["audioUri"] else { return nil }
    return uri
}

private func libraryEditorArrayCount(_ envelope: MessageEnvelope, field: String) -> Int? {
    guard let editor = libraryEditor(envelope),
          case let .array(values) = editor[field] else { return nil }
    return values.count
}

private func libraryEditorFirstRoleID(_ envelope: MessageEnvelope) -> String? {
    guard let editor = libraryEditor(envelope),
          case let .array(phrases) = editor["phrases"],
          case let .object(first)? = phrases.first,
          case let .string(roleID) = first["roleId"] else { return nil }
    return roleID
}

private func libraryEditorFirstLoopStrategy(_ envelope: MessageEnvelope) -> [String: JSONValue]? {
    guard let editor = libraryEditor(envelope),
          case let .array(phrases) = editor["phrases"],
          case let .object(first)? = phrases.first,
          case let .object(strategy) = first["loopStrategy"] else { return nil }
    return strategy
}

private func libraryEditorFirstLoopStrategyKind(_ envelope: MessageEnvelope) -> String? {
    guard let strategy = libraryEditorFirstLoopStrategy(envelope),
          case let .string(kind) = strategy["kind"] else { return nil }
    return kind
}

private func libraryEditorFirstFixedVariantID(_ envelope: MessageEnvelope) -> String? {
    guard let strategy = libraryEditorFirstLoopStrategy(envelope),
          case let .string(variantID) = strategy["fixedVariantId"] else { return nil }
    return variantID
}

private func libraryEditorBeatCount(_ envelope: MessageEnvelope) -> Int? {
    guard let editor = libraryEditor(envelope),
          case let .object(grid) = editor["beatGrid"],
          case let .array(markers) = grid["markers"] else { return nil }
    return markers.count
}

private func libraryEditorIsClosed(_ envelope: MessageEnvelope) -> Bool {
    guard case let .object(library) = envelope.payload["library"] else { return false }
    return library["editor"] == .null
}

private func libraryEditorTimelineRevision(_ envelope: MessageEnvelope) -> UInt64? {
    guard let editor = libraryEditor(envelope),
          case let .object(timeline) = editor["timeline"],
          case let .number(revision) = timeline["revision"] else { return nil }
    return UInt64(revision)
}

private func libraryEditorCanRedo(_ envelope: MessageEnvelope) -> Bool? {
    guard let editor = libraryEditor(envelope),
          case let .object(timeline) = editor["timeline"],
          case let .boolean(canRedo) = timeline["canRedo"] else { return nil }
    return canRedo
}

private func requiredStateRevision(_ envelope: MessageEnvelope) -> UInt64 {
    stateRevision(envelope) ?? 0
}

private func operationState(_ envelope: MessageEnvelope) -> String? {
    guard case let .string(state) = envelope.payload["operationState"] else {
        return nil
    }
    return state
}

private func simulationSpeed(_ envelope: MessageEnvelope) -> UInt64? {
    guard case let .object(simulation) = envelope.payload["simulation"],
          case let .number(speed) = simulation["speed"] else {
        return nil
    }
    return UInt64(speed)
}

private func outputRecordCount(_ envelope: MessageEnvelope) -> UInt64? {
    guard case let .object(output) = envelope.payload["outputProvider"],
          case let .number(count) = output["recordCount"] else {
        return nil
    }
    return UInt64(count)
}

private func timelineContainsSimulatedOutput(_ envelope: MessageEnvelope) -> Bool {
    guard case let .array(timeline) = envelope.payload["timeline"] else {
        return false
    }
    return timeline.contains { value in
        guard case let .object(entry) = value else { return false }
        return entry["source"] == .string("output")
            && entry["result"] == .string("simulated")
    }
}

private func planRevision(_ envelope: MessageEnvelope) -> UInt64? {
    guard case let .object(plan) = envelope.payload["nextPlan"],
          case let .number(revision) = plan["revision"] else {
        return nil
    }
    return UInt64(revision)
}

private func planThemeDecision(_ envelope: MessageEnvelope) -> [String: JSONValue]? {
    guard case let .object(plan) = envelope.payload["nextPlan"],
          case let .object(decision) = plan["themeDecision"] else {
        return nil
    }
    return decision
}

private func planThemeName(_ envelope: MessageEnvelope) -> String? {
    guard let decision = planThemeDecision(envelope),
          case let .string(name) = decision["themeName"] else { return nil }
    return name
}

private func planThemeReason(_ envelope: MessageEnvelope) -> String? {
    guard let decision = planThemeDecision(envelope),
          case let .string(reason) = decision["reason"] else { return nil }
    return reason
}

private func planMatchedColorRGB(_ envelope: MessageEnvelope) -> UInt64? {
    guard let decision = planThemeDecision(envelope),
          case let .number(color) = decision["matchedColorRgb"] else { return nil }
    return UInt64(color)
}

private func planCueThemeIDs(_ envelope: MessageEnvelope) -> Set<UInt64> {
    guard case let .object(plan) = envelope.payload["nextPlan"],
          case let .array(cues) = plan["cues"] else { return [] }
    return Set(cues.compactMap { cueValue in
        guard case let .object(cue) = cueValue,
              case let .object(action) = cue["action"],
              case let .number(themeID) = action["themeId"] else { return nil }
        return UInt64(themeID)
    })
}

@Test("A missing engine executable fails safely")
func rejectsMissingExecutable() async {
    let supervisor = EngineProcessSupervisor()
    let missing = URL(fileURLWithPath: "/private/tmp/lumi-engine-does-not-exist")

    await #expect(throws: EngineClientError.executableMissing) {
        try await supervisor.launch(engineExecutable: missing)
    }
}

@Test("A protocol mismatch fails before transport connection")
func rejectsProtocolMismatch() async throws {
    let fileManager = FileManager.default
    let temporaryDirectory = fileManager.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    try fileManager.createDirectory(
        at: temporaryDirectory,
        withIntermediateDirectories: true
    )
    defer {
        try? fileManager.removeItem(at: temporaryDirectory)
    }

    let fakeEngine = temporaryDirectory.appendingPathComponent("fake-lumi-engine")
    let script = """
        #!/bin/sh
        printf '%s\\n' '{"recordType":"engineReady","host":"127.0.0.1","port":54321,"protocolVersion":99}'
        exec sleep 10
        """
    try Data(script.utf8).write(to: fakeEngine)
    try fileManager.setAttributes(
        [.posixPermissions: 0o755],
        ofItemAtPath: fakeEngine.path
    )

    let supervisor = EngineProcessSupervisor()
    await #expect(
        throws: EngineClientError.protocolMismatch(
            expected: WireProtocol.version,
            received: 99
        )
    ) {
        try await supervisor.launch(engineExecutable: fakeEngine)
    }
}
