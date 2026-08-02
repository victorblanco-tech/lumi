import SwiftUI

public struct PhraseRow: View {
    private let phrase: String
    private let range: String
    private let scene: String
    private let isLocked: Bool
    private let isSelected: Bool
    private let action: @MainActor () -> Void

    public init(
        phrase: String,
        range: String,
        scene: String,
        isLocked: Bool,
        isSelected: Bool,
        action: @escaping @MainActor () -> Void
    ) {
        self.phrase = phrase
        self.range = range
        self.scene = scene
        self.isLocked = isLocked
        self.isSelected = isSelected
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: LumiSpacing.medium) {
                Image(systemName: isLocked ? "lock.fill" : "lock.open")
                    .foregroundStyle(isLocked ? LumiColor.accent : LumiColor.textSecondary)
                    .accessibilityLabel(Text(lockAccessibilityLabel))
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text(verbatim: phrase)
                        .font(LumiTypography.body.weight(.medium))
                    Text(verbatim: range)
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Text(verbatim: scene)
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(.horizontal, LumiSpacing.medium)
            .frame(minHeight: LumiControlMetric.prominentHeight)
            .background(isSelected ? LumiColor.accent.opacity(0.14) : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    private var lockAccessibilityLabel: LocalizedStringKey {
        isLocked ? "accessibility.locked" : "accessibility.unlocked"
    }
}
