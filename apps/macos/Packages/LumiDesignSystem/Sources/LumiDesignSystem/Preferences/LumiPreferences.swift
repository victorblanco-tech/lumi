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

    /// When enabled, Lumi explicitly joins Ableton Link after its local engine
    /// is ready. The safe default is off, so launching Lumi never changes a
    /// shared Link session without a saved user choice.
    public var abletonLinkAutoStart: Bool {
        didSet {
            userDefaults.set(
                abletonLinkAutoStart,
                forKey: LumiPreferenceKey.abletonLinkAutoStart
            )
        }
    }

    /// Signed lighting-output compensation. Negative values send an AutoLoop
    /// early; positive values deliberately delay it.
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
        abletonLinkAutoStart = userDefaults.bool(
            forKey: LumiPreferenceKey.abletonLinkAutoStart
        )
        let storedTimingOffset = userDefaults
            .integer(forKey: LumiPreferenceKey.lightingTimingOffsetMillis)
            .clamped(to: -250...250)
        let usesNaturalSignedConvention = userDefaults.integer(
            forKey: LumiPreferenceKey.lightingTimingOffsetConventionVersion
        ) >= 2
        lightingTimingOffsetMillis = usesNaturalSignedConvention
            ? storedTimingOffset
            : -storedTimingOffset
        if !usesNaturalSignedConvention {
            // Dev versions through 30 exposed the inverse sign. Preserve the
            // user's physical compensation while moving to -early / +late.
            userDefaults.set(
                lightingTimingOffsetMillis,
                forKey: LumiPreferenceKey.lightingTimingOffsetMillis
            )
            userDefaults.set(
                2,
                forKey: LumiPreferenceKey.lightingTimingOffsetConventionVersion
            )
        }
    }
}

public enum LumiPreferenceKey {
    public static let appearance = "co.victorblan.tech.lumi.preference.appearance"
    public static let keyNotation = "co.victorblan.tech.lumi.preference.key-notation"
    public static let lightingTimingOffsetMillis =
        "co.victorblan.tech.lumi.preference.lighting-timing-offset-millis"
    public static let lightingTimingOffsetConventionVersion =
        "co.victorblan.tech.lumi.preference.lighting-timing-offset-convention-version"
    public static let abletonLinkAutoStart =
        "co.victorblan.tech.lumi.preference.ableton-link-auto-start"
    /// Whether the app uses its fixed compact icon rail. The persisted key is
    /// intentionally unchanged so existing users retain their preference.
    public static let navigationHidden =
        "co.victorblan.tech.lumi.navigation.auto-hide"
    public static let preferredDeckSourceMode =
        "co.victorblan.tech.lumi.live.preferred-deck-source-mode"
    public static let rekordboxDeviceRoot =
        "co.victorblan.tech.lumi.rekordboxDevice.root"
    public static let rekordboxDevicePlaylistSelections =
        "co.victorblan.tech.lumi.rekordboxDevice.playlist-selections"
    public static let rekordboxDeviceBookmarks =
        "co.victorblan.tech.lumi.rekordboxDevice.security-bookmarks"
    public static let waveformZoomAnchor =
        "co.victorblan.tech.lumi.waveform.zoom-anchor"
    public static let waveformReverseHorizontalScroll =
        "co.victorblan.tech.lumi.waveform.reverse-horizontal-scroll"
    public static let libraryTableColumns =
        "co.victorblan.tech.lumi.library.table-columns"
}

public enum DeckSourceModePreference: String, CaseIterable, Sendable {
    case connectedDecks
    case localPlayback

    /// Live Decks is the performance-safe default. Preparation mode becomes
    /// sticky only after the user explicitly selects Local Playback.
    public static func load(from userDefaults: UserDefaults = .standard) -> Self {
        userDefaults.string(forKey: LumiPreferenceKey.preferredDeckSourceMode)
            .flatMap(Self.init(rawValue:)) ?? .connectedDecks
    }

    public func persist(in userDefaults: UserDefaults = .standard) {
        userDefaults.set(rawValue, forKey: LumiPreferenceKey.preferredDeckSourceMode)
    }
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
