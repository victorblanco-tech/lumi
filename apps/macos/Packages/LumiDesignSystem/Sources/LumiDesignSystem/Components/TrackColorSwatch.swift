import SwiftUI

/// Rekordbox uses a fixed eight-color track palette. The user-facing labels
/// can be renamed, but these RGB values remain the portable track identity
/// that Lumi receives from a OneLibrary USB source.
public enum LumiTrackColorPalette {
    public static func color(for rgb: UInt32) -> Color {
        Color(
            red: Double((rgb >> 16) & 0xff) / 255,
            green: Double((rgb >> 8) & 0xff) / 255,
            blue: Double(rgb & 0xff) / 255
        )
    }

    public static func name(for rgb: UInt32) -> String {
        switch rgb {
        case 0xff_33_cc: "Pink"
        case 0xff_33_33: "Red"
        case 0xff_8c_1a: "Orange"
        case 0xff_d6_00: "Yellow"
        case 0x32_d7_4b: "Green"
        case 0x32_d7_d5: "Aqua"
        case 0x32_80_ff: "Blue"
        case 0xaf_52_de: "Purple"
        default: String(format: "#%06X", rgb)
        }
    }

    public static func accessibilityLabel(for rgb: UInt32?) -> String {
        rgb.map { "Track color: \(name(for: $0))" } ?? "No track color"
    }
}

/// Compact, shared presentation for track color in library rows and decks.
/// An uncolored track remains visible as a hollow neutral ring instead of
/// being confused with a missing or broken UI element.
public struct LumiTrackColorSwatch: View {
    private let colorRGB: UInt32?
    private let diameter: CGFloat

    public init(colorRGB: UInt32?, diameter: CGFloat = 12) {
        self.colorRGB = colorRGB
        self.diameter = diameter
    }

    public var body: some View {
        Circle()
            .fill(colorRGB.map(LumiTrackColorPalette.color) ?? Color.clear)
            .overlay {
                Circle().stroke(
                    colorRGB == nil ? LumiColor.textSecondary.opacity(0.65) : Color.white.opacity(0.38),
                    lineWidth: 1
                )
            }
            .frame(width: diameter, height: diameter)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(LumiTrackColorPalette.accessibilityLabel(for: colorRGB))
    }
}
