import Foundation
import Observation

@Observable
@MainActor
public final class LumiPreferences {
    public var appearance: AppearancePreference {
        didSet {
            userDefaults.set(appearance.rawValue, forKey: LumiPreferenceKey.appearance)
        }
    }

    public var keyNotation: KeyNotationPreference {
        didSet {
            userDefaults.set(keyNotation.rawValue, forKey: LumiPreferenceKey.keyNotation)
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
                forKey: LumiPreferenceKey.lightingTimingOffsetMillis
            )
        }
    }

    private let userDefaults: UserDefaults

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
        appearance = userDefaults.string(forKey: LumiPreferenceKey.appearance)
            .flatMap(AppearancePreference.init(rawValue:)) ?? .dark
        keyNotation = userDefaults.string(forKey: LumiPreferenceKey.keyNotation)
            .flatMap(KeyNotationPreference.init(rawValue:)) ?? .camelot
        lightingTimingOffsetMillis = userDefaults
            .integer(forKey: LumiPreferenceKey.lightingTimingOffsetMillis)
            .clamped(to: -250...250)
    }
}

public enum LumiPreferenceKey {
    public static let appearance = "co.victorblan.tech.lumi.preference.appearance"
    public static let keyNotation = "co.victorblan.tech.lumi.preference.key-notation"
    public static let lightingTimingOffsetMillis =
        "co.victorblan.tech.lumi.preference.lighting-timing-offset-millis"
    public static let navigationAutoHide =
        "co.victorblan.tech.lumi.navigation.auto-hide"
    public static let rekordboxXMLFolder =
        "co.victorblan.tech.lumi.rekordboxXML.folder"
    public static let rekordboxXMLIncludeFutureChildren =
        "co.victorblan.tech.lumi.rekordboxXML.includeFutureChildren"
    public static let rekordboxXMLFollowedPaths =
        "co.victorblan.tech.lumi.rekordboxXML.followedPaths"
    public static let rekordboxDeviceRoot =
        "co.victorblan.tech.lumi.rekordboxDevice.root"
    public static let waveformZoomAnchor =
        "co.victorblan.tech.lumi.waveform.zoom-anchor"
    public static let waveformReverseHorizontalScroll =
        "co.victorblan.tech.lumi.waveform.reverse-horizontal-scroll"
    public static let libraryTableColumns =
        "co.victorblan.tech.lumi.library.table-columns"
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
