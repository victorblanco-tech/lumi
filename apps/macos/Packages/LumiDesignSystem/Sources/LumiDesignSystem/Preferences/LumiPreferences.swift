import Foundation
import Observation

@Observable
@MainActor
public final class LumiPreferences {
    public var appearance: AppearancePreference {
        didSet {
            userDefaults.set(appearance.rawValue, forKey: PreferenceKey.appearance)
        }
    }

    public var keyNotation: KeyNotationPreference {
        didSet {
            userDefaults.set(keyNotation.rawValue, forKey: PreferenceKey.keyNotation)
        }
    }

    private let userDefaults: UserDefaults

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
        appearance = userDefaults.string(forKey: PreferenceKey.appearance)
            .flatMap(AppearancePreference.init(rawValue:)) ?? .dark
        keyNotation = userDefaults.string(forKey: PreferenceKey.keyNotation)
            .flatMap(KeyNotationPreference.init(rawValue:)) ?? .camelot
    }
}

private enum PreferenceKey {
    static let appearance = "nl.blancoservices.lumi.preference.appearance"
    static let keyNotation = "nl.blancoservices.lumi.preference.key-notation"
}
