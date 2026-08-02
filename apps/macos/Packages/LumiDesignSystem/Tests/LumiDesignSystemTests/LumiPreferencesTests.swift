import Foundation
import Testing
@testable import LumiDesignSystem

@MainActor
@Test("First launch defaults to dark appearance and Camelot notation")
func defaultsAreStable() throws {
    let suite = "LumiPreferencesTests.defaults.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }

    let preferences = LumiPreferences(userDefaults: defaults)
    #expect(preferences.appearance == .dark)
    #expect(preferences.keyNotation == .camelot)
}

@MainActor
@Test("Appearance and key notation persist globally")
func preferencesPersist() throws {
    let suite = "LumiPreferencesTests.persistence.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }

    let preferences = LumiPreferences(userDefaults: defaults)
    preferences.appearance = .light
    preferences.keyNotation = .classic

    let restored = LumiPreferences(userDefaults: defaults)
    #expect(restored.appearance == .light)
    #expect(restored.keyNotation == .classic)
}
