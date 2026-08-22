import SwiftUI

#if canImport(AppKit)
import AppKit
#endif

/// The single visual contract for Lumi phrase roles.
///
/// Persisted 24-bit sRGB values override these defaults by stable role ID. All
/// phrase-aware screens receive the same value object, so a Settings change is
/// reflected without copying color switches into individual features.
public struct LumiPhraseColorPalette: Equatable, Sendable {
    public static let fallbackRGB: UInt32 = 0x33AD99

    public static let defaults = LumiPhraseColorPalette(roleColors: [
        "intro-outro": 0x408CF2,
        "bridge": 0x5E6BC7,
        "breakdown-1": 0x7A47D4,
        "breakdown-2": 0x7A47D4,
        "breakdown-3": 0x7A47D4,
        "synth": 0xD13DB8,
        "pre-drop": 0xF27433,
        "buildup-1": 0xF5A81F,
        "buildup-2": 0xF5A81F,
        "buildup-3": 0xF5A81F,
        "drop": 0xEB3342
    ])

    public let roleColors: [String: UInt32]

    public init(roleColors: [String: UInt32] = [:]) {
        self.roleColors = roleColors.reduce(into: [:]) { result, entry in
            guard entry.value <= 0xFF_FF_FF else { return }
            result[Self.normalized(entry.key)] = entry.value
        }
    }

    public func rgb(for roleID: String) -> UInt32 {
        let normalized = Self.normalized(roleID)
        if let configured = roleColors[normalized] { return configured }
        if let configured = Self.defaults.roleColors[normalized] { return configured }
        if normalized.hasPrefix("breakdown") {
            return roleColors["breakdown-1"]
                ?? Self.defaults.roleColors["breakdown-1"]
                ?? Self.fallbackRGB
        }
        if normalized.hasPrefix("buildup") {
            return roleColors["buildup-1"]
                ?? Self.defaults.roleColors["buildup-1"]
                ?? Self.fallbackRGB
        }
        return Self.fallbackRGB
    }

    public func color(for roleID: String) -> Color {
        let components = Self.components(rgb(for: roleID))
        return Color(red: components.red, green: components.green, blue: components.blue)
    }

    #if canImport(AppKit)
    public func nsColor(for roleID: String) -> NSColor {
        let components = Self.components(rgb(for: roleID))
        return NSColor(
            red: components.red,
            green: components.green,
            blue: components.blue,
            alpha: 1
        )
    }
    #endif

    private static func normalized(_ roleID: String) -> String {
        switch roleID.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "intro", "outro", "intro / outro": "intro-outro"
        case "breakdown": "breakdown-1"
        case "build", "buildup": "buildup-1"
        default: roleID.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        }
    }

    private static func components(_ rgb: UInt32) -> (red: Double, green: Double, blue: Double) {
        (
            Double((rgb >> 16) & 0xFF) / 255,
            Double((rgb >> 8) & 0xFF) / 255,
            Double(rgb & 0xFF) / 255
        )
    }
}
