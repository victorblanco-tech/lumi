import Foundation
import LumiLiveWorkspace
import LumiProtocol
import Testing

@Suite("Live workspace presentation")
struct LiveWorkspacePresenterTests {
    @Test("Recorded snapshot maps the authoritative leader to Live and the other deck to Next")
    func recordedSnapshotMapsLiveAndNext() throws {
        let envelope = try recordedEnvelope()
        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        let state = LiveWorkspacePresenter.ready(snapshot)

        #expect(state.condition == .ready)
        #expect(state.content?.liveDeck.deckID == 1)
        #expect(state.content?.liveDeck.title == "Aurora Signal")
        #expect(state.content?.nextDeck.deckID == 2)
        #expect(state.content?.nextDeck.title == "Neon Horizon")
        #expect(state.content?.plan?.deckID == state.content?.nextDeck.deckID)
        #expect(state.content?.plan?.cues.count == 4)
        #expect(state.content?.plan?.planID == "14113485664261432828")
        #expect(state.content?.plan?.cues.allSatisfy { !$0.locked } == true)
        #expect(state.content?.planningOptions.themes.count == 2)
        #expect(state.content?.planningOptions.scenes.count == 10)
        #expect(state.content?.operationState == "armed")
        #expect(state.content?.simulation.speed == 1)
        #expect(state.content?.timeline.count == 1)
        #expect(state.output.condition == .ready)
        #expect(state.planner.condition == .ready)
    }

    @Test("Plan interaction feedback retains the authoritative snapshot")
    func planInteractionRetainsSnapshot() {
        let state = LiveWorkspacePresenter.ready(
            LiveWorkspaceFixtures.readySnapshot,
            planInteraction: .rejected("Revision conflict")
        )

        #expect(state.planInteraction == .rejected("Revision conflict"))
        #expect(state.content?.plan?.revision == 1)
    }

    @Test("Fallback remains visible with authoritative decks and plan")
    func fallbackIsExplicit() {
        let state = LiveWorkspaceFixtures.fallback

        #expect(state.condition == .fallback)
        #expect(state.content != nil)
        #expect(state.content?.plan?.status == "fallback")
        #expect(state.diagnostic != nil)
    }

    @Test("Stale data retains the last complete snapshot")
    func staleRetainsSnapshot() {
        let state = LiveWorkspaceFixtures.stale

        #expect(state.condition == .stale)
        #expect(state.content?.nextDeck.title == "Neon Horizon")
        #expect(state.engine.condition == .stale)
    }

    @Test("Disconnected never presents fabricated deck data")
    func disconnectedHasNoContent() {
        let state = LiveWorkspacePresenter.disconnected()

        #expect(state.condition == .disconnected)
        #expect(state.content == nil)
        #expect(state.engine.condition == .error)
    }

    @Test("Decoder rejects a plan that targets the live deck")
    func decoderRejectsMismatchedPlan() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .object(plan) = payload["nextPlan"] else {
            Issue.record("Recorded fixture has no next plan")
            return
        }
        plan["deckId"] = .number(1)
        payload["nextPlan"] = .object(plan)
        let invalid = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        #expect(throws: EngineSnapshotDecodingError.invalidSnapshot) {
            try EngineSnapshotDecoder().decode(
                invalid,
                endpointDescription: "127.0.0.1:52841",
                protocolVersion: 1
            )
        }
    }

    private func recordedEnvelope() throws -> MessageEnvelope {
        var repositoryRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<7 {
            repositoryRoot.deleteLastPathComponent()
        }
        let fixture = repositoryRoot
            .appendingPathComponent("contracts/protocol/v1/fixtures/snapshot-state.json")
        let data = try Data(contentsOf: fixture)
        return try JSONDecoder().decode(MessageEnvelope.self, from: data)
    }
}
