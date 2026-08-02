import SwiftUI

struct FoundationView: View {
    @ObservedObject var engineStatus: EngineStatusModel

    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
    }

    var body: some View {
        VStack(spacing: 16) {
            Text("app.title")
                .font(.title2)
            engineHealth
            Text(productVersion)
                .font(.caption.monospacedDigit())
                .foregroundStyle(.tertiary)
        }
        .frame(minWidth: 480, minHeight: 320)
        .padding(24)
    }

    @ViewBuilder
    private var engineHealth: some View {
        switch engineStatus.state {
        case .stopped:
            status("engine.status.stopped", systemImage: "stop.circle")
        case .starting:
            status("engine.status.starting", systemImage: "gearshape.2")
        case let .connecting(endpoint):
            status("engine.status.connecting", detail: endpoint, systemImage: "cable.connector")
        case let .ready(engine):
            status(
                "engine.status.ready",
                detail: "\(engine.endpoint) · engine \(engine.engineVersion) · protocol v\(engine.protocolVersion) · snapshot #\(engine.snapshotSequence)",
                systemImage: "checkmark.circle.fill"
            )
        case .disconnected:
            statusWithRetry("engine.status.disconnected", systemImage: "bolt.slash")
        case let .failed(message):
            statusWithRetry("engine.status.failed", detail: message, systemImage: "exclamationmark.triangle")
        }
    }

    private func status(
        _ title: LocalizedStringKey,
        detail: String? = nil,
        systemImage: String
    ) -> some View {
        VStack(spacing: 8) {
            Label(title, systemImage: systemImage)
                .font(.headline)
            if let detail {
                Text(verbatim: detail)
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func statusWithRetry(
        _ title: LocalizedStringKey,
        detail: String? = nil,
        systemImage: String
    ) -> some View {
        VStack(spacing: 12) {
            status(title, detail: detail, systemImage: systemImage)
            Button("engine.action.retry") {
                Task {
                    await engineStatus.restart()
                }
            }
        }
    }
}

#Preview {
    FoundationView(engineStatus: EngineStatusModel())
        .preferredColorScheme(.dark)
}
