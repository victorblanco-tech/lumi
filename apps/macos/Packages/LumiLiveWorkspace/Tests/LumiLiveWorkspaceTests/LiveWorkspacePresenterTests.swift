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
        #expect(state.content?.liveDeck?.deckID == 1)
        #expect(state.content?.liveDeck?.title == "Aurora Signal")
        #expect(state.content?.nextDeck?.deckID == 2)
        #expect(state.content?.nextDeck?.title == "Neon Horizon")
        #expect(state.content?.decks.map(\.deckID) == [1, 2])
        #expect(state.content?.leaderDeckID == 1)
        #expect(state.content?.liveDeck?.durationBeats == 128)
        #expect(state.content?.liveDeck?.phrases.map(\.roleID) == ["intro-outro", "bridge", "buildup-1", "drop"])
        #expect(state.content?.plan?.deckID == state.content?.nextDeck?.deckID)
        #expect(state.content?.plan?.cues.count == 4)
        #expect(state.content?.plan?.planID == "14113485664261432828")
        #expect(state.content?.plan?.cues.allSatisfy { !$0.locked } == true)
        #expect(state.content?.planningOptions.themes.count == 4)
        #expect(state.content?.nextDeck?.colorRGB == 4_747_469)
        #expect(state.content?.plan?.themeDecision?.themeName == "Deep Ocean")
        #expect(state.content?.plan?.themeDecision?.reason == "colorPrefer")
        #expect(state.content?.plan?.themeDecision?.matchedColorRGB == 4_747_469)
        #expect(state.content?.planningOptions.scenes.count == 10)
        #expect(state.content?.operationState == "armed")
        #expect(state.content?.simulation == nil)
        #expect(state.content?.timeline.count == 1)
        #expect(state.output.condition == .ready)
        #expect(state.planner.condition == .ready)
    }

    @Test("Physical Deck A and B ordering remains stable when Deck B becomes master")
    func stableDeckOrderingSurvivesMasterChange() {
        let snapshot = LiveWorkspaceFixtures.readySnapshot
        let deckBMaster = EngineSnapshot(
            endpoint: snapshot.endpoint,
            engineVersion: snapshot.engineVersion,
            protocolVersion: snapshot.protocolVersion,
            snapshotSequence: snapshot.snapshotSequence + 1,
            stateRevision: snapshot.stateRevision + 1,
            operationState: snapshot.operationState,
            runtime: snapshot.runtime,
            deckSource: snapshot.deckSource,
            simulation: snapshot.simulation,
            outputProvider: snapshot.outputProvider,
            leaderDeckID: 2,
            decks: Array(snapshot.decks.reversed()),
            nextPlan: nil,
            planningOptions: snapshot.planningOptions,
            timeline: snapshot.timeline
        )

        let state = LiveWorkspacePresenter.ready(deckBMaster)

        #expect(state.content?.decks.map(\.deckID) == [1, 2])
        #expect(state.content?.leaderDeckID == 2)
        #expect(state.content?.liveDeck?.deckID == 2)
        #expect(state.content?.nextDeck?.deckID == 1)
    }

    @Test("Library playback fixture exposes provider-neutral RGB waveform data")
    func localLibraryWaveformIsExplicit() {
        let previews = LiveWorkspaceFixtures.readySnapshot.decks.compactMap(\.waveformPreview)

        #expect(previews.count == 2)
        #expect(previews.allSatisfy { $0.source == "library" && $0.style == "rgb" })
        #expect(previews.allSatisfy { $0.points.count == 192 })
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

    @Test("Library-backed Next retains source, exact Lumi revision, and logical Autoloop evidence")
    func libraryBackedNextIsVisible() {
        let plan = LiveWorkspaceFixtures.libraryBacked.content?.plan

        #expect(plan?.libraryTrack?.providerKind == "demo")
        #expect(plan?.libraryTrack?.timelineRevision == 2)
        #expect(plan?.cues.count == 4)
        #expect(plan?.cues[1].libraryResolution?.roleID == "breakdown-1")
        #expect(plan?.cues[1].libraryResolution?.strategy == "fixedVariant")
        #expect(plan?.cues[1].libraryResolution?.variantID == "variant-2")
        #expect(plan?.cues[1].libraryResolution?.entryID.contains("theme-2--") == true)
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
        #expect(state.content?.nextDeck?.title == "Neon Horizon")
        #expect(state.engine.condition == .stale)
    }

    @Test("Disconnected never presents fabricated deck data")
    func disconnectedHasNoContent() {
        let state = LiveWorkspacePresenter.disconnected()

        #expect(state.condition == .disconnected)
        #expect(state.content == nil)
        #expect(state.engine.condition == .error)
    }

    @Test("An unmatched connected track is shown safely with AUTO HELD")
    func unmatchedTrackFailsClosedWithoutFabricatedPhrases() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .array(decks) = payload["decks"],
              !decks.isEmpty,
              case var .object(deck) = decks[0],
              case var .object(track) = deck["track"] else {
            Issue.record("Recorded fixture must contain Deck A")
            return
        }
        track["phrases"] = .array([])
        deck["track"] = .object(track)
        deck["phraseIndex"] = .null
        deck["planEligibility"] = .string("autoHeld")
        decks[0] = .object(deck)
        payload["decks"] = .array(decks)
        payload["livePlan"] = .null
        payload["nextPlan"] = .null
        let unmatched = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            unmatched,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(snapshot.decks[0].planEligibility == .autoHeld)
        #expect(snapshot.decks[0].phrases.isEmpty)
        #expect(snapshot.livePlan == nil)
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

    @Test("Decoder rejects a Theme decision that does not match the plan's starting Theme")
    func decoderRejectsInconsistentThemeDecision() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .object(plan) = payload["nextPlan"],
              case var .object(decision) = plan["themeDecision"] else {
            Issue.record("Recorded fixture has no Theme decision")
            return
        }
        decision["themeId"] = .number(4)
        decision["themeName"] = .string("Ultraviolet")
        plan["themeDecision"] = .object(decision)
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

    @Test("Decoder accepts a user-selected Theme from a future phrase")
    func decoderAcceptsFuturePhraseThemeOverride() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .object(plan) = payload["nextPlan"],
              case var .array(cues) = plan["cues"],
              cues.count > 1,
              case var .object(futureCue) = cues[1],
              case var .object(action) = futureCue["action"] else {
            Issue.record("Recorded fixture has no future editable cue")
            return
        }
        action["themeId"] = .number(4)
        action["themeName"] = .string("Ultraviolet")
        futureCue["action"] = .object(action)
        cues[1] = .object(futureCue)
        plan["cues"] = .array(cues)
        payload["nextPlan"] = .object(plan)
        let revised = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            revised,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )
        #expect(snapshot.nextPlan?.cues[1].action == .applyLook(
            themeID: 4,
            themeName: "Ultraviolet",
            sceneID: 10,
            sceneName: "Slow Wave",
            category: "break",
            loopBank: 5,
            loopSlot: 2
        ))
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
