import AppKit
import LumiDesignSystem
import SwiftUI

@main
struct LumiApp: App {
    @StateObject private var engineStatus = EngineStatusModel()
    @State private var preferences = LumiPreferences()

    var body: some Scene {
        WindowGroup {
            FoundationView(
                engineStatus: engineStatus,
                preferences: preferences
            )
                .onAppear {
                    MacApplicationAppearance.apply(preferences.appearance)
                }
                .onChange(of: preferences.appearance) { _, appearance in
                    MacApplicationAppearance.apply(appearance)
                }
                .onChange(of: preferences.lightingTimingOffsetMillis) { _, millis in
                    Task {
                        await engineStatus.setLightingTimingOffset(millis)
                    }
                }
                .task {
                    await engineStatus.start()
                    await engineStatus.setLightingTimingOffset(
                        preferences.lightingTimingOffsetMillis
                    )
                }
        }
        .defaultSize(width: 1_280, height: 820)
    }
}

@MainActor
private enum MacApplicationAppearance {
    static func apply(_ preference: AppearancePreference) {
        NSApplication.shared.appearance = switch preference {
        case .dark:
            NSAppearance(named: .darkAqua)
        case .light:
            NSAppearance(named: .aqua)
        case .system:
            nil
        }
    }
}
