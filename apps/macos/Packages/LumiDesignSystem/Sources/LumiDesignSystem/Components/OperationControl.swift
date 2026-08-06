import SwiftUI

public struct OperationControl: View {
    private let label: LocalizedStringKey
    private let systemImage: String
    private let isSelected: Bool
    private let isEnabled: Bool
    private let keyboardShortcut: KeyEquivalent?
    private let action: @MainActor () -> Void

    public init(
        _ label: LocalizedStringKey,
        systemImage: String,
        isSelected: Bool = false,
        isEnabled: Bool = true,
        keyboardShortcut: KeyEquivalent? = nil,
        action: @escaping @MainActor () -> Void
    ) {
        self.label = label
        self.systemImage = systemImage
        self.isSelected = isSelected
        self.isEnabled = isEnabled
        self.keyboardShortcut = keyboardShortcut
        self.action = action
    }

    public var body: some View {
        Group {
            if let keyboardShortcut {
                controlButton.keyboardShortcut(keyboardShortcut, modifiers: [])
            } else {
                controlButton
            }
        }
    }

    private var controlButton: some View {
        Button(action: action) {
            Label(label, systemImage: systemImage)
                .font(LumiTypography.metadata.weight(.semibold))
                .frame(minHeight: LumiControlMetric.standardHeight)
                .padding(.horizontal, LumiSpacing.small)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(isSelected ? LumiColor.accent : LumiColor.textPrimary)
        .background(isSelected ? LumiColor.accent.opacity(0.14) : LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(isSelected ? LumiColor.accent : LumiColor.border, lineWidth: 1)
        }
        .disabled(!isEnabled)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
