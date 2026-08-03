import LumiDesignSystem
import LumiLiveWorkspace
import SwiftUI

struct FoundationView: View {
    @ObservedObject var engineStatus: EngineStatusModel
    @Bindable var preferences: LumiPreferences

    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
    }

    var body: some View {
        LiveWorkspaceView(
            state: engineStatus.workspaceState,
            productVersion: productVersion,
            appearance: $preferences.appearance,
            keyNotation: $preferences.keyNotation,
            onPlanMutation: { request in
                Task { await engineStatus.mutatePlan(request) }
            }
        )
    }
}

#Preview("Loading") {
    FoundationView(
        engineStatus: EngineStatusModel(),
        preferences: LumiPreferences()
    )
    .preferredColorScheme(.dark)
    .frame(width: 1_180, height: 820)
}
