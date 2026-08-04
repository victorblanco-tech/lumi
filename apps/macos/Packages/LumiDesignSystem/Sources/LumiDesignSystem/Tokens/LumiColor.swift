import AppKit
import SwiftUI

public enum LumiColor {
    public static let canvas = adaptive(
        dark: NSColor(red: 0.018, green: 0.023, blue: 0.032, alpha: 1),
        light: NSColor(red: 0.965, green: 0.972, blue: 0.982, alpha: 1)
    )
    public static let surface = adaptive(
        dark: NSColor(red: 0.035, green: 0.045, blue: 0.060, alpha: 1),
        light: NSColor(red: 0.985, green: 0.989, blue: 0.995, alpha: 1)
    )
    public static let surfaceElevated = adaptive(
        dark: NSColor(red: 0.055, green: 0.070, blue: 0.095, alpha: 1),
        light: NSColor(red: 0.925, green: 0.938, blue: 0.958, alpha: 1)
    )
    public static let border = adaptive(
        dark: NSColor(white: 1, alpha: 0.14),
        light: NSColor(white: 0, alpha: 0.14)
    )
    public static let textPrimary = adaptive(
        dark: NSColor(red: 0.94, green: 0.97, blue: 1, alpha: 1),
        light: NSColor(red: 0.08, green: 0.10, blue: 0.14, alpha: 1)
    )
    public static let textSecondary = adaptive(
        dark: NSColor(red: 0.56, green: 0.64, blue: 0.73, alpha: 1),
        light: NSColor(red: 0.34, green: 0.39, blue: 0.47, alpha: 1)
    )
    public static let accent = Color(red: 0.25, green: 0.76, blue: 1)
    public static let success = Color.green
    public static let warning = Color.orange
    public static let destructive = Color.red

    private static func adaptive(dark: NSColor, light: NSColor) -> Color {
        Color(
            nsColor: NSColor(name: nil) { appearance in
                appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light
            }
        )
    }
}
