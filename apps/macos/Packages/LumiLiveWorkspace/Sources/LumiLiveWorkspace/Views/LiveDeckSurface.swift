import AppKit
import LumiDesignSystem
import QuartzCore
import SwiftUI

struct LiveDeckSurface<Details: View>: View {
    let deck: DeckSnapshot
    let isMaster: Bool
    let operationState: String
    let plan: PlanSnapshot?
    let musicalKey: String
    let isLocalPlayback: Bool
    let visualClock: DeckVisualClockSnapshot?
    let waveformOverride: DeckWaveformPreviewSnapshot?
    let lightingTimingOffsetMillis: Int
    let pendingLightingTimingOffsetMillis: Int?
    let selectedPhraseIndex: UInt64?
    let onSelectPhrase: (UInt64) -> Void
    let onTogglePlayback: () -> Void
    let onStop: () -> Void
    let onSeek: (Double) -> Void
    let onMakeMaster: () -> Void
    private let details: Details
    @State private var viewport: LumiWaveformViewport
    @State private var usesLiveViewport: Bool
    @State private var magnificationAnchorBeats: Double?
    @State private var scrubProgress: Double?
    @State private var masterEmphasis = 1.0
    @AppStorage(LumiPreferenceKey.waveformZoomAnchor)
    private var waveformZoomAnchorRaw = LumiWaveformZoomAnchor.mouse.rawValue
    @AppStorage(LumiPreferenceKey.waveformReverseHorizontalScroll)
    private var reversesHorizontalScroll = false

    init(
        deck: DeckSnapshot,
        isMaster: Bool,
        operationState: String,
        plan: PlanSnapshot?,
        musicalKey: String,
        isLocalPlayback: Bool,
        visualClock: DeckVisualClockSnapshot? = nil,
        waveformOverride: DeckWaveformPreviewSnapshot? = nil,
        lightingTimingOffsetMillis: Int = 0,
        pendingLightingTimingOffsetMillis: Int? = nil,
        selectedPhraseIndex: UInt64?,
        onSelectPhrase: @escaping (UInt64) -> Void,
        onTogglePlayback: @escaping () -> Void = {},
        onStop: @escaping () -> Void = {},
        onSeek: @escaping (Double) -> Void = { _ in },
        onMakeMaster: @escaping () -> Void = {},
        @ViewBuilder details: () -> Details
    ) {
        self.deck = deck
        self.isMaster = isMaster
        self.operationState = operationState
        self.plan = plan
        self.musicalKey = musicalKey
        self.isLocalPlayback = isLocalPlayback
        self.visualClock = visualClock
        self.waveformOverride = waveformOverride
        self.lightingTimingOffsetMillis = lightingTimingOffsetMillis
        self.pendingLightingTimingOffsetMillis = pendingLightingTimingOffsetMillis
        self.selectedPhraseIndex = selectedPhraseIndex
        self.onSelectPhrase = onSelectPhrase
        self.onTogglePlayback = onTogglePlayback
        self.onStop = onStop
        self.onSeek = onSeek
        self.onMakeMaster = onMakeMaster
        self.details = details()
        let startsInLiveViewport = isMaster && (visualClock?.playing ?? deck.playing)
        _usesLiveViewport = State(initialValue: startsInLiveViewport)
        _viewport = State(initialValue: startsInLiveViewport
            ? LiveDeckViewportPolicy.live(
                playheadBeat: Double(deck.beat),
                totalBeats: deck.durationBeats
            )
            : LiveDeckViewportPolicy.overview(totalBeats: deck.durationBeats))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            metadata
            if isLocalPlayback { transportControls }
            if waveformPreview?.points.isEmpty == false { waveformToolbar }
            hotCueStrip
            synchronizedDeckTimeline
            details
        }
        .background(Color.black)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            masterBorder
        }
        .shadow(
            color: isMaster ? operationStatus.color.opacity(0.18) : Color.clear,
            radius: 14
        )
        .accessibilityElement(children: .contain)
        .onChange(of: deck.trackLoadID) { _, _ in
            scrubProgress = nil
            resetViewport()
        }
        .onChange(of: isMaster) { wasMaster, isMasterNow in
            guard wasMaster != isMasterNow else { return }
            if isMasterNow {
                activateDefaultLiveViewport()
            } else {
                freezeCurrentLiveViewport()
                usesLiveViewport = false
            }
        }
        .onChange(of: playbackIsActive) { wasPlaying, isPlaying in
            guard !wasPlaying, isPlaying, isMaster, !usesLiveViewport else { return }
            activateDefaultLiveViewport()
        }
        .onChange(of: visualClock?.discontinuityRevision) { previous, current in
            guard LiveDeckViewportPolicy.resumesFollow(
                previousDiscontinuityRevision: previous,
                currentDiscontinuityRevision: current,
                isMaster: isMaster
            ) else { return }
            activateDefaultLiveViewport()
        }
        .task(id: isMaster && operationStatus.pulses) {
            masterEmphasis = 1
            guard isMaster, operationStatus.pulses else { return }
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: .milliseconds(500))
                } catch {
                    return
                }
                withAnimation(.linear(duration: 0.12)) {
                    masterEmphasis = masterEmphasis == 1 ? 0.28 : 1
                }
            }
        }
    }

    private var synchronizedDeckTimeline: some View {
        VStack(alignment: .leading, spacing: 0) {
            animatedWaveformTimeline
            plannedTimelineSnapshot
        }
    }

    private var animatedWaveformTimeline: some View {
        let playheadBeat = displayedPlayheadBeat(at: Date())
        let renderingViewport = renderingViewport(for: playheadBeat)
        return waveform(
            playheadBeat: playheadBeat,
            renderingViewport: renderingViewport
        )
    }

    private var plannedTimelineSnapshot: some View {
        TimelineView(
            .animation(
                minimumInterval: 1.0 / 30.0,
                paused: !playbackIsActive
            )
        ) { timeline in
            let playheadBeat = displayedPlayheadBeat(at: timeline.date)
            let renderingViewport = renderingViewport(for: playheadBeat)
            VStack(alignment: .leading, spacing: 0) {
                phraseBand(
                    playheadBeat: playheadBeat,
                    renderingViewport: renderingViewport
                )
                plannedAutoloops(
                    playheadBeat: playheadBeat,
                    renderingViewport: renderingViewport
                )
            }
        }
    }

    private var header: some View {
        HStack(alignment: .top, spacing: LumiSpacing.medium) {
            Text(verbatim: deckName)
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(LumiColor.accent)
                .padding(.horizontal, LumiSpacing.small)
                .frame(height: LumiControlMetric.compactHeight)
                .background(LumiColor.accent.opacity(0.14))
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))

            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: deck.title)
                    .font(LumiTypography.cardTitle)
                    .foregroundStyle(Color.white)
                    .lineLimit(1)
                Text(verbatim: deck.artist)
                    .font(LumiTypography.metadata)
                    .foregroundStyle(Color.white.opacity(0.62))
                    .lineLimit(1)
            }
            Spacer(minLength: LumiSpacing.small)
            lightingTimingBadge
            roleBadge
        }
        .padding(LumiSpacing.medium)
        .background {
            if isMaster {
                LinearGradient(
                    colors: [operationStatus.color.opacity(0.16), Color.clear],
                    startPoint: .leading,
                    endPoint: .trailing
                )
            }
        }
    }

    private var lightingTimingBadge: some View {
        let applied = String(format: "%+d ms", lightingTimingOffsetMillis)
        let pending = pendingLightingTimingOffsetMillis.map {
            String(format: " → %+d ms", $0)
        } ?? ""
        return HStack(spacing: LumiSpacing.xSmall) {
            Image(systemName: "metronome")
            Text(verbatim: "\(applied)\(pending)")
                .monospacedDigit()
            if pendingLightingTimingOffsetMillis != nil {
                Text(verbatim: "NEXT")
                    .foregroundStyle(LumiColor.warning)
            }
        }
        .font(LumiTypography.technical.weight(.semibold))
        .foregroundStyle(Color.white.opacity(0.62))
        .frame(minWidth: 90, minHeight: LumiControlMetric.compactHeight)
        .accessibilityLabel(
            pendingLightingTimingOffsetMillis.map {
                "Lighting timing \(applied) applied, \(String(format: "%+d ms", $0)) pending for next phrase"
            } ?? "Lighting timing \(applied) applied"
        )
        .accessibilityIdentifier("lumi.deck.lightingTiming")
    }

    @ViewBuilder
    private var roleBadge: some View {
        if isMaster {
            HStack(spacing: LumiSpacing.xSmall) {
                Image(systemName: "circle.fill")
                Text(verbatim: "MASTER")
                if operationStatus.showsLiveNow(isPlaying: playbackIsActive) {
                    Text(verbatim: "·")
                        .foregroundStyle(Color.white.opacity(0.5))
                    Text(verbatim: "LIVE NOW")
                        .foregroundStyle(LumiColor.success)
                }
            }
            .font(LumiTypography.technical.weight(.semibold))
            .foregroundStyle(operationStatus.color)
            .padding(.horizontal, LumiSpacing.small)
            .frame(height: LumiControlMetric.compactHeight)
            .background(operationStatus.color.opacity(0.12))
            .clipShape(Capsule())
            .overlay {
                Capsule()
                    .stroke(
                        operationStatus.color.opacity(masterEmphasis),
                        lineWidth: 1
                    )
            }
            .accessibilityLabel(masterAccessibilityLabel)
            .accessibilityIdentifier("lumi.deck.masterBadge")
        } else if plan?.status == "ready" {
            Text(verbatim: "PLAN READY")
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(LumiColor.success)
                .padding(.horizontal, LumiSpacing.small)
                .frame(height: LumiControlMetric.compactHeight)
                .background(LumiColor.success.opacity(0.12))
                .clipShape(Capsule())
        } else {
            Text(verbatim: "DECK LOADED")
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(Color.white.opacity(0.62))
                .padding(.horizontal, LumiSpacing.small)
                .frame(height: LumiControlMetric.compactHeight)
                .background(Color.white.opacity(0.08))
                .clipShape(Capsule())
        }
    }

    private var masterBorder: some View {
        RoundedRectangle(cornerRadius: LumiRadius.panel)
            .strokeBorder(
                isMaster
                    ? operationStatus.color.opacity(masterEmphasis)
                    : LumiColor.border,
                lineWidth: isMaster ? 2 : 1
            )
    }

    private var operationStatus: LiveOperationStatus {
        LiveOperationStatus(engineState: operationState)
    }

    private var masterAccessibilityLabel: String {
        operationStatus.showsLiveNow(isPlaying: playbackIsActive)
            ? "Master, Live Now"
            : "Master"
    }

    private var metadata: some View {
        HStack(spacing: 0) {
            metadataValue("BPM", value: String(
                format: "%.1f",
                locale: Locale(identifier: "en_US_POSIX"),
                Double(deck.bpmMilli) / 1_000
            ))
            metadataValue("KEY", value: musicalKey)
            metadataValue("BEAT", value: "\(deck.beat)")
            metadataValue("TRANSPORT", value: playbackIsActive ? "PLAYING" : "PAUSED")
            metadataValue("PHRASE", value: activePhraseName(at: Double(deck.beat)))
        }
        .background(Color.white.opacity(0.035))
        .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.12)) }
        .overlay(alignment: .bottom) { Divider().overlay(Color.white.opacity(0.12)) }
    }

    private var transportControls: some View {
        HStack(spacing: LumiSpacing.small) {
            Button(action: onTogglePlayback) {
                Label(
                    playbackIsActive ? "Pause" : "Play",
                    systemImage: playbackIsActive ? "pause.fill" : "play.fill"
                )
            }
            .buttonStyle(.borderedProminent)
            Button(action: onStop) {
                Label("Cue", systemImage: "backward.end.fill")
            }
            .buttonStyle(.bordered)
            if !isMaster {
                Button(action: onMakeMaster) {
                    Label("Make Live", systemImage: "circle.fill")
                }
                .buttonStyle(.bordered)
            }
            Spacer()
            Label("LOCAL AUDIO", systemImage: "speaker.wave.2.fill")
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(LumiColor.accent)
            planEligibilityBadge
        }
        .padding(.horizontal, LumiSpacing.small)
        .padding(.vertical, LumiSpacing.small)
        .background(Color.white.opacity(0.025))
        .accessibilityIdentifier("lumi.deck.\(deck.deckID).transport")
    }

    private var planEligibilityBadge: some View {
        Text(verbatim: planEligibilityLabel)
        .font(LumiTypography.technical.weight(.semibold))
        .foregroundStyle(deck.planEligibility == .autoHeld ? LumiColor.warning : LumiColor.success)
        .padding(.horizontal, LumiSpacing.small)
        .frame(height: LumiControlMetric.compactHeight)
        .background(
            (deck.planEligibility == .autoHeld ? LumiColor.warning : LumiColor.success)
                .opacity(0.12)
        )
        .clipShape(Capsule())
    }

    private var planEligibilityLabel: String {
        switch deck.planEligibility {
        case .readyExact: "LIBRARY MATCH"
        case .readyTransient: "TRANSIENT PLAN"
        case .autoHeld: "AUTO HELD"
        }
    }

    private var waveformToolbar: some View {
        HStack {
            Text("WAVEFORM")
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(Color.white.opacity(0.48))
            Spacer()
            LumiWaveformZoomControls(
                zoom: zoomSliderBinding,
                visibleBars: viewport.visibleBars,
                zoomAnchor: waveformZoomAnchorBinding,
                reversesHorizontalScroll: $reversesHorizontalScroll,
                sliderWidth: 96,
                accessibilityPrefix: "lumi.deck.\(deck.deckID)"
            )
        }
        .padding(.horizontal, LumiSpacing.small)
        .frame(height: 34)
        .foregroundStyle(Color.white)
        .background(Color.white.opacity(0.025))
    }

    @ViewBuilder
    private var hotCueStrip: some View {
        if !deck.hotCues.isEmpty {
            HStack(spacing: LumiSpacing.small) {
                Text(verbatim: "HOT CUES")
                    .font(LumiTypography.caption.weight(.semibold))
                    .foregroundStyle(Color.white.opacity(0.48))
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 5) {
                        ForEach(deck.hotCues) { cue in
                            Button {
                                guard isLocalPlayback,
                                      let duration = beatGridTimeline?.durationMillis,
                                      duration > 0 else { return }
                                onSeek(Double(cue.timeMillis) / Double(duration))
                            } label: {
                                hotCueBadge(cue)
                            }
                            .buttonStyle(.plain)
                            .allowsHitTesting(isLocalPlayback)
                            .help("Hot Cue \(cue.letter)")
                        }
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, LumiSpacing.small)
            .frame(height: 30)
            .background(Color.white.opacity(0.025))
            .accessibilityIdentifier("lumi.deck.\(deck.deckID).hotCues")
        }
    }

    private func hotCueBadge(_ cue: DeckHotCueSnapshot) -> some View {
        Text(verbatim: cue.letter)
            .font(LumiTypography.hotCueLetter)
            .foregroundStyle(Color.black.opacity(0.82))
            .frame(width: 19, height: 19)
            .background(hotCueColor(cue.colorRGB))
            .clipShape(RoundedRectangle(cornerRadius: 3))
    }

    private func metadataValue(_ label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text(verbatim: label)
                .font(LumiTypography.caption)
                .foregroundStyle(Color.white.opacity(0.48))
            Text(verbatim: value)
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(Color.white)
                .lineLimit(1)
        }
        .padding(.horizontal, LumiSpacing.small)
        .padding(.vertical, LumiSpacing.small)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func waveform(
        playheadBeat: Double,
        renderingViewport: LumiWaveformViewport
    ) -> some View {
        let displayViewport = isMaster && usesLiveViewport ? viewport : renderingViewport
        return ZStack(alignment: .topLeading) {
            Color.black
            if let preview = waveformPreview, !preview.points.isEmpty {
                RGBDeckWaveform(
                    points: preview.points,
                    channelMaximum: preview.source == "localLibraryDetail" ? 255 : 31,
                    waveformID: deck.trackLoadID,
                    durationBeats: deck.durationBeats,
                    beatGrid: beatGridTimeline,
                    hotCues: deck.hotCues.map {
                        WaveformHotCueMarker(
                            index: $0.index,
                            letter: $0.letter,
                            beat: hotCueBeat($0),
                            colorRGB: $0.colorRGB
                        )
                    },
                    playheadBeat: playheadBeat,
                    viewport: displayViewport,
                    visualClock: scrubProgress == nil ? visualClock : nil,
                    followsLiveViewport: isMaster && usesLiveViewport
                )
                Text(verbatim: "RGB · \(preview.source.uppercased())")
                    .font(LumiTypography.technical)
                    .foregroundStyle(Color.white.opacity(0.56))
                    .padding(LumiSpacing.small)
            } else {
                ContentUnavailableView(
                    "Waveform unavailable",
                    systemImage: "waveform.slash",
                    description: Text("Waiting for library or deck-provider data")
                )
                .foregroundStyle(Color.white.opacity(0.58))
            }
        }
        .frame(height: 156)
        .contentShape(Rectangle())
        .overlay {
            GeometryReader { proxy in
                Color.clear
                    .contentShape(Rectangle())
                    .overlay {
                        LumiWaveformInteractionMonitor(
                            onScroll: { deltaX in
                                // Once follow is suspended, every trackpad event
                                // must build on the viewport produced by the
                                // previous event. Reusing the captured live
                                // viewport made a gesture repeatedly jump from
                                // one stale origin and feel unresponsive.
                                let panOrigin = usesLiveViewport
                                    ? renderingViewport
                                    : viewport
                                let navigation = LiveDeckViewportPolicy.manualPan(
                                    renderedViewport: panOrigin,
                                    deltaPixels: deltaX,
                                    width: proxy.size.width,
                                    reversesDirection: reversesHorizontalScroll
                                )
                                viewport = navigation.viewport
                                usesLiveViewport = navigation.usesLiveViewport
                            },
                            onZoom: { delta, pointerFraction in
                                zoomFromScroll(delta, pointerFraction: pointerFraction)
                            }
                        )
                    }
                    .highPriorityGesture(
                        DragGesture(minimumDistance: 0)
                            .onChanged { value in
                                guard isLocalPlayback else { return }
                                scrubProgress = seekProgress(
                                    atX: value.location.x,
                                    width: proxy.size.width,
                                    renderingViewport: renderingViewport
                                )
                            }
                            .onEnded { value in
                                guard isLocalPlayback else { return }
                                let progress = seekProgress(
                                    atX: value.location.x,
                                    width: proxy.size.width,
                                    renderingViewport: renderingViewport
                                )
                                scrubProgress = nil
                                onSeek(progress)
                            }
                    )
                    .simultaneousGesture(
                        MagnifyGesture()
                            .onChanged { value in
                                let baseline = magnificationAnchorBeats
                                    ?? renderingViewport.visibleBeats
                                magnificationAnchorBeats = baseline
                                viewport = renderingViewport.zoomed(
                                    to: baseline / max(0.05, value.magnification),
                                    aroundBeat: playheadBeat
                                )
                            }
                            .onEnded { _ in magnificationAnchorBeats = nil }
                    )
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("RGB waveform for \(deck.title), beat \(Int(playheadBeat))")
        .accessibilityHint(
            isLocalPlayback
                ? "Click or drag to seek. Playback continues from the selected position."
                : "Waveform follows the connected deck."
        )
    }

    private func hotCueBeat(_ cue: DeckHotCueSnapshot) -> Double {
        if let beatGridTimeline {
            return beatGridTimeline.beat(atTimeMillis: Double(cue.timeMillis))
        }
        let bpm = Double(max(1, deck.bpmMilli)) / 1_000
        let durationMillis = Double(max(1, deck.durationBeats)) * 60_000 / bpm
        return Double(cue.timeMillis) / durationMillis * Double(max(1, deck.durationBeats))
    }

    private func hotCueColor(_ rgb: UInt32) -> Color {
        Color(
            red: Double((rgb >> 16) & 0xff) / 255,
            green: Double((rgb >> 8) & 0xff) / 255,
            blue: Double(rgb & 0xff) / 255
        )
    }

    private func displayedPlayheadBeat(at date: Date) -> Double {
        if let scrubProgress {
            return scrubProgress * Double(max(1, deck.durationBeats))
        }
        return LiveDeckVisualTimeline.playheadBeat(
            trackLoadID: deck.trackLoadID,
            durationBeats: deck.durationBeats,
            fallbackBeat: Double(deck.beat),
            visualClock: visualClock,
            beatGrid: beatGridTimeline,
            at: date
        )
    }

    private func seekProgress(
        atX x: Double,
        width: Double,
        renderingViewport: LumiWaveformViewport
    ) -> Double {
        let beat = renderingViewport.beat(atX: x, width: width)
        if let beatGridTimeline {
            return beatGridTimeline.trackProgress(atBeat: beat)
        }
        return min(max(0, beat / Double(max(1, deck.durationBeats))), 1)
    }

    private func phraseBand(
        playheadBeat: Double,
        renderingViewport: LumiWaveformViewport
    ) -> some View {
        let activePhraseIndex = phraseIndex(at: playheadBeat)
        return GeometryReader { proxy in
            ZStack(alignment: .leading) {
                ForEach(visiblePhrases(in: renderingViewport)) { phrase in
                    Button {
                        onSelectPhrase(phrase.index)
                    } label: {
                        HStack(spacing: 3) {
                            if cue(for: phrase)?.locked == true {
                                Image(systemName: "pin.fill")
                            }
                            Text(verbatim: phraseDisplayName(phrase))
                                .lineLimit(1)
                        }
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(Color.white)
                        .frame(
                            width: phraseWidth(
                                phrase,
                                totalWidth: proxy.size.width,
                                renderingViewport: renderingViewport
                            ),
                            height: 28
                        )
                        .background(phraseColor(phrase.roleID ?? phrase.kind))
                        .opacity(phrase.index < (activePhraseIndex ?? 0) ? 0.48 : 1)
                        .overlay {
                            if phrase.index == selectedPhraseIndex {
                                Rectangle().strokeBorder(LumiColor.accent, lineWidth: 3)
                            } else if phrase.index == activePhraseIndex {
                                Rectangle().strokeBorder(Color.white, lineWidth: 2)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .contentShape(Rectangle())
                    .offset(
                        x: phraseOffset(
                            phrase,
                            totalWidth: proxy.size.width,
                            renderingViewport: renderingViewport
                        )
                    )
                    .accessibilityIdentifier("lumi.deck.\(deck.deckID).phrase.\(phrase.index)")
                }
            }
        }
        .frame(height: 28)
        .padding(.horizontal, LumiSpacing.small)
        .padding(.bottom, LumiSpacing.small)
        .accessibilityElement(children: .contain)
    }

    @ViewBuilder
    private func plannedAutoloops(
        playheadBeat: Double,
        renderingViewport: LumiWaveformViewport
    ) -> some View {
        let items = PlannedAutoloopPresenter.items(
            deck: deck,
            plan: plan,
            isMaster: isMaster,
            playheadBeat: playheadBeat
        )
        if !items.isEmpty {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                HStack(spacing: LumiSpacing.small) {
                    Text(verbatim: "AUTOLOOP PLAN")
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(Color.white.opacity(0.52))
                    Text(verbatim: "\(items.count) PHRASES")
                        .font(LumiTypography.technical)
                        .foregroundStyle(Color.white.opacity(0.36))
                    Spacer()
                    Text(verbatim: "Click an item to edit")
                        .font(LumiTypography.technical)
                        .foregroundStyle(Color.white.opacity(0.36))
                }
                plannedAutoloopTimeline(
                    items,
                    playheadBeat: playheadBeat,
                    renderingViewport: renderingViewport
                )
            }
            .padding(.horizontal, LumiSpacing.small)
            .padding(.vertical, LumiSpacing.small)
            .background(Color.white.opacity(0.025))
            .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.1)) }
            .accessibilityIdentifier("lumi.deck.\(deck.deckID).autoloopPlan")
        }
    }

    private func plannedAutoloopTimeline(
        _ items: [PlannedAutoloopPresentation],
        playheadBeat: Double,
        renderingViewport: LumiWaveformViewport
    ) -> some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Color.black.opacity(0.7)
                ForEach(items) { item in
                    if let phrase = phrase(for: item),
                       phraseIsVisible(phrase, in: renderingViewport) {
                        plannedAutoloopBlock(
                            item,
                            width: phraseWidth(
                                phrase,
                                totalWidth: proxy.size.width,
                                renderingViewport: renderingViewport
                            )
                        )
                        .offset(
                            x: phraseOffset(
                                phrase,
                                totalWidth: proxy.size.width,
                                renderingViewport: renderingViewport
                            )
                        )
                    }
                }
                if playheadIsVisible(playheadBeat, in: renderingViewport) {
                    lightPlanPlayhead
                        .offset(
                            x: renderingViewport.x(
                                forBeat: playheadBeat,
                                width: proxy.size.width
                            ) - 5
                        )
                }
            }
        }
        .frame(height: 82)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.compact)
                .strokeBorder(Color.white.opacity(0.1), lineWidth: 1)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Synchronized AutoLoop plan timeline")
    }

    private func plannedAutoloopBlock(
        _ item: PlannedAutoloopPresentation,
        width: CGFloat
    ) -> some View {
        Button {
            onSelectPhrase(item.phraseIndex)
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 4) {
                    Circle()
                        .fill(autoloopStatusColor(item.status))
                        .frame(width: 6, height: 6)
                    if width >= 58 {
                        Text(verbatim: autoloopStatusLabel(item.status))
                            .font(LumiTypography.technical.weight(.semibold))
                            .foregroundStyle(autoloopStatusColor(item.status))
                    }
                    if item.locked {
                        Image(systemName: "lock.fill")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.warning)
                    }
                }
                if width >= 72 {
                    Text(verbatim: item.phraseName.uppercased())
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(Color.white.opacity(0.56))
                        .lineLimit(1)
                }
                if width >= 42 {
                    Text(verbatim: item.autoloopName)
                        .font(LumiTypography.metadata.weight(.semibold))
                        .foregroundStyle(Color.white)
                        .lineLimit(1)
                }
                if width >= 100 {
                    if let bank = item.bankNumber, let slot = item.slotNumber {
                        Text(verbatim: "BANK \(bank) · LOOP \(slot)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(Color.white.opacity(0.42))
                            .lineLimit(1)
                    } else if item.holdsCurrentLook {
                        Text(verbatim: "NO MIDI CHANGE")
                            .font(LumiTypography.technical)
                            .foregroundStyle(Color.white.opacity(0.42))
                            .lineLimit(1)
                    }
                }
            }
            .padding(.horizontal, 6)
            .padding(.vertical, LumiSpacing.xSmall)
            .frame(width: width, height: 82, alignment: .topLeading)
            .background(autoloopStatusColor(item.status).opacity(0.11))
            .overlay(alignment: .top) {
                Rectangle()
                    .fill(autoloopStatusColor(item.status))
                    .frame(height: item.status == .active ? 3 : 2)
            }
            .overlay {
                Rectangle().strokeBorder(
                    item.phraseIndex == selectedPhraseIndex
                        ? LumiColor.accent
                        : Color.white.opacity(0.1),
                    lineWidth: item.phraseIndex == selectedPhraseIndex ? 2 : 1
                )
            }
            .opacity(item.status == .completed ? 0.5 : 1)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(
            "\(item.phraseName), \(item.autoloopName), \(autoloopStatusLabel(item.status))"
        )
        .accessibilityIdentifier(
            "lumi.deck.\(deck.deckID).autoloop.\(item.phraseIndex)"
        )
    }

    private var lightPlanPlayhead: some View {
        ZStack(alignment: .top) {
            Rectangle()
                .fill(LumiColor.accent)
                .frame(width: 2, height: 82)
                .shadow(color: LumiColor.accent.opacity(0.8), radius: 3)
            Image(systemName: "arrowtriangle.down.fill")
                .font(LumiTypography.caption.weight(.bold))
                .foregroundStyle(LumiColor.accent)
        }
        .frame(width: 10, height: 82)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func autoloopStatusLabel(_ status: PlannedAutoloopStatus) -> String {
        switch status {
        case .active: "ACTIVE"
        case .next: "NEXT"
        case .planned: "PLANNED"
        case .completed: "DONE"
        }
    }

    private func autoloopStatusColor(_ status: PlannedAutoloopStatus) -> Color {
        switch status {
        case .active: LumiColor.destructive
        case .next: LumiColor.accent
        case .planned: Color.white.opacity(0.62)
        case .completed: LumiColor.success
        }
    }

    private var deckName: String {
        switch deck.deckID {
        case 1: "DECK A"
        case 2: "DECK B"
        default: "DECK \(deck.deckID)"
        }
    }

    private func activePhraseName(at playheadBeat: Double) -> String {
        guard let phraseIndex = phraseIndex(at: playheadBeat),
              let phrase = deck.phrases.first(where: { $0.index == phraseIndex }) else {
            return "Not started"
        }
        return phraseDisplayName(phrase)
    }

    private func visiblePhrases(
        in renderingViewport: LumiWaveformViewport
    ) -> [DeckPhraseSnapshot] {
        deck.phrases.filter { phraseIsVisible($0, in: renderingViewport) }
    }

    private func phraseIsVisible(
        _ phrase: DeckPhraseSnapshot,
        in renderingViewport: LumiWaveformViewport
    ) -> Bool {
        Double(phrase.endBeat) > renderingViewport.startBeat
            && Double(phrase.startBeat) < renderingViewport.endBeat
    }

    private func phrase(for item: PlannedAutoloopPresentation) -> DeckPhraseSnapshot? {
        deck.phrases.first(where: { $0.index == item.phraseIndex })
    }

    private func playheadIsVisible(
        _ playheadBeat: Double,
        in renderingViewport: LumiWaveformViewport
    ) -> Bool {
        playheadBeat >= renderingViewport.startBeat
            && playheadBeat <= renderingViewport.endBeat
    }

    private func phraseWidth(
        _ phrase: DeckPhraseSnapshot,
        totalWidth: CGFloat,
        renderingViewport: LumiWaveformViewport
    ) -> CGFloat {
        let start = max(Double(phrase.startBeat), renderingViewport.startBeat)
        let end = min(Double(phrase.endBeat), renderingViewport.endBeat)
        return max(
            1,
            totalWidth * CGFloat(
                max(0, end - start) / renderingViewport.visibleBeats
            ) - 1
        )
    }

    private func phraseOffset(
        _ phrase: DeckPhraseSnapshot,
        totalWidth: CGFloat,
        renderingViewport: LumiWaveformViewport
    ) -> CGFloat {
        let start = max(Double(phrase.startBeat), renderingViewport.startBeat)
        return totalWidth * CGFloat(
            (start - renderingViewport.startBeat) / renderingViewport.visibleBeats
        )
    }

    private func phraseIndex(at beat: Double) -> UInt64? {
        deck.phrases.first(where: {
            beat >= Double($0.startBeat) && beat < Double($0.endBeat)
        })?.index ?? deck.phrases.last?.index
    }

    private var playbackIsActive: Bool {
        visualClock?.playing ?? deck.playing
    }

    private var waveformPreview: DeckWaveformPreviewSnapshot? {
        waveformOverride ?? deck.waveformPreview
    }

    private var beatGridTimeline: LiveBeatGridTimeline? {
        LiveBeatGridTimeline(grid: deck.beatGrid, totalBeats: deck.durationBeats)
    }

    private func renderingViewport(for playheadBeat: Double) -> LumiWaveformViewport {
        guard isMaster, usesLiveViewport else {
            return viewport
        }
        return LiveDeckViewportPolicy.live(
            playheadBeat: playheadBeat,
            totalBeats: viewport.totalBeats,
            visibleBeats: viewport.visibleBeats,
            beatsPerBar: viewport.beatsPerBar
        )
    }

    private func activateDefaultLiveViewport() {
        viewport = LiveDeckViewportPolicy.live(
            playheadBeat: displayedPlayheadBeat(at: Date()),
            totalBeats: deck.durationBeats,
            beatsPerBar: viewport.beatsPerBar
        )
        usesLiveViewport = true
    }

    private func freezeCurrentLiveViewport() {
        guard usesLiveViewport else { return }
        viewport = renderingViewport(
            for: displayedPlayheadBeat(at: Date())
        )
    }

    private var zoomSliderBinding: Binding<Double> {
        Binding(
            get: {
                let total = Double(max(1, deck.durationBeats))
                guard total > 1 else { return 1 }
                return min(max(0, log(total / viewport.visibleBeats) / log(total)), 1)
            },
            set: { value in
                let total = Double(max(1, deck.durationBeats))
                let visible = total / pow(total, min(max(0, value), 1))
                viewport = viewport.zoomed(to: visible, aroundBeat: Double(deck.beat))
            }
        )
    }

    private var waveformZoomAnchor: LumiWaveformZoomAnchor {
        LumiWaveformZoomAnchor(rawValue: waveformZoomAnchorRaw) ?? .mouse
    }

    private var waveformZoomAnchorBinding: Binding<LumiWaveformZoomAnchor> {
        Binding(
            get: { waveformZoomAnchor },
            set: { waveformZoomAnchorRaw = $0.rawValue }
        )
    }

    private func zoomFromScroll(_ delta: Double, pointerFraction: Double) {
        let boundedDelta = min(max(delta, -24), 24)
        let factor = exp(-boundedDelta * 0.025)
        let anchorBeat = switch waveformZoomAnchor {
        case .mouse:
            viewport.startBeat + viewport.visibleBeats * pointerFraction
        case .playhead:
            Double(deck.beat)
        }
        viewport = viewport.zoomed(
            to: viewport.visibleBeats * factor,
            aroundBeat: anchorBeat
        )
    }

    private func resetViewport() {
        usesLiveViewport = false
        viewport = LiveDeckViewportPolicy.overview(totalBeats: deck.durationBeats)
    }

    private func phraseColor(_ role: String) -> Color {
        switch role {
        case "intro-outro", "intro", "outro": Color(red: 0.25, green: 0.55, blue: 0.95)
        case "bridge": Color(red: 0.37, green: 0.42, blue: 0.78)
        case "breakdown-1", "breakdown-2", "breakdown-3", "breakdown": Color(red: 0.48, green: 0.28, blue: 0.83)
        case "synth": Color(red: 0.82, green: 0.24, blue: 0.72)
        case "pre-drop": Color(red: 0.95, green: 0.46, blue: 0.20)
        case "buildup-1", "buildup-2", "buildup-3", "build": Color(red: 0.96, green: 0.66, blue: 0.12)
        case "drop": Color(red: 0.92, green: 0.20, blue: 0.26)
        default: Color(red: 0.20, green: 0.68, blue: 0.60)
        }
    }

    private func phraseDisplayName(_ phrase: DeckPhraseSnapshot) -> String {
        phrase.roleName ?? phrase.kind.capitalized
    }

    private func cue(for phrase: DeckPhraseSnapshot) -> PlanCueSnapshot? {
        plan?.cues.first(where: { $0.phraseIndex == phrase.index })
    }
}

private struct RGBDeckWaveform: View {
    let points: [DeckWaveformPointSnapshot]
    let channelMaximum: Double
    let waveformID: UInt64
    let durationBeats: UInt64
    let beatGrid: LiveBeatGridTimeline?
    let hotCues: [WaveformHotCueMarker]
    let playheadBeat: Double
    let viewport: LumiWaveformViewport
    let visualClock: DeckVisualClockSnapshot?
    let followsLiveViewport: Bool
    @State private var rasterImage: CGImage?

    init(
        points: [DeckWaveformPointSnapshot],
        channelMaximum: Double,
        waveformID: UInt64,
        durationBeats: UInt64,
        beatGrid: LiveBeatGridTimeline?,
        hotCues: [WaveformHotCueMarker],
        playheadBeat: Double,
        viewport: LumiWaveformViewport,
        visualClock: DeckVisualClockSnapshot?,
        followsLiveViewport: Bool
    ) {
        self.points = points
        self.channelMaximum = channelMaximum
        self.waveformID = waveformID
        self.durationBeats = durationBeats
        self.beatGrid = beatGrid
        self.hotCues = hotCues
        self.playheadBeat = playheadBeat
        self.viewport = viewport
        self.visualClock = visualClock
        self.followsLiveViewport = followsLiveViewport
    }

    var body: some View {
        GeometryReader { proxy in
            if let rasterImage {
                RGBWaveformLayerView(
                    rasterImage: rasterImage,
                    motion: motionPlan,
                    hotCues: hotCues,
                    viewportWidth: proxy.size.width
                )
            }
        }
        .task(id: rasterKey) {
            let samples = points
            let zoomScale = Double(max(1, durationBeats)) / viewport.visibleBeats
            let rasterWidth = max(
                samples.count,
                min(65_536, Int(ceil(2_048 * zoomScale)))
            )
            rasterImage = await Task.detached(priority: .utility) {
                Self.makeRasterImage(
                    points: samples,
                    width: rasterWidth,
                    channelMaximum: channelMaximum,
                    durationBeats: durationBeats,
                    beatsPerBar: beatGrid?.beatsPerBar ?? viewport.beatsPerBar,
                    beatGrid: beatGrid
                )
            }.value
        }
    }

    private var motionPlan: LiveWaveformMotionPlan {
        LiveWaveformMotionPlan(
            waveformID: waveformID,
            totalBeats: Double(max(1, durationBeats)),
            viewportStartBeat: viewport.startBeat,
            visibleBeats: viewport.visibleBeats,
            followsLiveViewport: followsLiveViewport,
            fallbackPlayheadBeat: playheadBeat,
            visualClock: visualClock,
            beatGrid: beatGrid
        )
    }

    private var rasterKey: WaveformRasterKey {
        WaveformRasterKey(
            waveformID: waveformID,
            durationBeats: durationBeats,
            visibleBeats: viewport.visibleBeats,
            pointCount: points.count,
            channelMaximum: channelMaximum,
            beatGridMarkerCount: beatGrid?.timesMillis.count ?? 0,
            firstBeatTimeMillis: beatGrid?.timesMillis.first,
            lastBeatTimeMillis: beatGrid?.timesMillis.last
        )
    }

    nonisolated private static func makeRasterImage(
        points: [DeckWaveformPointSnapshot],
        width: Int,
        channelMaximum: Double,
        durationBeats: UInt64,
        beatsPerBar: UInt8,
        beatGrid: LiveBeatGridTimeline?
    ) -> CGImage? {
        guard !points.isEmpty else { return nil }
        let width = max(1, width)
        let height = 156
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }
        let center = Double(height) / 2
        let maximumAmplitude = Double(height) * 0.43
        context.setLineWidth(1)
        for pixel in 0..<width {
            let beat = Double(pixel) / Double(max(1, width - 1))
                * Double(max(1, durationBeats))
            let trackProgress = beatGrid?.trackProgress(atBeat: beat)
                ?? beat / Double(max(1, durationBeats))
            let position = trackProgress * Double(max(0, points.count - 1))
            let lower = Int(position.rounded(.down))
            let upper = min(points.count - 1, lower + 1)
            let sampleFraction = position - Double(lower)
            let first = points[lower]
            let second = points[upper]
            func mix(_ lhs: UInt8, _ rhs: UInt8) -> Double {
                (Double(lhs) + (Double(rhs) - Double(lhs)) * sampleFraction)
                    / max(1, channelMaximum)
            }
            let low = mix(first.low, second.low)
            let mid = mix(first.mid, second.mid)
            let high = mix(first.high, second.high)
            let peak = max(low, max(mid, high))
            guard peak > 0.000_1 else { continue }
            let amplitude = pow(peak, 0.58) * maximumAmplitude
            context.setStrokeColor(
                red: pow(high / peak, 0.72),
                green: pow(mid / peak, 0.72),
                blue: pow(low / peak, 0.72),
                alpha: 0.98
            )
            let x = Double(pixel) + 0.5
            context.move(to: CGPoint(x: x, y: center - amplitude))
            context.addLine(to: CGPoint(x: x, y: center + amplitude))
            context.strokePath()
        }
        for beat in 0...Int(max(1, durationBeats)) {
            let x = Double(beat) / Double(max(1, durationBeats)) * Double(width)
            let isBar = beat.isMultiple(of: Int(max(1, beatsPerBar)))
            context.setLineWidth(isBar ? 1.2 : 0.6)
            context.setStrokeColor(
                red: 1,
                green: 1,
                blue: 1,
                alpha: isBar ? 0.30 : 0.09
            )
            context.move(to: CGPoint(x: x, y: 0))
            context.addLine(to: CGPoint(x: x, y: Double(height)))
            context.strokePath()
        }
        return context.makeImage()
    }

}

private struct WaveformHotCueMarker: Equatable {
    let index: UInt8
    let letter: String
    let beat: Double
    let colorRGB: UInt32
}

struct LiveWaveformMotionPlan: Equatable {
    let waveformID: UInt64
    let totalBeats: Double
    let viewportStartBeat: Double
    let visibleBeats: Double
    let followsLiveViewport: Bool
    let fallbackPlayheadBeat: Double
    let visualClock: DeckVisualClockSnapshot?
    let beatGrid: LiveBeatGridTimeline?

    init(
        waveformID: UInt64,
        totalBeats: Double,
        viewportStartBeat: Double,
        visibleBeats: Double,
        followsLiveViewport: Bool,
        fallbackPlayheadBeat: Double,
        visualClock: DeckVisualClockSnapshot?,
        beatGrid: LiveBeatGridTimeline? = nil
    ) {
        self.waveformID = waveformID
        self.totalBeats = totalBeats
        self.viewportStartBeat = viewportStartBeat
        self.visibleBeats = visibleBeats
        self.followsLiveViewport = followsLiveViewport
        self.fallbackPlayheadBeat = fallbackPlayheadBeat
        self.visualClock = visualClock
        self.beatGrid = beatGrid
    }

    var animationIdentity: AnimationIdentity {
        let hasAuthoritativeClock = visualClock?.trackLoadID == waveformID
            && (visualClock?.durationMillis ?? 0) > 0
        return AnimationIdentity(
            waveformID: waveformID,
            viewportStartBeat: viewportStartBeat,
            visibleBeats: visibleBeats,
            followsLiveViewport: followsLiveViewport,
            positionMillis: visualClock?.positionMillis,
            durationMillis: visualClock?.durationMillis,
            playing: visualClock?.playing,
            anchoredAtReferenceTime: visualClock?.anchoredAtReferenceTime,
            playbackRate: visualClock?.playbackRate,
            discontinuityRevision: visualClock?.discontinuityRevision,
            fallbackPlayheadBeat: hasAuthoritativeClock ? nil : fallbackPlayheadBeat,
            beatGridMarkerCount: beatGrid?.timesMillis.count ?? 0
        )
    }

    func playheadBeat(at date: Date) -> Double {
        LiveDeckVisualTimeline.playheadBeat(
            trackLoadID: waveformID,
            durationBeats: UInt64(max(1, totalBeats.rounded())),
            fallbackBeat: fallbackPlayheadBeat,
            visualClock: visualClock,
            beatGrid: beatGrid,
            at: date
        )
    }

    func startBeat(for playheadBeat: Double) -> Double {
        guard followsLiveViewport else { return viewportStartBeat }
        let maximumStart = max(0, totalBeats - visibleBeats)
        return min(
            maximumStart,
            max(0, playheadBeat - visibleBeats * LiveDeckViewportPolicy.playheadFraction)
        )
    }

    func positionMillis(at date: Date) -> Double? {
        guard let visualClock,
              visualClock.trackLoadID == waveformID,
              visualClock.durationMillis > 0 else {
            return nil
        }
        return visualClock.positionMillis(at: date)
    }

    func remainingDuration(at date: Date) -> TimeInterval? {
        guard let visualClock,
              visualClock.trackLoadID == waveformID,
              visualClock.durationMillis > 0,
              visualClock.playing,
              let positionMillis = positionMillis(at: date) else {
            return nil
        }
        return max(
            0.01,
            (Double(visualClock.durationMillis) - positionMillis) / 1_000
                / max(0.000_001, visualClock.playbackRate)
        )
    }

    func timeMillis(atBeat beat: Double) -> Double {
        if let beatGrid {
            return beatGrid.timeMillis(atBeat: beat)
        }
        guard let visualClock, visualClock.durationMillis > 0 else { return beat }
        return min(max(0, beat / totalBeats), 1) * Double(visualClock.durationMillis)
    }

    var playbackEndBeat: Double {
        guard let visualClock, visualClock.durationMillis > 0 else {
            return totalBeats
        }
        return beatGrid.map {
            min(totalBeats, $0.beat(atTimeMillis: Double(visualClock.durationMillis)))
        } ?? totalBeats
    }

    struct AnimationIdentity: Equatable {
        let waveformID: UInt64
        let viewportStartBeat: Double
        let visibleBeats: Double
        let followsLiveViewport: Bool
        let positionMillis: UInt64?
        let durationMillis: UInt64?
        let playing: Bool?
        let anchoredAtReferenceTime: TimeInterval?
        let playbackRate: Double?
        let discontinuityRevision: UInt64?
        let fallbackPlayheadBeat: Double?
        let beatGridMarkerCount: Int
    }
}

private struct WaveformRasterKey: Hashable {
    let waveformID: UInt64
    let durationBeats: UInt64
    let visibleBeats: Double
    let pointCount: Int
    let channelMaximum: Double
    let beatGridMarkerCount: Int
    let firstBeatTimeMillis: UInt64?
    let lastBeatTimeMillis: UInt64?
}

private struct RGBWaveformLayerView: NSViewRepresentable {
    let rasterImage: CGImage
    let motion: LiveWaveformMotionPlan
    let hotCues: [WaveformHotCueMarker]
    let viewportWidth: CGFloat

    func makeNSView(context: Context) -> RGBWaveformLayerHostView {
        RGBWaveformLayerHostView()
    }

    func updateNSView(_ nsView: RGBWaveformLayerHostView, context: Context) {
        nsView.update(
            rasterImage: rasterImage,
            motion: motion,
            hotCues: hotCues,
            viewportWidth: viewportWidth
        )
    }
}

private final class RGBWaveformLayerHostView: NSView {
    private let waveformLayer = CALayer()
    private let playheadLayer = CALayer()
    private let playheadCapLayer = CALayer()
    private var rasterImage: CGImage?
    private var motion: LiveWaveformMotionPlan?
    private var hotCues: [WaveformHotCueMarker] = []
    private var hotCueLayers: [(line: CALayer, badge: CATextLayer)] = []
    private var viewportWidth: CGFloat = 0
    private var animationIdentity: LiveWaveformMotionPlan.AnimationIdentity?
    private var appliedBoundsSize = CGSize.zero

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.masksToBounds = true
        layer?.backgroundColor = NSColor.black.cgColor
        waveformLayer.anchorPoint = .zero
        waveformLayer.contentsGravity = .resize
        waveformLayer.magnificationFilter = .linear
        waveformLayer.minificationFilter = .linear
        playheadLayer.anchorPoint = .zero
        playheadLayer.backgroundColor = NSColor.white.cgColor
        playheadLayer.shadowColor = NSColor.black.cgColor
        playheadLayer.shadowOpacity = 0.5
        playheadLayer.shadowRadius = 1
        playheadCapLayer.anchorPoint = .zero
        playheadCapLayer.backgroundColor = NSColor.white.cgColor
        layer?.addSublayer(waveformLayer)
        layer?.addSublayer(playheadLayer)
        layer?.addSublayer(playheadCapLayer)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        nil
    }

    override func layout() {
        super.layout()
        guard abs(appliedBoundsSize.width - bounds.width) > 0.5
                || abs(appliedBoundsSize.height - bounds.height) > 0.5 else {
            return
        }
        applyCurrentState(restartAnimation: true)
    }

    func update(
        rasterImage: CGImage,
        motion: LiveWaveformMotionPlan,
        hotCues: [WaveformHotCueMarker],
        viewportWidth: CGFloat
    ) {
        let imageChanged = self.rasterImage !== rasterImage
        let motionChanged = animationIdentity != motion.animationIdentity
        let widthChanged = abs(self.viewportWidth - viewportWidth) > 0.5
        let hotCuesChanged = self.hotCues != hotCues
        self.rasterImage = rasterImage
        self.motion = motion
        self.hotCues = hotCues
        self.viewportWidth = viewportWidth
        if imageChanged {
            waveformLayer.contents = rasterImage
        }
        if hotCuesChanged {
            rebuildHotCueLayers()
        }
        if imageChanged || motionChanged || widthChanged || hotCuesChanged {
            animationIdentity = motion.animationIdentity
            applyCurrentState(restartAnimation: true)
        }
    }

    private func applyCurrentState(restartAnimation: Bool) {
        guard let motion, bounds.width > 0, bounds.height > 0 else { return }
        appliedBoundsSize = bounds.size
        if restartAnimation {
            waveformLayer.removeAllAnimations()
            playheadLayer.removeAllAnimations()
            playheadCapLayer.removeAllAnimations()
        }

        let width = bounds.width
        let height = bounds.height
        let fullTrackWidth = width * CGFloat(motion.totalBeats / motion.visibleBeats)
        let now = Date()
        let currentBeat = motion.playheadBeat(at: now)
        let currentStartBeat = motion.startBeat(for: currentBeat)
        let waveformX = -width * CGFloat(currentStartBeat / motion.visibleBeats)
        let playheadX = width * CGFloat((currentBeat - currentStartBeat) / motion.visibleBeats)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        waveformLayer.bounds = CGRect(x: 0, y: 0, width: fullTrackWidth, height: height)
        waveformLayer.position = CGPoint(x: waveformX, y: 0)
        layoutHotCueLayers(fullTrackWidth: fullTrackWidth, height: height, motion: motion)
        playheadLayer.bounds = CGRect(x: 0, y: 0, width: 2, height: height)
        playheadLayer.position = CGPoint(x: playheadX - 1, y: 0)
        playheadCapLayer.bounds = CGRect(x: 0, y: 0, width: 6, height: 7)
        playheadCapLayer.position = CGPoint(x: playheadX - 3, y: 0)
        CATransaction.commit()

        guard restartAnimation,
              let remainingDuration = motion.remainingDuration(at: now),
              let currentPositionMillis = motion.positionMillis(at: now),
              currentBeat < motion.playbackEndBeat else {
            return
        }
        animateWaveform(
            motion: motion,
            currentBeat: currentBeat,
            currentPositionMillis: currentPositionMillis,
            width: width,
            duration: remainingDuration
        )
        animatePlayhead(
            motion: motion,
            currentBeat: currentBeat,
            currentPositionMillis: currentPositionMillis,
            width: width,
            duration: remainingDuration
        )
    }

    private func rebuildHotCueLayers() {
        hotCueLayers.forEach {
            $0.line.removeFromSuperlayer()
            $0.badge.removeFromSuperlayer()
        }
        hotCueLayers = hotCues.map { cue in
            let color = NSColor(
                red: CGFloat((cue.colorRGB >> 16) & 0xff) / 255,
                green: CGFloat((cue.colorRGB >> 8) & 0xff) / 255,
                blue: CGFloat(cue.colorRGB & 0xff) / 255,
                alpha: 1
            )
            let line = CALayer()
            line.anchorPoint = .zero
            line.backgroundColor = color.withAlphaComponent(0.72).cgColor
            let badge = CATextLayer()
            badge.anchorPoint = .zero
            badge.string = cue.letter
            badge.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .semibold)
            badge.fontSize = 10
            badge.alignmentMode = .center
            badge.foregroundColor = NSColor.black.withAlphaComponent(0.82).cgColor
            badge.backgroundColor = color.cgColor
            badge.cornerRadius = 3
            badge.contentsScale = NSScreen.main?.backingScaleFactor ?? 2
            waveformLayer.addSublayer(line)
            waveformLayer.addSublayer(badge)
            return (line, badge)
        }
    }

    private func layoutHotCueLayers(
        fullTrackWidth: CGFloat,
        height: CGFloat,
        motion: LiveWaveformMotionPlan
    ) {
        for (index, layers) in hotCueLayers.enumerated() where index < hotCues.count {
            let cue = hotCues[index]
            let x = fullTrackWidth * CGFloat(cue.beat / max(1, motion.totalBeats))
            layers.line.bounds = CGRect(x: 0, y: 0, width: 1, height: height)
            layers.line.position = CGPoint(x: x - 0.5, y: 0)
            layers.badge.bounds = CGRect(x: 0, y: 0, width: 17, height: 17)
            layers.badge.position = CGPoint(x: x - 8.5, y: 0)
        }
    }

    private func animateWaveform(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double,
        currentPositionMillis: Double,
        width: CGFloat,
        duration: TimeInterval
    ) {
        let keyBeats = animationKeyBeats(motion: motion, currentBeat: currentBeat)
        let values = keyBeats.map { beat in
            NSNumber(value: Double(-width * CGFloat(
                motion.startBeat(for: beat) / motion.visibleBeats
            )))
        }
        let animation = keyframeAnimation(
            keyPath: "position.x",
            values: values,
            keyBeats: keyBeats,
            motion: motion,
            currentPositionMillis: currentPositionMillis,
            duration: duration
        )
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        waveformLayer.position.x = values.last?.doubleValue ?? waveformLayer.position.x
        CATransaction.commit()
        waveformLayer.add(animation, forKey: "lumi.waveform.motion")
    }

    private func animatePlayhead(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double,
        currentPositionMillis: Double,
        width: CGFloat,
        duration: TimeInterval
    ) {
        let keyBeats = animationKeyBeats(motion: motion, currentBeat: currentBeat)
        let centerValues = keyBeats.map { beat in
            let startBeat = motion.startBeat(for: beat)
            return width * CGFloat((beat - startBeat) / motion.visibleBeats)
        }
        let lineValues = centerValues.map { NSNumber(value: Double($0 - 1)) }
        let capValues = centerValues.map { NSNumber(value: Double($0 - 3)) }
        let lineAnimation = keyframeAnimation(
            keyPath: "position.x",
            values: lineValues,
            keyBeats: keyBeats,
            motion: motion,
            currentPositionMillis: currentPositionMillis,
            duration: duration
        )
        let capAnimation = keyframeAnimation(
            keyPath: "position.x",
            values: capValues,
            keyBeats: keyBeats,
            motion: motion,
            currentPositionMillis: currentPositionMillis,
            duration: duration
        )
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playheadLayer.position.x = lineValues.last?.doubleValue ?? playheadLayer.position.x
        playheadCapLayer.position.x = capValues.last?.doubleValue ?? playheadCapLayer.position.x
        CATransaction.commit()
        playheadLayer.add(lineAnimation, forKey: "lumi.playhead.motion")
        playheadCapLayer.add(capAnimation, forKey: "lumi.playhead.cap.motion")
    }

    private func animationKeyBeats(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double
    ) -> [Double] {
        var keyBeats = [currentBeat]
        if let beatGrid = motion.beatGrid {
            keyBeats.append(contentsOf: beatGrid.timesMillis.indices.lazy
                .map(Double.init)
                .filter { $0 > currentBeat && $0 < motion.playbackEndBeat })
        }
        guard motion.followsLiveViewport else {
            keyBeats.append(motion.playbackEndBeat)
            return uniqueSortedBeats(keyBeats)
        }
        let leadingBeat = motion.visibleBeats * LiveDeckViewportPolicy.playheadFraction
        let trailingBeat = max(
            leadingBeat,
            motion.totalBeats - motion.visibleBeats
                * (1 - LiveDeckViewportPolicy.playheadFraction)
        )
        keyBeats.append(contentsOf: [
            leadingBeat,
            trailingBeat,
            motion.playbackEndBeat
        ])
        return uniqueSortedBeats(keyBeats.filter {
            $0 >= currentBeat && $0 <= motion.playbackEndBeat
        })
    }

    private func uniqueSortedBeats(_ values: [Double]) -> [Double] {
        values.sorted().reduce(into: [Double]()) { beats, beat in
            if beats.last.map({ abs($0 - beat) > 0.000_1 }) ?? true {
                beats.append(beat)
            }
        }
    }

    private func keyframeAnimation(
        keyPath: String,
        values: [NSNumber],
        keyBeats: [Double],
        motion: LiveWaveformMotionPlan,
        currentPositionMillis: Double,
        duration: TimeInterval
    ) -> CAKeyframeAnimation {
        let finalTimeMillis = max(
            currentPositionMillis + 0.001,
            motion.timeMillis(atBeat: motion.playbackEndBeat)
        )
        let remainingMillis = finalTimeMillis - currentPositionMillis
        let animation = CAKeyframeAnimation(keyPath: keyPath)
        animation.values = values
        animation.keyTimes = keyBeats.enumerated().map { index, beat in
            guard index > 0 else { return NSNumber(value: 0) }
            return NSNumber(value: min(max(
                0,
                (motion.timeMillis(atBeat: beat) - currentPositionMillis)
                    / remainingMillis
            ), 1))
        }
        animation.timingFunctions = Array(
            repeating: CAMediaTimingFunction(name: .linear),
            count: max(0, values.count - 1)
        )
        animation.duration = duration
        animation.isRemovedOnCompletion = true
        return animation
    }
}
