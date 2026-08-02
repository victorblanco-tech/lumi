import SwiftUI

@main
struct LumiApp: App {
    var body: some Scene {
        WindowGroup {
            FoundationView()
                .preferredColorScheme(.dark)
        }
        .defaultSize(width: 960, height: 640)
    }
}
