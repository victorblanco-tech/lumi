import Foundation
@testable import LumiLiveWorkspace
import LumiProtocol
import Testing

@Suite("Live workspace presentation")
struct LiveWorkspacePresenterTests {
    @Test("Mounted USB inspection snapshot preserves the live workspace")
    func mountedUSBInspectionSnapshotDecodesWhenProvided() throws {
        guard let envelopePath = ProcessInfo.processInfo.environment[
            "LUMI_TEST_USB_ENVELOPE"
        ] else {
            return
        }
        let data = try Data(contentsOf: URL(fileURLWithPath: envelopePath))
        let envelope = try JSONDecoder().decode(MessageEnvelope.self, from: data)
        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:1",
            protocolVersion: 1
        )
        #expect(snapshot.snapshotSequence > 0)
    }

    @Test("Operation status drives one shared Master and control presentation")
    func operationStatusPresentationIsConsistent() {
        #expect(LiveOperationStatus(engineState: "off") == .off)
        #expect(LiveOperationStatus(engineState: "armed") == .armed)
        #expect(LiveOperationStatus(engineState: "live") == .live)
        #expect(LiveOperationStatus(engineState: "paused") == .paused)
        #expect(LiveOperationStatus(engineState: "unknown") == .off)

        #expect(!LiveOperationStatus.off.showsLiveNow(isPlaying: true))
        #expect(!LiveOperationStatus.armed.showsLiveNow(isPlaying: true))
        #expect(!LiveOperationStatus.live.showsLiveNow(isPlaying: false))
        #expect(LiveOperationStatus.live.showsLiveNow(isPlaying: true))
        #expect(!LiveOperationStatus.paused.showsLiveNow(isPlaying: true))
    }

    @Test("Only Paused requests pulsing emphasis")
    func pausedStatusPulses() {
        #expect(!LiveOperationStatus.off.pulses)
        #expect(!LiveOperationStatus.armed.pulses)
        #expect(!LiveOperationStatus.live.pulses)
        #expect(LiveOperationStatus.paused.pulses)
    }

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
        #expect(state.lightingMidi.condition == .ready)
        #expect(state.playbackClock.condition == .ready)
        #expect(snapshot.abletonLinkIntegration?.provider == "Carabiner")
        #expect(snapshot.abletonLinkIntegration?.source == "localPlayback")
        #expect(snapshot.abletonLinkIntegration?.lastBeatAgeMillis == 4)
        #expect(snapshot.abletonLinkIntegration?.lastReanchor == "started")
        #expect(snapshot.midiIntegration?.autoPublishEnabled == true)
        #expect(snapshot.midiIntegration?.timingOffsetMillis == 0)
        #expect(state.planner.condition == .ready)
    }

    @Test("Live timing distinguishes the applied value from the next-phrase value")
    func pendingTimingIsPresentedWithoutReplacingAppliedTiming() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .object(midi) = payload["midiIntegration"] else {
            Issue.record("Recorded fixture has no lighting MIDI status")
            return
        }
        midi["timingOffsetMillis"] = .number(0)
        midi["pendingTimingOffsetMillis"] = .number(20)
        payload["midiIntegration"] = .object(midi)
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )
        let state = LiveWorkspacePresenter.ready(snapshot)

        #expect(snapshot.midiIntegration?.timingOffsetMillis == 0)
        #expect(snapshot.midiIntegration?.pendingTimingOffsetMillis == 20)
        #expect(state.content?.lightingTimingOffsetMillis == 0)
        #expect(state.content?.pendingLightingTimingOffsetMillis == 20)
        #expect(state.lightingMidi.detail.contains("+0 ms applied"))
        #expect(state.lightingMidi.detail.contains("+20 ms pending for next phrase"))
    }

    @Test("Tech degrades when the lighting MIDI source is unavailable")
    func stoppedLightingMidiIsVisible() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .object(midi) = payload["midiIntegration"] else {
            Issue.record("Recorded fixture has no lighting MIDI status")
            return
        }
        midi["state"] = .string("stopped")
        midi["lastError"] = .string("CoreMIDI source unavailable")
        payload["midiIntegration"] = .object(midi)
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )
        let state = LiveWorkspacePresenter.ready(snapshot)

        #expect(state.condition == .degraded)
        #expect(state.lightingMidi.condition == .degraded)
        #expect(state.lightingMidi.detail.contains("CoreMIDI source unavailable"))
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
            midiIntegration: snapshot.midiIntegration,
            midiClockIntegration: snapshot.midiClockIntegration,
            abletonLinkIntegration: snapshot.abletonLinkIntegration,
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

    @Test("Optimistic local leader presentation swaps Live and Next plans immediately")
    func optimisticLocalLeaderPresentationSwapsPlans() throws {
        let snapshot = LiveWorkspaceFixtures.libraryBackedSnapshot()
        let currentLivePlan = try #require(snapshot.livePlan)
        let currentNextPlan = try #require(snapshot.nextPlan)

        let switched = snapshot.optimisticallySettingLocalPlaybackLeader(2)

        #expect(switched.leaderDeckID == 2)
        #expect(switched.livePlan?.deckID == 2)
        #expect(switched.nextPlan?.deckID == 1)
        #expect(switched.livePlan?.planID == currentNextPlan.planID)
        #expect(switched.nextPlan?.planID == currentLivePlan.planID)
        #expect(switched.stateRevision == snapshot.stateRevision)
    }

    @Test("Live notices centralize deck, plan, and local playback feedback")
    func liveNoticeCentralizesFeedback() {
        let localNotice = LiveWorkspaceNoticePresenter.notice(
            state: LiveWorkspaceFixtures.ready,
            localPlaybackFeedback: "Track loaded on Deck B.",
            localPlaybackFeedbackIsError: false
        )
        let commandNotice = LiveWorkspaceNoticePresenter.notice(
            state: LiveWorkspacePresenter.ready(
                LiveWorkspaceFixtures.readySnapshot,
                sessionInteraction: .submitting
            ),
            localPlaybackFeedback: "Track loaded on Deck B.",
            localPlaybackFeedbackIsError: false
        )
        let rejectedNotice = LiveWorkspaceNoticePresenter.notice(
            state: LiveWorkspacePresenter.ready(
                LiveWorkspaceFixtures.readySnapshot,
                planInteraction: .rejected("AutoLoop could not be saved."),
                sessionInteraction: .succeeded("Deck B is Live.")
            ),
            localPlaybackFeedback: nil,
            localPlaybackFeedbackIsError: false
        )

        #expect(localNotice == .init(message: "Track loaded on Deck B.", tone: .success))
        #expect(commandNotice == .init(message: "Applying deck command…", tone: .working))
        #expect(rejectedNotice == .init(message: "AutoLoop could not be saved.", tone: .warning))
    }

    @Test("Library playback fixture exposes provider-neutral RGB waveform data")
    func localLibraryWaveformIsExplicit() {
        let previews = LiveWorkspaceFixtures.readySnapshot.decks.compactMap(\.waveformPreview)

        #expect(previews.count == 2)
        #expect(previews.allSatisfy { $0.source == "library" && $0.style == "rgb" })
        #expect(previews.allSatisfy { $0.points.count == 192 })
    }

    @Test("Local visual clock advances independently and clamps at track end")
    func localVisualClockAdvancesSmoothly() {
        let playing = DeckVisualClockSnapshot(
            trackLoadID: 7,
            positionMillis: 1_000,
            durationMillis: 4_000,
            playing: true,
            anchoredAtReferenceTime: 100
        )
        let paused = DeckVisualClockSnapshot(
            trackLoadID: 7,
            positionMillis: 1_000,
            durationMillis: 4_000,
            playing: false,
            anchoredAtReferenceTime: 100
        )
        let pitched = DeckVisualClockSnapshot(
            trackLoadID: 7,
            positionMillis: 1_000,
            durationMillis: 4_000,
            playing: true,
            anchoredAtReferenceTime: 100,
            playbackRate: 1.1
        )

        #expect(playing.positionMillis(at: Date(timeIntervalSinceReferenceDate: 101.25)) == 2_250)
        #expect(playing.positionMillis(at: Date(timeIntervalSinceReferenceDate: 110)) == 4_000)
        #expect(paused.positionMillis(at: Date(timeIntervalSinceReferenceDate: 110)) == 1_000)
        #expect(pitched.positionMillis(at: Date(timeIntervalSinceReferenceDate: 101)) == 2_100)
    }

    @Test("Live waveform motion keeps a constant fixed playhead while the waveform scrolls")
    func liveWaveformMotionKeepsFixedPlayhead() {
        let motion = LiveWaveformMotionPlan(
            waveformID: 7,
            totalBeats: 800,
            viewportStartBeat: 0,
            visibleBeats: 160,
            followsLiveViewport: true,
            fallbackPlayheadBeat: 320,
            visualClock: nil
        )
        let firstBeat = 320.0
        let nextBeat = 321.0
        let firstFraction = (firstBeat - motion.startBeat(for: firstBeat)) / motion.visibleBeats
        let nextFraction = (nextBeat - motion.startBeat(for: nextBeat)) / motion.visibleBeats

        #expect(abs(firstFraction - LiveDeckViewportPolicy.playheadFraction) < 0.000_1)
        #expect(abs(nextFraction - firstFraction) < 0.000_1)
    }

    @Test("Live beat coordinates preserve exact Rekordbox marker times and grid offset")
    func liveBeatGridUsesExactRekordboxTimes() throws {
        let grid = DeckBeatGridSnapshot(
            beatsPerBar: 4,
            durationMillis: 1_800,
            timesMillis: [60, 447, 901, 1_300]
        )
        let timeline = try #require(LiveBeatGridTimeline(grid: grid, totalBeats: 4))

        #expect(timeline.timeMillis(atBeat: 0) == 60)
        #expect(abs(timeline.timeMillis(atBeat: 1.5) - 674) < 0.000_1)
        #expect(abs(timeline.beat(atTimeMillis: 674) - 1.5) < 0.000_1)
        #expect(abs(timeline.trackProgress(atBeat: 1.5) - 674.0 / 1_800.0) < 0.000_1)
    }

    @Test("Waveform and plan share one exact visual playhead clock")
    func waveformAndPlanShareVisualClock() throws {
        let grid = DeckBeatGridSnapshot(
            beatsPerBar: 4,
            durationMillis: 1_250,
            timesMillis: [60, 447, 901, 1_300]
        )
        let timeline = try #require(LiveBeatGridTimeline(grid: grid, totalBeats: 4))
        let clock = DeckVisualClockSnapshot(
            trackLoadID: 7,
            positionMillis: 447,
            durationMillis: 1_250,
            playing: true,
            anchoredAtReferenceTime: 100
        )
        let date = Date(timeIntervalSinceReferenceDate: 100.3)
        let sharedBeat = LiveDeckVisualTimeline.playheadBeat(
            trackLoadID: 7,
            durationBeats: 4,
            fallbackBeat: 0,
            visualClock: clock,
            beatGrid: timeline,
            at: date
        )
        let waveformMotion = LiveWaveformMotionPlan(
            waveformID: 7,
            totalBeats: 4,
            viewportStartBeat: 0,
            visibleBeats: 4,
            followsLiveViewport: true,
            fallbackPlayheadBeat: 0,
            visualClock: clock,
            beatGrid: timeline
        )

        #expect(abs(sharedBeat - waveformMotion.playheadBeat(at: date)) < 0.000_1)
        #expect(waveformMotion.playbackEndBeat < 3)
        #expect(abs(timeline.timeMillis(atBeat: waveformMotion.playbackEndBeat) - 1_250) < 0.001)
    }

    @Test("Recorded Live decks decode exact Rekordbox beat-grid markers")
    func liveDeckDecodesExactBeatGrid() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .array(decks) = payload["decks"],
              case var .object(deck) = decks[0],
              case var .object(track) = deck["track"] else {
            Issue.record("Recorded deck fixture is malformed")
            return
        }
        track["beatGrid"] = .object([
            "beatsPerBar": .number(4),
            "durationMillis": .number(1_800),
            "timesMillis": .array([.number(60), .number(447), .number(901), .number(1_300)])
        ])
        deck["track"] = .object(track)
        decks[0] = .object(deck)
        payload["decks"] = .array(decks)
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(snapshot.decks[0].beatGrid?.timesMillis == [60, 447, 901, 1_300])
    }

    @Test("Recorded Live decks accept a final Rekordbox marker past the audio duration")
    func liveDeckAcceptsRekordboxTrailingBeatMarker() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .array(decks) = payload["decks"],
              case var .object(deck) = decks[0],
              case var .object(track) = deck["track"] else {
            Issue.record("Recorded deck fixture is malformed")
            return
        }
        track["beatGrid"] = .object([
            "beatsPerBar": .number(4),
            "durationMillis": .number(1_250),
            "timesMillis": .array([.number(60), .number(447), .number(901), .number(1_300)])
        ])
        deck["track"] = .object(track)
        decks[0] = .object(deck)
        payload["decks"] = .array(decks)
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(snapshot.decks[0].beatGrid?.durationMillis == 1_250)
        #expect(snapshot.decks[0].beatGrid?.timesMillis.last == 1_300)
    }

    @Test("Authoritative playback clock prevents poll snapshots from restarting waveform motion")
    func playbackClockKeepsWaveformAnimationIdentityStable() {
        let clock = DeckVisualClockSnapshot(
            trackLoadID: 7,
            positionMillis: 1_000,
            durationMillis: 120_000,
            playing: true,
            anchoredAtReferenceTime: 100
        )
        let first = LiveWaveformMotionPlan(
            waveformID: 7,
            totalBeats: 512,
            viewportStartBeat: 0,
            visibleBeats: 160,
            followsLiveViewport: true,
            fallbackPlayheadBeat: 10,
            visualClock: clock
        )
        let nextPoll = LiveWaveformMotionPlan(
            waveformID: 7,
            totalBeats: 512,
            viewportStartBeat: 0,
            visibleBeats: 160,
            followsLiveViewport: true,
            fallbackPlayheadBeat: 11,
            visualClock: clock
        )

        #expect(first.animationIdentity == nextPoll.animationIdentity)
    }

    @Test("One-time library waveform detail accepts the full high-resolution payload")
    func highResolutionWaveformDetailDecodes() throws {
        let pointCount = 16_384
        let envelope = MessageEnvelope(
            protocolVersion: 1,
            messageType: .snapshot,
            messageId: "waveform-detail",
            sequence: 1,
            correlationId: "test",
            sentAt: "2026-08-07T00:00:00Z",
            payload: [
                "waveformDetail": .object([
                    "trackId": .number(42),
                    "source": .string("localLibraryDetail"),
                    "style": .string("rgb"),
                    "points": .array(Array(
                        repeating: .array([.number(32), .number(137), .number(241)]),
                        count: pointCount
                    ))
                ])
            ]
        )

        let detail = try EngineSnapshotDecoder().decodeWaveformDetail(envelope)

        #expect(detail.trackID == 42)
        #expect(detail.preview.source == "localLibraryDetail")
        #expect(detail.preview.points.count == pointCount)
        #expect(detail.preview.points.last?.high == 241)
    }

    @Test("Live deck viewport defaults to 40 bars and keeps the playhead left")
    func liveDeckViewportDefaultsToFortyBars() {
        let viewport = LiveDeckViewportPolicy.live(
            playheadBeat: 320,
            totalBeats: 1_024
        )

        #expect(viewport.visibleBars == 40)
        #expect(abs(viewport.x(forBeat: 320, width: 1_000) - 220) < 0.001)
    }

    @Test("Next deck viewport remains a full-track overview")
    func nextDeckViewportRemainsOverview() {
        let viewport = LiveDeckViewportPolicy.overview(totalBeats: 752)

        #expect(viewport.startBeat == 0)
        #expect(viewport.visibleBeats == 752)
        #expect(viewport.visibleBars == 188)
    }

    @Test("A user-selected Live zoom keeps the same fixed playhead position")
    func liveDeckViewportPreservesUserZoom() {
        let viewport = LiveDeckViewportPolicy.live(
            playheadBeat: 400,
            totalBeats: 1_024,
            visibleBeats: 96
        )

        #expect(viewport.visibleBeats == 96)
        #expect(abs(viewport.x(forBeat: 400, width: 800) - 176) < 0.001)
    }

    @Test("Live AutoLoop plan exposes active, next, and future status with output details")
    func liveAutoloopStatusIsExplicit() throws {
        let content = try #require(LiveWorkspaceFixtures.ready.content)
        let deck = try #require(content.liveDeck)
        let items = PlannedAutoloopPresenter.items(
            deck: deck,
            plan: content.livePlan,
            isMaster: true
        )

        #expect(items.map(\.status) == [.active, .next, .planned, .planned])
        #expect(items.first?.bankNumber == 1)
        #expect(items.first?.slotNumber == 1)
        #expect(items.first?.autoloopName == "Soft Motion")
    }

    @Test("Visual playhead beat advances AutoLoop status without waiting for a snapshot")
    func visualBeatDrivesAutoloopStatus() throws {
        let content = try #require(LiveWorkspaceFixtures.ready.content)
        let deck = try #require(content.liveDeck)
        let plan = try #require(content.livePlan)
        let thirdCue = plan.cues[2]
        let items = PlannedAutoloopPresenter.items(
            deck: deck,
            plan: plan,
            isMaster: true,
            playheadBeat: Double(thirdCue.startBeat)
        )

        #expect(items.map(\.status) == [.completed, .completed, .active, .next])
    }

    @Test("Next deck AutoLoop plan marks only its first item as next")
    func nextAutoloopStatusIsExplicit() throws {
        let content = try #require(LiveWorkspaceFixtures.libraryBacked.content)
        let deck = try #require(content.nextDeck)
        let items = PlannedAutoloopPresenter.items(
            deck: deck,
            plan: content.plan,
            isMaster: false
        )

        #expect(items.map(\.status) == [.next, .planned, .planned, .planned])
        #expect(items[1].phraseName == "Breakdown 1")
        #expect(items[1].autoloopName.contains("Variant 2"))
    }

    @Test("A transport-neutral snapshot sequence does not invalidate the Live presentation")
    func snapshotSequenceDoesNotForceARelayout() {
        let snapshot = LiveWorkspaceFixtures.readySnapshot
        let nextSequence = EngineSnapshot(
            endpoint: snapshot.endpoint,
            engineVersion: snapshot.engineVersion,
            protocolVersion: snapshot.protocolVersion,
            snapshotSequence: snapshot.snapshotSequence + 1,
            stateRevision: snapshot.stateRevision,
            operationState: snapshot.operationState,
            runtime: snapshot.runtime,
            deckSource: snapshot.deckSource,
            deckInputIntegration: snapshot.deckInputIntegration,
            midiIntegration: snapshot.midiIntegration,
            midiClockIntegration: snapshot.midiClockIntegration,
            abletonLinkIntegration: snapshot.abletonLinkIntegration,
            simulation: snapshot.simulation,
            outputProvider: snapshot.outputProvider,
            leaderDeckID: snapshot.leaderDeckID,
            decks: snapshot.decks,
            livePlan: snapshot.livePlan,
            nextPlan: snapshot.nextPlan,
            planningOptions: snapshot.planningOptions,
            timeline: snapshot.timeline
        )

        #expect(
            LiveWorkspacePresenter.ready(snapshot)
                == LiveWorkspacePresenter.ready(nextSequence)
        )
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
        track["key"] = .object([
            "pitchClass": .string("c"),
            "mode": .string("minor"),
            "known": .boolean(false)
        ])
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
        #expect(snapshot.decks[0].keyKnown == false)
        #expect(snapshot.livePlan == nil)
    }

    @Test("Effective deck BPM overrides immutable track BPM")
    func effectiveDeckBPMIsDecoded() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case var .array(decks) = payload["decks"],
              !decks.isEmpty,
              case var .object(deck) = decks[0],
              case let .object(track) = deck["track"],
              let trackBPM = track["bpmMilli"] else {
            Issue.record("Recorded fixture must contain Deck A BPM")
            return
        }
        deck["effectiveBpmMilli"] = .number(131_300)
        decks[0] = .object(deck)
        payload["decks"] = .array(decks)
        let updated = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            updated,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(trackBPM != .number(131_300))
        #expect(snapshot.decks[0].bpmMilli == 131_300)
    }

    @Test("A loaded connected deck can be planned before a Master is elected")
    func loadedConnectedDeckWithoutMasterDecodesAsNextPlan() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        guard case let .array(decks) = payload["decks"],
              decks.count == 2,
              case let .object(nextPlan) = payload["nextPlan"],
              let nextDeckID = nextPlan["deckId"],
              let nextDeck = decks.first(where: { deck in
                  guard case let .object(value) = deck else { return false }
                  return value["deckId"] == nextDeckID
              }) else {
            Issue.record("Recorded fixture must contain a next deck and plan")
            return
        }
        payload["leaderDeckId"] = .null
        payload["decks"] = .array([nextDeck])
        payload["livePlan"] = .null
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(snapshot.leaderDeckID == nil)
        #expect(snapshot.decks.count == 1)
        #expect(snapshot.nextPlan?.deckID == snapshot.decks[0].deckID)
        #expect(snapshot.nextPlan?.trackLoadID == snapshot.decks[0].trackLoadID)
        #expect(snapshot.livePlan == nil)
    }

    @Test("BLT input diagnostics decode as a separate connected-deck integration")
    func deckInputDiagnosticsDecode() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        payload["deckInputIntegration"] = .object([
            "state": .string("ready"),
            "destinationName": .string("Lumi Deck Input"),
            "protocol": .string("BLT MIDI Deck Frame"),
            "protocolVersion": .number(1),
            "receivedMessageCount": .number(34),
            "invalidWordCount": .number(0),
            "committedFrameCount": .number(2),
            "ignoredMessageCount": .number(1),
            "duplicateFrameCount": .number(0),
            "lastDeckId": .number(2),
            "lastFrameSequence": .number(9)
        ])
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(snapshot.deckInputIntegration?.destinationName == "Lumi Deck Input")
        #expect(snapshot.deckInputIntegration?.committedFrameCount == 2)
        #expect(snapshot.deckInputIntegration?.lastDeckID == 2)
    }

    @Test("Direct Pro DJ Link diagnostics accept the bridge sequence and player range")
    func directProLinkDiagnosticsDecode() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        payload["deckInputIntegration"] = .object([
            "state": .string("ready"),
            "destinationName": .null,
            "protocol": .string("lumi-prolink-bridge"),
            "protocolVersion": .number(1),
            "receivedMessageCount": .number(340),
            "invalidWordCount": .number(0),
            "committedFrameCount": .number(340),
            "ignoredMessageCount": .number(0),
            "duplicateFrameCount": .number(0),
            "lastDeckId": .number(4),
            "lastFrameSequence": .number(340)
        ])
        let envelope = MessageEnvelope(
            protocolVersion: recorded.protocolVersion,
            messageType: recorded.messageType,
            messageId: recorded.messageId,
            sequence: recorded.sequence,
            correlationId: recorded.correlationId,
            sentAt: recorded.sentAt,
            payload: payload
        )

        let snapshot = try EngineSnapshotDecoder().decode(
            envelope,
            endpointDescription: "127.0.0.1:52841",
            protocolVersion: 1
        )

        #expect(snapshot.deckInputIntegration?.protocolName == "lumi-prolink-bridge")
        #expect(snapshot.deckInputIntegration?.lastFrameSequence == 340)
        #expect(snapshot.deckInputIntegration?.lastDeckID == 4)
    }

    @Test("Malformed optional BLT diagnostics fail strict decoding")
    func malformedDeckInputDiagnosticsFailStrictly() throws {
        let recorded = try recordedEnvelope()
        var payload = recorded.payload
        payload["deckInputIntegration"] = .object([
            "state": .string("ready"),
            "destinationName": .number(4),
            "protocol": .string("BLT MIDI Deck Frame"),
            "protocolVersion": .number(1),
            "receivedMessageCount": .number(0),
            "invalidWordCount": .number(0),
            "committedFrameCount": .number(0),
            "ignoredMessageCount": .number(0),
            "duplicateFrameCount": .number(0),
            "lastDeckId": .null,
            "lastFrameSequence": .null
        ])
        let envelope = MessageEnvelope(
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
                envelope,
                endpointDescription: "127.0.0.1:52841",
                protocolVersion: 1
            )
        }
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
