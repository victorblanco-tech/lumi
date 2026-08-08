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

    /// Signed lighting-output compensation. Positive values send an AutoLoop
    /// early; negative values deliberately delay it.
    public var lightingTimingOffsetMillis: Int {
        didSet {
            let clampedValue = lightingTimingOffsetMillis.clamped(to: -250...250)
            if clampedValue != lightingTimingOffsetMillis {
                lightingTimingOffsetMillis = clampedValue
                return
            }
            userDefaults.set(
                clampedValue,
                forKey: PreferenceKey.lightingTimingOffsetMillis
            )
        }
    }

    private let userDefaults: UserDefaults

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
        appearance = userDefaults.string(forKey: PreferenceKey.appearance)
            .flatMap(AppearancePreference.init(rawValue:)) ?? .dark
        keyNotation = userDefaults.string(forKey: PreferenceKey.keyNotation)
            .flatMap(KeyNotationPreference.init(rawValue:)) ?? .camelot
        lightingTimingOffsetMillis = userDefaults
            .integer(forKey: PreferenceKey.lightingTimingOffsetMillis)
            .clamped(to: -250...250)
    }
}

private enum PreferenceKey {
    static let appearance = "nl.blancoservices.lumi.preference.appearance"
    static let keyNotation = "nl.blancoservices.lumi.preference.key-notation"
    static let lightingTimingOffsetMillis =
        "nl.blancoservices.lumi.preference.lighting-timing-offset-millis"
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
