import SwiftUI

public enum KeyNotationPreference: String, CaseIterable, Identifiable, Sendable {
    case camelot
    case classic

    public var id: Self { self }

    public var titleKey: LocalizedStringKey {
        switch self {
        case .camelot: "preference.key.camelot"
        case .classic: "preference.key.classic"
        }
    }
}
