import SwiftUI
import LumiDesignSystem

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
                .preferredColorScheme(preferences.appearance.colorScheme)
                .task {
                    await engineStatus.start()
                }
        }
        .defaultSize(width: 1_100, height: 760)
    }
}
