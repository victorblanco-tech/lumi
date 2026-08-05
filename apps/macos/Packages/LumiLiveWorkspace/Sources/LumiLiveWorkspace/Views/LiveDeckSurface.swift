import LumiDesignSystem
import SwiftUI

struct LiveDeckSurface<Details: View>: View {
    let deck: DeckSnapshot
    let isMaster: Bool
    let plan: PlanSnapshot?
    let musicalKey: String
    let selectedPhraseIndex: UInt64?
    let onSelectPhrase: (UInt64) -> Void
    private let details: Details

    init(
        deck: DeckSnapshot,
        isMaster: Bool,
        plan: PlanSnapshot?,
        musicalKey: String,
        selectedPhraseIndex: UInt64?,
        onSelectPhrase: @escaping (UInt64) -> Void,
        @ViewBuilder details: () -> Details
    ) {
        self.deck = deck
        self.isMaster = isMaster
        self.plan = plan
        self.musicalKey = musicalKey
        self.selectedPhraseIndex = selectedPhraseIndex
        self.onSelectPhrase = onSelectPhrase
        self.details = details()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            metadata
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
                        Text(verbatim: phrase.kind.capitalized)
                            .font(LumiTypography.caption.weight(.semibold))
                            .foregroundStyle(Color.white)
                            .lineLimit(1)
                            .frame(
                                width: phraseWidth(phrase, totalWidth: proxy.size.width),
                                height: 28
                            )
                            .background(phraseColor(phrase.kind))
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
        return phrase.kind.capitalized
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

    private func phraseColor(_ kind: String) -> Color {
        switch kind {
        case "intro", "outro": Color(red: 0.14, green: 0.40, blue: 0.66)
        case "verse": Color(red: 0.14, green: 0.48, blue: 0.35)
        case "breakdown": Color(red: 0.40, green: 0.25, blue: 0.62)
        case "build": Color(red: 0.16, green: 0.50, blue: 0.29)
        case "drop": Color(red: 0.64, green: 0.18, blue: 0.23)
        default: Color.gray
        }
    }
}

private struct RGBDeckWaveform: View {
    let points: [DeckWaveformPointSnapshot]
    let progress: Double

    var body: some View {
        Canvas { context, size in
            drawBeatGrid(context: &context, size: size)
            drawWaveform(context: &context, size: size)
            drawPlayhead(context: &context, size: size)
        }
    }

    private func drawBeatGrid(context: inout GraphicsContext, size: CGSize) {
        for index in 0...16 {
            let x = size.width * CGFloat(index) / 16
            var path = Path()
            path.move(to: CGPoint(x: x, y: 0))
            path.addLine(to: CGPoint(x: x, y: size.height))
            context.stroke(
                path,
                with: .color(Color.white.opacity(index.isMultiple(of: 4) ? 0.20 : 0.08)),
                lineWidth: 1
            )
        }
    }

    private func drawWaveform(context: inout GraphicsContext, size: CGSize) {
        guard !points.isEmpty else { return }
        let center = size.height / 2
        let usableHeight = size.height * 0.86
        let step = size.width / CGFloat(max(1, points.count - 1))
        for (index, point) in points.enumerated() {
            let low = Double(point.low) / 31
            let mid = Double(point.mid) / 31
            let high = Double(point.high) / 31
            let amplitude = CGFloat(max(low, max(mid, high))) * usableHeight / 2
            let x = CGFloat(index) * step
            var path = Path()
            path.move(to: CGPoint(x: x, y: center - amplitude))
            path.addLine(to: CGPoint(x: x, y: center + amplitude))
            context.stroke(
                path,
                with: .color(Color(red: high, green: mid, blue: low).opacity(0.96)),
                lineWidth: max(1, step)
            )
        }
    }

    private func drawPlayhead(context: inout GraphicsContext, size: CGSize) {
        let x = size.width * CGFloat(min(1, max(0, progress)))
        var path = Path()
        path.move(to: CGPoint(x: x, y: 0))
        path.addLine(to: CGPoint(x: x, y: size.height))
        context.stroke(path, with: .color(.white), lineWidth: 2)
    }
}
