import SwiftUI

public struct DeckCard: View {
    private let deckLabel: LocalizedStringKey
    private let title: String
    private let artist: String
    private let bpm: String
    private let musicalKey: String
    private let stateLabel: LocalizedStringKey
    private let state: LumiComponentState

    public init(
        deckLabel: LocalizedStringKey,
        title: String,
        artist: String,
        bpm: String,
        musicalKey: String,
        stateLabel: LocalizedStringKey,
        state: LumiComponentState
    ) {
        self.deckLabel = deckLabel
        self.title = title
        self.artist = artist
        self.bpm = bpm
        self.musicalKey = musicalKey
        self.stateLabel = stateLabel
        self.state = state
    }

    public var body: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    Text(deckLabel)
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(LumiColor.textSecondary)
                        .textCase(.uppercase)
                    Spacer()
                    StatusBadge(stateLabel, state: state)
                }

                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text(verbatim: title)
                        .font(LumiTypography.cardTitle)
                        .foregroundStyle(LumiColor.textPrimary)
                        .lineLimit(1)
                    Text(verbatim: artist)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                        .lineLimit(1)
                }

                HStack(spacing: LumiSpacing.large) {
                    metadata(value: bpm, label: "design.preview.bpm")
                    metadata(value: musicalKey, label: "design.preview.key")
                }
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func metadata(value: String, label: LocalizedStringKey) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text(label)
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
            Text(verbatim: value)
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(LumiColor.textPrimary)
        }
    }
}
