import SwiftUI

public enum AppearancePreference: String, CaseIterable, Identifiable, Sendable {
    case dark
    case light
    case system

    public var id: Self { self }

    public var colorScheme: ColorScheme? {
        switch self {
        case .dark: .dark
        case .light: .light
        case .system: nil
        }
    }

    public var titleKey: LocalizedStringKey {
        switch self {
        case .dark: "preference.appearance.dark"
        case .light: "preference.appearance.light"
        case .system: "preference.appearance.system"
        }
    }
}
