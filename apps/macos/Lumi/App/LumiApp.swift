import SwiftUI

@main
struct LumiApp: App {
    @StateObject private var engineStatus = EngineStatusModel()

    var body: some Scene {
        WindowGroup {
            FoundationView(engineStatus: engineStatus)
                .preferredColorScheme(.dark)
                .task {
                    await engineStatus.start()
                }
        }
        .defaultSize(width: 960, height: 640)
    }
}
