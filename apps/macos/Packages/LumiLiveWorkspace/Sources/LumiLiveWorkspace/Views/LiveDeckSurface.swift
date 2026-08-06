import LumiDesignSystem
import SwiftUI

struct LiveDeckSurface<Details: View>: View {
    let deck: DeckSnapshot
    let isMaster: Bool
    let plan: PlanSnapshot?
    let musicalKey: String
    let isLocalPlayback: Bool
    let selectedPhraseIndex: UInt64?
    let onSelectPhrase: (UInt64) -> Void
    let onTogglePlayback: () -> Void
    let onStop: () -> Void
    let onSeek: (Double) -> Void
    let onMakeMaster: () -> Void
    private let details: Details

    init(
        deck: DeckSnapshot,
        isMaster: Bool,
        plan: PlanSnapshot?,
        musicalKey: String,
        isLocalPlayback: Bool,
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
        self.selectedPhraseIndex = selectedPhraseIndex
        self.onSelectPhrase = onSelectPhrase
        self.onTogglePlayback = onTogglePlayback
        self.onStop = onStop
        self.onSeek = onSeek
        self.onMakeMaster = onMakeMaster
        self.details = details()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            metadata
            if isLocalPlayback { transportControls }
            waveform
            phraseBand
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

    private var waveform: some View {
        ZStack(alignment: .topLeading) {
            Color.black
            if let preview = deck.waveformPreview, !preview.points.isEmpty {
                RGBDeckWaveform(
                    points: preview.points,
                    durationBeats: deck.durationBeats,
                    progress: playbackProgress
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
                    .gesture(
                        DragGesture(minimumDistance: 0)
                            .onEnded { value in
                                guard isLocalPlayback else { return }
                                onSeek(min(max(0, value.location.x / max(1, proxy.size.width)), 1))
                            }
                    )
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("RGB waveform for \(deck.title), beat \(deck.beat)")
    }

    private var phraseBand: some View {
        GeometryReader { proxy in
            HStack(spacing: 2) {
                ForEach(deck.phrases) { phrase in
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
                            width: phraseWidth(phrase, totalWidth: proxy.size.width),
                            height: 28
                        )
                        .background(phraseColor(phrase.roleID ?? phrase.kind))
                        .opacity(phrase.index < (deck.phraseIndex ?? 0) ? 0.48 : 1)
                        .overlay {
                            if phrase.index == selectedPhraseIndex {
                                Rectangle().strokeBorder(LumiColor.accent, lineWidth: 3)
                            } else if phrase.index == deck.phraseIndex {
                                Rectangle().strokeBorder(Color.white, lineWidth: 2)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .contentShape(Rectangle())
                    .accessibilityIdentifier("lumi.deck.\(deck.deckID).phrase.\(phrase.index)")
                }
            }
        }
        .frame(height: 28)
        .padding(.horizontal, LumiSpacing.small)
        .padding(.bottom, LumiSpacing.small)
        .accessibilityElement(children: .contain)
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

    private var playbackProgress: Double {
        guard deck.durationBeats > 0 else { return 0 }
        return min(1, Double(deck.beat) / Double(deck.durationBeats))
    }

    private func phraseWidth(_ phrase: DeckPhraseSnapshot, totalWidth: CGFloat) -> CGFloat {
        guard deck.durationBeats > 0 else { return 0 }
        let gaps = CGFloat(max(0, deck.phrases.count - 1)) * 2
        let available = max(0, totalWidth - gaps)
        let duration = phrase.endBeat - phrase.startBeat
        return available * CGFloat(Double(duration) / Double(deck.durationBeats))
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
    let progress: Double

    var body: some View {
        Canvas { context, size in
            drawBeatGrid(context: &context, size: size)
            drawWaveform(context: &context, size: size)
            drawPlayhead(context: &context, size: size)
        }
    }

    private func drawBeatGrid(context: inout GraphicsContext, size: CGSize) {
        let barCount = max(1, Int(ceil(Double(max(1, durationBeats)) / 4)))
        let visibleBarCapacity = max(1, Int(size.width / 5))
        let stride = max(1, Int(ceil(Double(barCount) / Double(visibleBarCapacity))))
        for bar in Swift.stride(from: 0, through: barCount, by: stride) {
            let x = size.width * CGFloat(bar) / CGFloat(barCount)
            var path = Path()
            path.move(to: CGPoint(x: x, y: 0))
            path.addLine(to: CGPoint(x: x, y: size.height))
            context.stroke(
                path,
                with: .color(Color.white.opacity(bar.isMultiple(of: 4) ? 0.24 : 0.09)),
                lineWidth: bar.isMultiple(of: 4) ? 1.1 : 0.6
            )
        }
    }

    private func drawWaveform(context: inout GraphicsContext, size: CGSize) {
        guard !points.isEmpty else { return }
        let center = Double(size.height) / 2
        let maximumAmplitude = Double(size.height) * 0.43
        let width = max(1, Int(size.width.rounded(.up)))
        for pixel in 0..<width {
            let progress = Double(pixel) / Double(max(1, width - 1))
            let position = progress * Double(max(0, points.count - 1))
            let lower = Int(position.rounded(.down))
            let upper = min(points.count - 1, lower + 1)
            let fraction = position - Double(lower)
            let a = points[lower]
            let b = points[upper]
            func mix(_ lhs: UInt8, _ rhs: UInt8) -> Double {
                (Double(lhs) + (Double(rhs) - Double(lhs)) * fraction) / 31
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
        let x = size.width * CGFloat(min(1, max(0, progress)))
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
