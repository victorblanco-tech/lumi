import SwiftUI

public struct InspectorField<Content: View>: View {
    private let label: LocalizedStringKey
    private let content: Content

    public init(
        _ label: LocalizedStringKey,
        @ViewBuilder content: () -> Content
    ) {
        self.label = label
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            Text(label)
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(LumiColor.textSecondary)
            content
                .frame(minHeight: LumiControlMetric.standardHeight)
        }
    }
}
