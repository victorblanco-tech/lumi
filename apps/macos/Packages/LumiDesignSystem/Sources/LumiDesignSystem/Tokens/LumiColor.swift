import SwiftUI

#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

public enum LumiColor {
    public static let canvas = adaptive(
        dark: RGBA(red: 0.018, green: 0.023, blue: 0.032),
        light: RGBA(red: 0.965, green: 0.972, blue: 0.982)
    )
    public static let surface = adaptive(
        dark: RGBA(red: 0.035, green: 0.045, blue: 0.060),
        light: RGBA(red: 0.985, green: 0.989, blue: 0.995)
    )
    public static let surfaceElevated = adaptive(
        dark: RGBA(red: 0.055, green: 0.070, blue: 0.095),
        light: RGBA(red: 0.925, green: 0.938, blue: 0.958)
    )
    public static let border = adaptive(
        dark: RGBA(red: 1, green: 1, blue: 1, alpha: 0.14),
        light: RGBA(red: 0, green: 0, blue: 0, alpha: 0.14)
    )
    public static let textPrimary = adaptive(
        dark: RGBA(red: 0.94, green: 0.97, blue: 1),
        light: RGBA(red: 0.08, green: 0.10, blue: 0.14)
    )
    public static let textSecondary = adaptive(
        dark: RGBA(red: 0.56, green: 0.64, blue: 0.73),
        light: RGBA(red: 0.34, green: 0.39, blue: 0.47)
    )
    public static let accent = Color(red: 0.25, green: 0.76, blue: 1)
    public static let success = Color.green
    public static let warning = Color.orange
    public static let destructive = Color.red

    private struct RGBA {
        let red: CGFloat
        let green: CGFloat
        let blue: CGFloat
        var alpha: CGFloat = 1
    }

    private static func adaptive(dark: RGBA, light: RGBA) -> Color {
        #if canImport(AppKit)
        Color(
            nsColor: NSColor(name: nil) { appearance in
                let value = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
                    ? dark
                    : light
                return NSColor(
                    red: value.red,
                    green: value.green,
                    blue: value.blue,
                    alpha: value.alpha
                )
            }
        )
        #elseif canImport(UIKit)
        Color(
            uiColor: UIColor { traits in
                let value = traits.userInterfaceStyle == .dark ? dark : light
                return UIColor(
                    red: value.red,
                    green: value.green,
                    blue: value.blue,
                    alpha: value.alpha
                )
            }
        )
        #else
        Color(
            red: Double(dark.red),
            green: Double(dark.green),
            blue: Double(dark.blue),
            opacity: Double(dark.alpha)
        )
        #endif
    }
}
