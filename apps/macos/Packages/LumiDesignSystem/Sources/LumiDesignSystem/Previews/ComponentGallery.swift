import SwiftUI

public struct ComponentGallery: View {
    public init() {}

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                Text("design.preview.componentGallery")
                    .font(LumiTypography.screenTitle)

                HStack(spacing: LumiSpacing.small) {
                    ForEach(LumiComponentState.allCases, id: \.self) { state in
                        StatusBadge(state.titleKey, state: state)
                    }
                }

                DeckCard(
                    deckLabel: "design.preview.nextDeck",
                    title: "Midnight Circuit",
                    artist: "Lumi Demo",
                    bpm: "128.0",
                    musicalKey: "8A",
                    stateLabel: "design.state.ready",
                    state: .ready
                )

                PhraseRow(
                    phrase: "Breakdown",
                    range: "01:32–02:04",
                    scene: "Neon Pulse · Loop 3",
                    isLocked: true,
                    isSelected: true,
                    action: {}
                )

                ProviderStatus(
                    name: "design.preview.engineProvider",
                    detail: "127.0.0.1 · protocol v1",
                    stateLabel: "design.state.ready",
                    state: .ready
                )

                HStack(spacing: LumiSpacing.small) {
                    OperationControl("operation.arm", systemImage: "shield", action: {})
                    OperationControl("operation.start", systemImage: "play.fill", action: {})
                    OperationControl("operation.pause", systemImage: "pause.fill", action: {})
                    OperationControl("operation.off", systemImage: "stop.fill", action: {})
                }
            }
            .padding(LumiSpacing.xLarge)
        }
        .background(LumiColor.canvas)
    }
}

#Preview("Lumi Design System · Dark") {
    ComponentGallery()
        .preferredColorScheme(.dark)
}

#Preview("Lumi Design System · Light") {
    ComponentGallery()
        .preferredColorScheme(.light)
}
