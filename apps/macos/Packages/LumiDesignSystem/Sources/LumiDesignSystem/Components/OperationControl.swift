import SwiftUI

public struct OperationControl: View {
    private let label: LocalizedStringKey
    private let systemImage: String
    private let isSelected: Bool
    private let isEnabled: Bool
    private let selectedColor: Color
    private let pulsesWhenSelected: Bool
    private let keyboardShortcut: KeyEquivalent?
    private let action: @MainActor () -> Void
    @State private var selectedEmphasis = 1.0

    public init(
        _ label: LocalizedStringKey,
        systemImage: String,
        isSelected: Bool = false,
        isEnabled: Bool = true,
        selectedColor: Color = LumiColor.accent,
        pulsesWhenSelected: Bool = false,
        keyboardShortcut: KeyEquivalent? = nil,
        action: @escaping @MainActor () -> Void
    ) {
        self.label = label
        self.systemImage = systemImage
        self.isSelected = isSelected
        self.isEnabled = isEnabled
        self.selectedColor = selectedColor
        self.pulsesWhenSelected = pulsesWhenSelected
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
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
                .frame(minHeight: LumiControlMetric.standardHeight)
                .padding(.horizontal, LumiSpacing.small)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(isSelected ? selectedColor : LumiColor.textPrimary)
        .background(isSelected ? selectedColor.opacity(0.14) : LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(
                    isSelected ? selectedColor.opacity(selectedEmphasis) : LumiColor.border,
                    lineWidth: isSelected ? 1.5 : 1
                )
        }
        .disabled(!isEnabled)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
        .task(id: isSelected && pulsesWhenSelected) {
            selectedEmphasis = 1
            guard isSelected, pulsesWhenSelected else { return }
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: .milliseconds(500))
                } catch {
                    return
                }
                withAnimation(.linear(duration: 0.12)) {
                    selectedEmphasis = selectedEmphasis == 1 ? 0.28 : 1
                }
            }
        }
    }
}
