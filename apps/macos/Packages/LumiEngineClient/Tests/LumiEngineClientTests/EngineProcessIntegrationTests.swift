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
            "actualStateRevision": .number(11)
        ]
    )

    let failure = EngineCommandFailure(envelope)
    #expect(failure?.kind == "revisionConflict")
    #expect(failure?.actualPlanRevision == 4)
    #expect(failure?.actualStateRevision == 11)
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
    do {
        let endpoint = try await supervisor.launch(
            engineExecutable: URL(fileURLWithPath: executablePath)
        )
        #expect(endpoint.host == "127.0.0.1")
        #expect(endpoint.protocolVersion == WireProtocol.version)

        let snapshot = try await supervisor.connect(to: endpoint)
        #expect(snapshot.messageType == .snapshot)
        #expect(snapshot.sequence == 1)

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
