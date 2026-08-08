import Foundation
import Testing
@testable import LumiDesignSystem

@MainActor
@Test("First launch defaults to dark appearance, Camelot notation, and neutral lighting timing")
func defaultsAreStable() throws {
    let suite = "LumiPreferencesTests.defaults.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }

    let preferences = LumiPreferences(userDefaults: defaults)
    #expect(preferences.appearance == .dark)
    #expect(preferences.keyNotation == .camelot)
    #expect(preferences.lightingTimingOffsetMillis == 0)
}

@MainActor
@Test("Appearance, key notation, and signed lighting timing persist globally")
func preferencesPersist() throws {
    let suite = "LumiPreferencesTests.persistence.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }

    let preferences = LumiPreferences(userDefaults: defaults)
    preferences.appearance = .light
    preferences.keyNotation = .classic
    preferences.lightingTimingOffsetMillis = 35

    let restored = LumiPreferences(userDefaults: defaults)
    #expect(restored.appearance == .light)
    #expect(restored.keyNotation == .classic)
    #expect(restored.lightingTimingOffsetMillis == 35)
}

@MainActor
@Test("Lighting timing is clamped before it is exposed or persisted")
func lightingTimingIsClamped() throws {
    let suite = "LumiPreferencesTests.timingClamp.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }

    let preferences = LumiPreferences(userDefaults: defaults)
    preferences.lightingTimingOffsetMillis = 500
    #expect(preferences.lightingTimingOffsetMillis == 250)

    let restored = LumiPreferences(userDefaults: defaults)
    #expect(restored.lightingTimingOffsetMillis == 250)
}
