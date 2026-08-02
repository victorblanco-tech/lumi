import SwiftUI

struct FoundationView: View {
    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
    }

    var body: some View {
        VStack(spacing: 12) {
            Text("app.title")
                .font(.title2)
            Text("foundation.status.ready")
                .foregroundStyle(.secondary)
            Text(productVersion)
                .font(.caption.monospacedDigit())
                .foregroundStyle(.tertiary)
        }
        .frame(minWidth: 480, minHeight: 320)
        .padding(24)
    }
}

#Preview {
    FoundationView()
        .preferredColorScheme(.dark)
}
