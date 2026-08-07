import AppKit
import LumiDesignSystem
import QuartzCore
import SwiftUI

struct LiveDeckSurface<Details: View>: View {
    let deck: DeckSnapshot
    let isMaster: Bool
    let plan: PlanSnapshot?
    let musicalKey: String
    let isLocalPlayback: Bool
    let visualClock: LocalPlaybackVisualClockSnapshot?
    let waveformOverride: DeckWaveformPreviewSnapshot?
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
        waveformOverride: DeckWaveformPreviewSnapshot? = nil,
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
        self.waveformOverride = waveformOverride
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
        let playheadBeat = Double(deck.beat)
        let renderingViewport = renderingViewport(for: playheadBeat)
        return VStack(alignment: .leading, spacing: 0) {
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
            if let preview = waveformPreview, !preview.points.isEmpty {
                RGBDeckWaveform(
                    points: preview.points,
                    channelMaximum: preview.source == "localLibraryDetail" ? 255 : 31,
                    waveformID: deck.trackLoadID,
                    durationBeats: deck.durationBeats,
                    playheadBeat: playheadBeat,
                    viewport: isMaster && usesLiveViewport ? viewport : renderingViewport,
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
        return SynchronizedPlanTimelineLayerView(
            segments: deck.phrases.map { phrase in
                PlanTimelineLayerSegment(
                    id: phrase.index,
                    startBeat: Double(phrase.startBeat),
                    endBeat: Double(phrase.endBeat),
                    title: (cue(for: phrase)?.locked == true ? "◆ " : "")
                        + phraseDisplayName(phrase),
                    detail: nil,
                    footer: nil,
                    color: phraseLayerColor(phrase.roleID ?? phrase.kind),
                    opacity: phrase.index < (activePhraseIndex ?? 0) ? 0.48 : 1,
                    emphasis: phrase.index == selectedPhraseIndex
                        ? .selected
                        : phrase.index == activePhraseIndex ? .active : .normal
                )
            },
            motion: timelineMotion(
                playheadBeat: playheadBeat,
                renderingViewport: renderingViewport
            ),
            style: .phrases,
            onSelect: onSelectPhrase
        )
        .frame(height: 28)
        .padding(.horizontal, LumiSpacing.small)
        .padding(.bottom, LumiSpacing.small)
        .accessibilityLabel("Synchronized phrase timeline")
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
        SynchronizedPlanTimelineLayerView(
            segments: items.compactMap { item in
                guard let phrase = phrase(for: item) else { return nil }
                let footer = if let bank = item.bankNumber, let slot = item.slotNumber {
                    "BANK \(bank) · LOOP \(slot)"
                } else if item.holdsCurrentLook {
                    "NO MIDI CHANGE"
                } else {
                    ""
                }
                return PlanTimelineLayerSegment(
                    id: item.phraseIndex,
                    startBeat: Double(phrase.startBeat),
                    endBeat: Double(phrase.endBeat),
                    title: autoloopStatusLabel(item.status),
                    detail: "\(item.phraseName.uppercased())\n\(item.autoloopName)",
                    footer: footer,
                    color: autoloopLayerColor(item.status),
                    opacity: item.status == .completed ? 0.5 : 1,
                    emphasis: item.phraseIndex == selectedPhraseIndex ? .selected : .normal
                )
            },
            motion: timelineMotion(
                playheadBeat: playheadBeat,
                renderingViewport: renderingViewport
            ),
            style: .autoloops,
            onSelect: onSelectPhrase
        )
        .frame(height: 82)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.compact)
                .strokeBorder(Color.white.opacity(0.1), lineWidth: 1)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Synchronized AutoLoop plan timeline")
    }

    private func timelineMotion(
        playheadBeat: Double,
        renderingViewport: LumiWaveformViewport
    ) -> LiveWaveformMotionPlan {
        let motionViewport = isMaster && usesLiveViewport ? viewport : renderingViewport
        return LiveWaveformMotionPlan(
            waveformID: deck.trackLoadID,
            totalBeats: Double(max(1, deck.durationBeats)),
            viewportStartBeat: motionViewport.startBeat,
            visibleBeats: motionViewport.visibleBeats,
            followsLiveViewport: isMaster && usesLiveViewport,
            fallbackPlayheadBeat: playheadBeat,
            visualClock: scrubProgress == nil ? visualClock : nil,
            beatsPerBar: motionViewport.beatsPerBar
        )
    }

    private func autoloopStatusLabel(_ status: PlannedAutoloopStatus) -> String {
        switch status {
        case .active: "ACTIVE"
        case .next: "NEXT"
        case .planned: "PLANNED"
        case .completed: "DONE"
        }
    }

    private func autoloopLayerColor(
        _ status: PlannedAutoloopStatus
    ) -> PlanTimelineLayerColor {
        switch status {
        case .active: .init(red: 0.96, green: 0.18, blue: 0.22)
        case .next: .init(red: 0.24, green: 0.72, blue: 0.96)
        case .planned: .init(red: 0.72, green: 0.74, blue: 0.78)
        case .completed: .init(red: 0.20, green: 0.76, blue: 0.45)
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

    private func phrase(for item: PlannedAutoloopPresentation) -> DeckPhraseSnapshot? {
        deck.phrases.first(where: { $0.index == item.phraseIndex })
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

    private func phraseLayerColor(_ role: String) -> PlanTimelineLayerColor {
        switch role {
        case "intro-outro", "intro", "outro": .init(red: 0.25, green: 0.55, blue: 0.95)
        case "bridge": .init(red: 0.37, green: 0.42, blue: 0.78)
        case "breakdown-1", "breakdown-2", "breakdown-3", "breakdown":
            .init(red: 0.48, green: 0.28, blue: 0.83)
        case "synth": .init(red: 0.82, green: 0.24, blue: 0.72)
        case "pre-drop": .init(red: 0.95, green: 0.46, blue: 0.20)
        case "buildup-1", "buildup-2", "buildup-3", "build":
            .init(red: 0.96, green: 0.66, blue: 0.12)
        case "drop": .init(red: 0.92, green: 0.20, blue: 0.26)
        default: .init(red: 0.20, green: 0.68, blue: 0.60)
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
    let playheadBeat: Double
    let viewport: LumiWaveformViewport
    let visualClock: LocalPlaybackVisualClockSnapshot?
    let followsLiveViewport: Bool
    @State private var rasterImage: CGImage?

    init(
        points: [DeckWaveformPointSnapshot],
        channelMaximum: Double,
        waveformID: UInt64,
        durationBeats: UInt64,
        playheadBeat: Double,
        viewport: LumiWaveformViewport,
        visualClock: LocalPlaybackVisualClockSnapshot?,
        followsLiveViewport: Bool
    ) {
        self.points = points
        self.channelMaximum = channelMaximum
        self.waveformID = waveformID
        self.durationBeats = durationBeats
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
                    channelMaximum: channelMaximum
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
            beatsPerBar: viewport.beatsPerBar
        )
    }

    private var rasterKey: WaveformRasterKey {
        WaveformRasterKey(
            waveformID: waveformID,
            durationBeats: durationBeats,
            visibleBeats: viewport.visibleBeats,
            pointCount: points.count,
            channelMaximum: channelMaximum
        )
    }

    nonisolated private static func makeRasterImage(
        points: [DeckWaveformPointSnapshot],
        width: Int,
        channelMaximum: Double
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
            let position = Double(pixel) / Double(max(1, width - 1))
                * Double(max(0, points.count - 1))
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
        return context.makeImage()
    }

}

struct LiveWaveformMotionPlan: Equatable {
    let waveformID: UInt64
    let totalBeats: Double
    let viewportStartBeat: Double
    let visibleBeats: Double
    let followsLiveViewport: Bool
    let fallbackPlayheadBeat: Double
    let visualClock: LocalPlaybackVisualClockSnapshot?
    let beatsPerBar: UInt8

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
            fallbackPlayheadBeat: hasAuthoritativeClock ? nil : fallbackPlayheadBeat
        )
    }

    func playheadBeat(at date: Date) -> Double {
        guard let visualClock,
              visualClock.trackLoadID == waveformID,
              visualClock.durationMillis > 0 else {
            return min(totalBeats, max(0, fallbackPlayheadBeat))
        }
        let progress = visualClock.positionMillis(at: date)
            / Double(visualClock.durationMillis)
        return min(totalBeats, max(0, progress * totalBeats))
    }

    func startBeat(for playheadBeat: Double) -> Double {
        guard followsLiveViewport else { return viewportStartBeat }
        let maximumStart = max(0, totalBeats - visibleBeats)
        return min(
            maximumStart,
            max(0, playheadBeat - visibleBeats * LiveDeckViewportPolicy.playheadFraction)
        )
    }

    func secondsPerBeat() -> Double? {
        guard let visualClock,
              visualClock.trackLoadID == waveformID,
              visualClock.durationMillis > 0,
              visualClock.playing else {
            return nil
        }
        return Double(visualClock.durationMillis) / 1_000 / totalBeats
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
        let fallbackPlayheadBeat: Double?
    }
}

struct LiveBeatGridPlan: Equatable {
    let beatIndices: [Int]
    let barBeatIndices: [Int]

    init(totalBeats: Double, beatsPerBar: UInt8) {
        beatIndices = Array(0...Int(max(1, totalBeats)))
        let barLength = Int(max(1, beatsPerBar))
        barBeatIndices = beatIndices.filter { $0.isMultiple(of: barLength) }
    }
}

private struct WaveformRasterKey: Hashable {
    let waveformID: UInt64
    let durationBeats: UInt64
    let visibleBeats: Double
    let pointCount: Int
    let channelMaximum: Double
}

private struct PlanTimelineLayerColor: Equatable {
    let red: CGFloat
    let green: CGFloat
    let blue: CGFloat

    var nsColor: NSColor {
        NSColor(calibratedRed: red, green: green, blue: blue, alpha: 1)
    }
}

private enum PlanTimelineLayerEmphasis: Equatable {
    case normal
    case active
    case selected
}

private struct PlanTimelineLayerSegment: Equatable, Identifiable {
    let id: UInt64
    let startBeat: Double
    let endBeat: Double
    let title: String
    let detail: String?
    let footer: String?
    let color: PlanTimelineLayerColor
    let opacity: Double
    let emphasis: PlanTimelineLayerEmphasis
}

private enum PlanTimelineLayerStyle: Equatable {
    case phrases
    case autoloops

    var showsPlayhead: Bool { self == .autoloops }
}

private struct SynchronizedPlanTimelineLayerView: NSViewRepresentable {
    let segments: [PlanTimelineLayerSegment]
    let motion: LiveWaveformMotionPlan
    let style: PlanTimelineLayerStyle
    let onSelect: (UInt64) -> Void

    func makeNSView(context: Context) -> SynchronizedPlanTimelineHostView {
        SynchronizedPlanTimelineHostView()
    }

    func updateNSView(_ nsView: SynchronizedPlanTimelineHostView, context: Context) {
        nsView.update(
            segments: segments,
            motion: motion,
            style: style,
            onSelect: onSelect
        )
    }
}

private final class SynchronizedPlanTimelineHostView: NSView {
    private let contentLayer = CALayer()
    private let playheadLayer = CALayer()
    private let playheadCapLayer = CALayer()
    private var segments: [PlanTimelineLayerSegment] = []
    private var motion: LiveWaveformMotionPlan?
    private var style = PlanTimelineLayerStyle.phrases
    private var onSelect: ((UInt64) -> Void)?
    private var animationIdentity: LiveWaveformMotionPlan.AnimationIdentity?
    private var appliedBoundsSize = CGSize.zero

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.masksToBounds = true
        layer?.backgroundColor = NSColor.black.withAlphaComponent(0.7).cgColor
        contentLayer.anchorPoint = .zero
        playheadLayer.anchorPoint = .zero
        playheadLayer.backgroundColor = NSColor(
            calibratedRed: 0.24,
            green: 0.72,
            blue: 0.96,
            alpha: 1
        ).cgColor
        playheadLayer.shadowColor = playheadLayer.backgroundColor
        playheadLayer.shadowOpacity = 0.8
        playheadLayer.shadowRadius = 3
        playheadCapLayer.anchorPoint = .zero
        playheadCapLayer.backgroundColor = playheadLayer.backgroundColor
        layer?.addSublayer(contentLayer)
        layer?.addSublayer(playheadLayer)
        layer?.addSublayer(playheadCapLayer)
        setAccessibilityElement(true)
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
        applyCurrentState(rebuildSegments: true, restartAnimation: true)
    }

    override func mouseDown(with event: NSEvent) {
        guard let motion else { return }
        let point = convert(event.locationInWindow, from: nil)
        let contentX = (contentLayer.presentation() ?? contentLayer).position.x
        let beat = Double((point.x - contentX) / max(1, bounds.width))
            * motion.visibleBeats
        guard let segment = segments.first(where: {
            beat >= $0.startBeat && beat < $0.endBeat
        }) else {
            return
        }
        onSelect?(segment.id)
    }

    override func accessibilityLabel() -> String? {
        style == .phrases
            ? "Synchronized phrase timeline"
            : "Synchronized AutoLoop plan timeline"
    }

    func update(
        segments: [PlanTimelineLayerSegment],
        motion: LiveWaveformMotionPlan,
        style: PlanTimelineLayerStyle,
        onSelect: @escaping (UInt64) -> Void
    ) {
        let segmentsChanged = self.segments != segments || self.style != style
        let motionChanged = animationIdentity != motion.animationIdentity
        self.segments = segments
        self.motion = motion
        self.style = style
        self.onSelect = onSelect
        if segmentsChanged || motionChanged {
            animationIdentity = motion.animationIdentity
            applyCurrentState(
                rebuildSegments: segmentsChanged,
                restartAnimation: true
            )
        }
    }

    private func applyCurrentState(
        rebuildSegments: Bool,
        restartAnimation: Bool
    ) {
        guard let motion, bounds.width > 0, bounds.height > 0 else { return }
        appliedBoundsSize = bounds.size
        if restartAnimation {
            contentLayer.removeAllAnimations()
            playheadLayer.removeAllAnimations()
            playheadCapLayer.removeAllAnimations()
        }

        let width = bounds.width
        let height = bounds.height
        let fullTrackWidth = width * CGFloat(motion.totalBeats / motion.visibleBeats)
        let currentBeat = motion.playheadBeat(at: Date())
        let currentStartBeat = motion.startBeat(for: currentBeat)
        let contentX = -width * CGFloat(currentStartBeat / motion.visibleBeats)
        let playheadX = width * CGFloat((currentBeat - currentStartBeat) / motion.visibleBeats)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        contentLayer.bounds = CGRect(x: 0, y: 0, width: fullTrackWidth, height: height)
        contentLayer.position = CGPoint(x: contentX, y: 0)
        if rebuildSegments || contentLayer.sublayers?.isEmpty != false {
            rebuildSegmentLayers(
                motion: motion,
                fullTrackWidth: fullTrackWidth,
                height: height
            )
        }
        configurePlayhead(x: playheadX, height: height)
        CATransaction.commit()

        guard restartAnimation,
              let secondsPerBeat = motion.secondsPerBeat(),
              currentBeat < motion.totalBeats else {
            return
        }
        let duration = max(0.01, (motion.totalBeats - currentBeat) * secondsPerBeat)
        animateContent(
            motion: motion,
            currentBeat: currentBeat,
            width: width,
            duration: duration
        )
        if style.showsPlayhead {
            animatePlayhead(
                motion: motion,
                currentBeat: currentBeat,
                width: width,
                duration: duration
            )
        }
    }

    private func rebuildSegmentLayers(
        motion: LiveWaveformMotionPlan,
        fullTrackWidth: CGFloat,
        height: CGFloat
    ) {
        contentLayer.sublayers = nil
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        for segment in segments {
            let x = fullTrackWidth * CGFloat(segment.startBeat / motion.totalBeats)
            let width = max(
                1,
                fullTrackWidth * CGFloat(
                    (segment.endBeat - segment.startBeat) / motion.totalBeats
                ) - 1
            )
            let block = CALayer()
            block.frame = CGRect(x: x, y: 0, width: width, height: height)
            block.backgroundColor = segment.color.nsColor
                .withAlphaComponent(style == .phrases ? 0.92 : 0.11)
                .cgColor
            block.opacity = Float(segment.opacity)
            block.masksToBounds = true
            block.borderColor = borderColor(for: segment).cgColor
            block.borderWidth = borderWidth(for: segment)

            if style == .autoloops {
                let statusLine = CALayer()
                statusLine.backgroundColor = segment.color.nsColor.cgColor
                statusLine.frame = CGRect(x: 0, y: height - 3, width: width, height: 3)
                block.addSublayer(statusLine)
            }

            let textLayer = CATextLayer()
            textLayer.contentsScale = scale
            textLayer.frame = CGRect(
                x: style == .phrases ? 4 : 6,
                y: style == .phrases ? 5 : 7,
                width: max(0, width - (style == .phrases ? 8 : 12)),
                height: max(0, height - (style == .phrases ? 10 : 14))
            )
            textLayer.alignmentMode = style == .phrases ? .center : .left
            textLayer.truncationMode = .end
            textLayer.isWrapped = true
            textLayer.string = segmentText(segment, width: width)
            block.addSublayer(textLayer)
            contentLayer.addSublayer(block)
        }
    }

    private func segmentText(
        _ segment: PlanTimelineLayerSegment,
        width: CGFloat
    ) -> NSAttributedString {
        let result = NSMutableAttributedString()
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = style == .phrases ? .center : .left
        paragraph.lineBreakMode = .byTruncatingTail
        let titleColor = style == .phrases
            ? NSColor.white
            : segment.color.nsColor
        result.append(NSAttributedString(
            string: segment.title,
            attributes: [
                .font: NSFont.systemFont(
                    ofSize: style == .phrases ? 9 : 8,
                    weight: .semibold
                ),
                .foregroundColor: titleColor,
                .paragraphStyle: paragraph
            ]
        ))
        if style == .autoloops, width >= 44, let detail = segment.detail {
            result.append(NSAttributedString(
                string: "\n\(detail)",
                attributes: [
                    .font: NSFont.systemFont(ofSize: 9, weight: .semibold),
                    .foregroundColor: NSColor.white,
                    .paragraphStyle: paragraph
                ]
            ))
        }
        if style == .autoloops, width >= 100,
           let footer = segment.footer, !footer.isEmpty {
            result.append(NSAttributedString(
                string: "\n\(footer)",
                attributes: [
                    .font: NSFont.monospacedSystemFont(ofSize: 7, weight: .regular),
                    .foregroundColor: NSColor.white.withAlphaComponent(0.44),
                    .paragraphStyle: paragraph
                ]
            ))
        }
        return result
    }

    private func borderColor(for segment: PlanTimelineLayerSegment) -> NSColor {
        switch segment.emphasis {
        case .selected:
            NSColor(calibratedRed: 0.24, green: 0.72, blue: 0.96, alpha: 1)
        case .active:
            .white
        case .normal:
            NSColor.white.withAlphaComponent(0.1)
        }
    }

    private func borderWidth(for segment: PlanTimelineLayerSegment) -> CGFloat {
        switch segment.emphasis {
        case .selected: 2
        case .active: 2
        case .normal: 1
        }
    }

    private func configurePlayhead(x: CGFloat, height: CGFloat) {
        let hidden = !style.showsPlayhead
        playheadLayer.isHidden = hidden
        playheadCapLayer.isHidden = hidden
        playheadLayer.bounds = CGRect(x: 0, y: 0, width: 2, height: height)
        playheadLayer.position = CGPoint(x: x - 1, y: 0)
        playheadCapLayer.bounds = CGRect(x: 0, y: 0, width: 8, height: 3)
        playheadCapLayer.position = CGPoint(x: x - 4, y: height - 3)
    }

    private func animateContent(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double,
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
            currentBeat: currentBeat,
            totalBeats: motion.totalBeats,
            duration: duration
        )
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        contentLayer.position.x = values.last?.doubleValue ?? contentLayer.position.x
        CATransaction.commit()
        contentLayer.add(animation, forKey: "lumi.plan.content.motion")
    }

    private func animatePlayhead(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double,
        width: CGFloat,
        duration: TimeInterval
    ) {
        let keyBeats = animationKeyBeats(motion: motion, currentBeat: currentBeat)
        let centers = keyBeats.map { beat in
            let startBeat = motion.startBeat(for: beat)
            return width * CGFloat((beat - startBeat) / motion.visibleBeats)
        }
        let lineValues = centers.map { NSNumber(value: Double($0 - 1)) }
        let capValues = centers.map { NSNumber(value: Double($0 - 4)) }
        let lineAnimation = keyframeAnimation(
            keyPath: "position.x",
            values: lineValues,
            keyBeats: keyBeats,
            currentBeat: currentBeat,
            totalBeats: motion.totalBeats,
            duration: duration
        )
        let capAnimation = keyframeAnimation(
            keyPath: "position.x",
            values: capValues,
            keyBeats: keyBeats,
            currentBeat: currentBeat,
            totalBeats: motion.totalBeats,
            duration: duration
        )
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playheadLayer.position.x = lineValues.last?.doubleValue ?? playheadLayer.position.x
        playheadCapLayer.position.x = capValues.last?.doubleValue ?? playheadCapLayer.position.x
        CATransaction.commit()
        playheadLayer.add(lineAnimation, forKey: "lumi.plan.playhead.motion")
        playheadCapLayer.add(capAnimation, forKey: "lumi.plan.playhead.cap.motion")
    }

    private func animationKeyBeats(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double
    ) -> [Double] {
        guard motion.followsLiveViewport else {
            return [currentBeat, motion.totalBeats]
        }
        let leadingBeat = motion.visibleBeats * LiveDeckViewportPolicy.playheadFraction
        let trailingBeat = max(
            leadingBeat,
            motion.totalBeats - motion.visibleBeats
                * (1 - LiveDeckViewportPolicy.playheadFraction)
        )
        return [currentBeat, leadingBeat, trailingBeat, motion.totalBeats]
            .filter { $0 >= currentBeat && $0 <= motion.totalBeats }
            .reduce(into: [Double]()) { beats, beat in
                if beats.last.map({ abs($0 - beat) > 0.000_1 }) ?? true {
                    beats.append(beat)
                }
            }
    }

    private func keyframeAnimation(
        keyPath: String,
        values: [NSNumber],
        keyBeats: [Double],
        currentBeat: Double,
        totalBeats: Double,
        duration: TimeInterval
    ) -> CAKeyframeAnimation {
        let remainingBeats = max(0.000_1, totalBeats - currentBeat)
        let animation = CAKeyframeAnimation(keyPath: keyPath)
        animation.values = values
        animation.keyTimes = keyBeats.map {
            NSNumber(value: ($0 - currentBeat) / remainingBeats)
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

private struct RGBWaveformLayerView: NSViewRepresentable {
    let rasterImage: CGImage
    let motion: LiveWaveformMotionPlan
    let viewportWidth: CGFloat

    func makeNSView(context: Context) -> RGBWaveformLayerHostView {
        RGBWaveformLayerHostView()
    }

    func updateNSView(_ nsView: RGBWaveformLayerHostView, context: Context) {
        nsView.update(
            rasterImage: rasterImage,
            motion: motion,
            viewportWidth: viewportWidth
        )
    }
}

private final class RGBWaveformLayerHostView: NSView {
    private let waveformLayer = CALayer()
    private let beatGridLayer = CAShapeLayer()
    private let barMarkerLayer = CAShapeLayer()
    private let playheadLayer = CALayer()
    private let playheadCapLayer = CALayer()
    private var rasterImage: CGImage?
    private var motion: LiveWaveformMotionPlan?
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
        beatGridLayer.fillColor = nil
        beatGridLayer.strokeColor = NSColor(white: 0.82, alpha: 0.46).cgColor
        beatGridLayer.lineWidth = 0.5
        barMarkerLayer.fillColor = NSColor(
            calibratedRed: 0.96,
            green: 0.14,
            blue: 0.18,
            alpha: 0.96
        ).cgColor
        barMarkerLayer.strokeColor = nil
        waveformLayer.addSublayer(beatGridLayer)
        waveformLayer.addSublayer(barMarkerLayer)
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
        viewportWidth: CGFloat
    ) {
        let imageChanged = self.rasterImage !== rasterImage
        let motionChanged = animationIdentity != motion.animationIdentity
        let widthChanged = abs(self.viewportWidth - viewportWidth) > 0.5
        self.rasterImage = rasterImage
        self.motion = motion
        self.viewportWidth = viewportWidth
        if imageChanged {
            waveformLayer.contents = rasterImage
        }
        if imageChanged || motionChanged || widthChanged {
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
        configureBeatGrid(
            totalBeats: motion.totalBeats,
            beatsPerBar: motion.beatsPerBar,
            width: fullTrackWidth,
            height: height
        )
        playheadLayer.bounds = CGRect(x: 0, y: 0, width: 2, height: height)
        playheadLayer.position = CGPoint(x: playheadX - 1, y: 0)
        playheadCapLayer.bounds = CGRect(x: 0, y: 0, width: 6, height: 7)
        playheadCapLayer.position = CGPoint(x: playheadX - 3, y: 0)
        CATransaction.commit()

        guard restartAnimation,
              let secondsPerBeat = motion.secondsPerBeat(),
              currentBeat < motion.totalBeats else {
            return
        }
        let remainingDuration = max(0.01, (motion.totalBeats - currentBeat) * secondsPerBeat)
        animateWaveform(
            motion: motion,
            currentBeat: currentBeat,
            width: width,
            duration: remainingDuration
        )
        animatePlayhead(
            motion: motion,
            currentBeat: currentBeat,
            width: width,
            duration: remainingDuration
        )
    }

    private func animateWaveform(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double,
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
            currentBeat: currentBeat,
            totalBeats: motion.totalBeats,
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
            currentBeat: currentBeat,
            totalBeats: motion.totalBeats,
            duration: duration
        )
        let capAnimation = keyframeAnimation(
            keyPath: "position.x",
            values: capValues,
            keyBeats: keyBeats,
            currentBeat: currentBeat,
            totalBeats: motion.totalBeats,
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

    private func configureBeatGrid(
        totalBeats: Double,
        beatsPerBar: UInt8,
        width: CGFloat,
        height: CGFloat
    ) {
        let beatPath = CGMutablePath()
        let markerPath = CGMutablePath()
        let plan = LiveBeatGridPlan(
            totalBeats: totalBeats,
            beatsPerBar: beatsPerBar
        )
        for beat in plan.beatIndices {
            let x = CGFloat(Double(beat) / totalBeats) * width
            beatPath.move(to: CGPoint(x: x, y: 0))
            beatPath.addLine(to: CGPoint(x: x, y: height))
        }
        for beat in plan.barBeatIndices {
            let x = CGFloat(Double(beat) / totalBeats) * width
            let halfWidth: CGFloat = 3.5
            let markerDepth: CGFloat = 5
            markerPath.move(to: CGPoint(x: x - halfWidth, y: 0))
            markerPath.addLine(to: CGPoint(x: x + halfWidth, y: 0))
            markerPath.addLine(to: CGPoint(x: x, y: markerDepth))
            markerPath.closeSubpath()
            markerPath.move(to: CGPoint(x: x - halfWidth, y: height))
            markerPath.addLine(to: CGPoint(x: x + halfWidth, y: height))
            markerPath.addLine(to: CGPoint(x: x, y: height - markerDepth))
            markerPath.closeSubpath()
        }
        beatGridLayer.frame = CGRect(x: 0, y: 0, width: width, height: height)
        beatGridLayer.path = beatPath
        barMarkerLayer.frame = CGRect(x: 0, y: 0, width: width, height: height)
        barMarkerLayer.path = markerPath
    }

    private func animationKeyBeats(
        motion: LiveWaveformMotionPlan,
        currentBeat: Double
    ) -> [Double] {
        guard motion.followsLiveViewport else {
            return [currentBeat, motion.totalBeats]
        }
        let leadingBeat = motion.visibleBeats * LiveDeckViewportPolicy.playheadFraction
        let trailingBeat = max(
            leadingBeat,
            motion.totalBeats - motion.visibleBeats
                * (1 - LiveDeckViewportPolicy.playheadFraction)
        )
        return [currentBeat, leadingBeat, trailingBeat, motion.totalBeats]
            .filter { $0 >= currentBeat && $0 <= motion.totalBeats }
            .reduce(into: [Double]()) { beats, beat in
                if beats.last.map({ abs($0 - beat) > 0.000_1 }) ?? true {
                    beats.append(beat)
                }
            }
    }

    private func keyframeAnimation(
        keyPath: String,
        values: [NSNumber],
        keyBeats: [Double],
        currentBeat: Double,
        totalBeats: Double,
        duration: TimeInterval
    ) -> CAKeyframeAnimation {
        let remainingBeats = max(0.000_1, totalBeats - currentBeat)
        let animation = CAKeyframeAnimation(keyPath: keyPath)
        animation.values = values
        animation.keyTimes = keyBeats.map {
            NSNumber(value: ($0 - currentBeat) / remainingBeats)
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
