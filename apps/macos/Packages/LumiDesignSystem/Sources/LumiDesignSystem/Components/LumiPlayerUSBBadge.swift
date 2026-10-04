import SwiftUI

/// Presentation only: the resolver owns identity, not this badge or its color.
public struct LumiPlayerUSBBadge: View {
    let state: String
    let sourceName: String?
    let colorID: UInt8?

    public init(state: String, sourceName: String?, colorID: UInt8? = nil) {
        self.state = state
        self.sourceName = sourceName
        self.colorID = state == "trusted" ? colorID : nil
    }

    nonisolated public static func label(state: String, sourceName: String?) -> String {
        switch state {
        case "trusted": "USB · \(sourceName ?? "Identified")"
        case "unknown": "USB · Unknown source"
        case "conflict": "USB · Identity conflict"
        case "unavailable": "USB · Not identified"
        default: "USB · Identifying…"
        }
    }

    private var statusTint: Color {
        switch state {
        case "trusted": LumiColor.success
        case "unknown", "conflict": LumiColor.warning
        default: LumiColor.textSecondary
        }
    }

    /// Native media UI palette, not Rekordbox's track-color IDs.
    nonisolated public static func colorRGB(for id: UInt8?) -> UInt32? {
        switch id {
        case 1: 0xff3399
        case 2: 0xff3333
        case 3: 0xff9933
        case 4: 0xffdd33
        case 5: 0x33cc66
        case 6: 0x33cccc
        case 7: 0x3399ff
        case 8: 0x9966ff
        default: nil
        }
    }

    private var mediaTint: Color {
        guard let rgb = Self.colorRGB(for: colorID) else { return LumiColor.textSecondary }
        return Color(red: Double((rgb >> 16) & 255) / 255,
                     green: Double((rgb >> 8) & 255) / 255,
                     blue: Double(rgb & 255) / 255)
    }

    public var body: some View {
        HStack(spacing: 4) {
            Image(systemName: "externaldrive.fill").foregroundStyle(mediaTint)
            Text(Self.label(state: state, sourceName: sourceName))
                .foregroundStyle(state == "trusted" ? LumiColor.textSecondary : statusTint)
                .lineLimit(1)
                .truncationMode(.middle)
            Image(systemName: state == "trusted" ? "checkmark.circle.fill" : "questionmark.circle")
                .foregroundStyle(statusTint)
        }
            .font(LumiTypography.caption)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Self.label(state: state, sourceName: sourceName))
    }
}
