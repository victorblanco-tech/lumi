import SwiftUI

public struct StatusBadge: View {
    private let label: LocalizedStringKey
    private let state: LumiComponentState

    public init(_ label: LocalizedStringKey, state: LumiComponentState) {
        self.label = label
        self.state = state
    }

    public var body: some View {
        Label(label, systemImage: state.systemImage)
            .font(LumiTypography.caption.weight(.medium))
            .foregroundStyle(state.color)
            .padding(.horizontal, LumiSpacing.small)
            .frame(minHeight: LumiControlMetric.compactHeight)
            .background(state.color.opacity(0.12))
            .clipShape(Capsule())
            .accessibilityElement(children: .combine)
    }
}
