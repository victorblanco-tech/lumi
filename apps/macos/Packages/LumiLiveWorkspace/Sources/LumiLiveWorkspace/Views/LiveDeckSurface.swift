import LumiDesignSystem
import SwiftUI

struct LiveDeckSurface<Details: View>: View {
    let deck: DeckSnapshot
    let isMaster: Bool
    let plan: PlanSnapshot?
    let musicalKey: String
    let isLocalPlayback: Bool
    let visualClock: LocalPlaybackVisualClockSnapshot?
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
    @AppStorage("nl.blancoservices.lumi.waveform.zoom-anchor")
    private var waveformZoomAnchorRaw = LumiWaveformZoomAnchor.mouse.rawValue
    @AppStorage("nl.blancoservices.lumi.waveform.reverse-horizontal-scroll")
    private var reversesHorizontalScroll = false

    init(
        deck: DeckSnapshot,
        isMaster: Bool,
        plan: PlanSnapshot?,
        musicalKey: String,
        isLocalPlayback: Bool,
        visualClock: LocalPlaybackVisualClockSnapshot? = nil,
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
        self.plan = plan
        self.musicalKey = musicalKey
        self.isLocalPlayback = isLocalPlayback
        self.visualClock = visualClock
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
            if deck.waveformPreview?.points.isEmpty == false { waveformToolbar }
            synchronizedDeckTimeline
            details
        }
        .background(Color.black)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.panel)
                .strokeBorder(
                    isMaster ? LumiColor.destructive : LumiColor.border,
                    lineWidth: isMaster ? 2 : 1
                )
        }
        .shadow(
            color: isMaster ? LumiColor.destructive.opacity(0.18) : Color.clear,
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
    }

    private var synchronizedDeckTimeline: some View {
        TimelineView(
            .animation(
                minimumInterval: 1.0 / 30.0,
                paused: visualClock?.playing != true || scrubProgress != nil
            )
        ) { context in
            let playheadBeat = displayedPlayheadBeat(at: context.date)
            let renderingViewport = renderingViewport(for: playheadBeat)
            VStack(alignment: .leading, spacing: 0) {
                waveform(playheadBeat: playheadBeat, renderingViewport: renderingViewport)
                phraseBand(playheadBeat: playheadBeat, renderingViewport: renderingViewport)
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
            roleBadge
        }
        .padding(LumiSpacing.medium)
        .background {
            if isMaster {
                LinearGradient(
                    colors: [LumiColor.destructive.opacity(0.16), Color.clear],
                    startPoint: .leading,
                    endPoint: .trailing
                )
            }
        }
    }

    @ViewBuilder
    private var roleBadge: some View {
        if isMaster {
            Label("MASTER · LIVE NOW", systemImage: "circle.fill")
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(Color.white)
                .padding(.horizontal, LumiSpacing.small)
                .frame(height: LumiControlMetric.compactHeight)
                .background(LumiColor.destructive.opacity(0.9))
                .clipShape(Capsule())
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

    private var metadata: some View {
        HStack(spacing: 0) {
            metadataValue("BPM", value: String(
                format: "%.1f",
                locale: Locale(identifier: "en_US_POSIX"),
                Double(deck.bpmMilli) / 1_000
            ))
            metadataValue("KEY", value: musicalKey)
            metadataValue("BEAT", value: "\(deck.beat)")
            metadataValue("TRANSPORT", value: deck.playing ? "PLAYING" : "PAUSED")
            metadataValue("PHRASE", value: activePhraseName)
        }
        .background(Color.white.opacity(0.035))
        .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.12)) }
        .overlay(alignment: .bottom) { Divider().overlay(Color.white.opacity(0.12)) }
    }

    private var transportControls: some View {
        HStack(spacing: LumiSpacing.small) {
            Button(action: onTogglePlayback) {
                Label(deck.playing ? "Pause" : "Play", systemImage: deck.playing ? "pause.fill" : "play.fill")
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
        ZStack(alignment: .topLeading) {
            Color.black
            if let preview = deck.waveformPreview, !preview.points.isEmpty {
                RGBDeckWaveform(
                    points: preview.points,
                    durationBeats: deck.durationBeats,
                    playheadBeat: playheadBeat,
                    viewport: renderingViewport
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
                                let direction = reversesHorizontalScroll ? -1.0 : 1.0
                                viewport = renderingViewport.panned(
                                    byPixels: deltaX * direction,
                                    width: proxy.size.width
                                )
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

    private func displayedPlayheadBeat(at date: Date) -> Double {
        if let scrubProgress {
            return scrubProgress * Double(max(1, deck.durationBeats))
        }
        guard let visualClock,
              visualClock.trackLoadID == deck.trackLoadID,
              visualClock.durationMillis > 0 else {
            return Double(deck.beat)
        }
        let progress = visualClock.positionMillis(at: date)
            / Double(visualClock.durationMillis)
        return min(
            Double(max(1, deck.durationBeats)),
            max(0, progress * Double(max(1, deck.durationBeats)))
        )
    }

    private func seekProgress(
        atX x: Double,
        width: Double,
        renderingViewport: LumiWaveformViewport
    ) -> Double {
        let beat = renderingViewport.beat(atX: x, width: width)
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
                .font(.system(size: 9, weight: .bold))
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

    private var activePhraseName: String {
        guard let phraseIndex = deck.phraseIndex,
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
    let durationBeats: UInt64
    let playheadBeat: Double
    let viewport: LumiWaveformViewport

    var body: some View {
        Canvas { context, size in
            drawBeatGrid(context: &context, size: size)
            drawWaveform(context: &context, size: size)
            drawPlayhead(context: &context, size: size)
        }
    }

    private func drawBeatGrid(context: inout GraphicsContext, size: CGSize) {
        let maximumLines = max(1, Int(size.width / 4))
        let beatStride = max(1, Int(ceil(viewport.visibleBeats / Double(maximumLines))))
        let firstBeat = Int(floor(viewport.startBeat / Double(beatStride))) * beatStride
        let lastBeat = Int(ceil(viewport.endBeat))
        for beat in Swift.stride(from: firstBeat, through: lastBeat, by: beatStride) {
            let x = viewport.x(forBeat: Double(beat), width: size.width)
            guard x >= 0, x <= size.width else { continue }
            let isBar = beat.isMultiple(of: Int(viewport.beatsPerBar))
            var path = Path()
            path.move(to: CGPoint(x: x, y: 0))
            path.addLine(to: CGPoint(x: x, y: size.height))
            context.stroke(
                path,
                with: .color(Color.white.opacity(isBar ? 0.24 : 0.09)),
                lineWidth: isBar ? 1.1 : 0.6
            )
        }
    }

    private func drawWaveform(context: inout GraphicsContext, size: CGSize) {
        guard !points.isEmpty else { return }
        let center = Double(size.height) / 2
        let maximumAmplitude = Double(size.height) * 0.43
        let width = max(1, Int(size.width.rounded(.up)))
        for pixel in 0..<width {
            let fraction = Double(pixel) / Double(max(1, width - 1))
            let beat = viewport.startBeat + fraction * viewport.visibleBeats
            let trackProgress = beat / Double(max(1, durationBeats))
            let position = min(max(0, trackProgress), 1) * Double(max(0, points.count - 1))
            let lower = Int(position.rounded(.down))
            let upper = min(points.count - 1, lower + 1)
            let sampleFraction = position - Double(lower)
            let a = points[lower]
            let b = points[upper]
            func mix(_ lhs: UInt8, _ rhs: UInt8) -> Double {
                (Double(lhs) + (Double(rhs) - Double(lhs)) * sampleFraction) / 31
            }
            let low = mix(a.low, b.low)
            let mid = mix(a.mid, b.mid)
            let high = mix(a.high, b.high)
            let peak = max(low, max(mid, high))
            guard peak > 0.000_1 else { continue }
            let amplitude = pow(peak, 0.58) * maximumAmplitude
            let red = pow(high / peak, 0.72)
            let green = pow(mid / peak, 0.72)
            let blue = pow(low / peak, 0.72)
            let x = Double(pixel)
            var path = Path()
            path.move(to: CGPoint(x: x, y: center - amplitude))
            path.addLine(to: CGPoint(x: x, y: center + amplitude))
            context.stroke(
                path,
                with: .color(Color(red: red, green: green, blue: blue).opacity(0.98)),
                lineWidth: 1
            )
        }
    }

    private func drawPlayhead(context: inout GraphicsContext, size: CGSize) {
        guard playheadBeat >= viewport.startBeat, playheadBeat <= viewport.endBeat else { return }
        let x = viewport.x(forBeat: playheadBeat, width: size.width)
        var path = Path()
        path.move(to: CGPoint(x: x, y: 0))
        path.addLine(to: CGPoint(x: x, y: size.height))
        context.stroke(path, with: .color(.white), lineWidth: 2)
        context.fill(
            Path(CGRect(x: x - 3, y: 0, width: 6, height: 7)),
            with: .color(.white)
        )
    }
}
