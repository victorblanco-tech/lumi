import SwiftUI

public struct ProviderStatus: View {
    private let name: LocalizedStringKey
    private let detail: String
    private let stateLabel: LocalizedStringKey
    private let state: LumiComponentState

    public init(
        name: LocalizedStringKey,
        detail: String,
        stateLabel: LocalizedStringKey,
        state: LumiComponentState
    ) {
        self.name = name
        self.detail = detail
        self.stateLabel = stateLabel
        self.state = state
    }

    public var body: some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: "server.rack")
                .foregroundStyle(state.color)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(name)
                    .font(LumiTypography.body.weight(.medium))
                Text(verbatim: detail)
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .lineLimit(1)
            }
            Spacer()
            StatusBadge(stateLabel, state: state)
        }
        .accessibilityElement(children: .combine)
    }
}
