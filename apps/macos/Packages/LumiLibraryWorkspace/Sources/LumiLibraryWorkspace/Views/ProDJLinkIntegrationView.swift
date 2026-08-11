import LumiDesignSystem
import SwiftUI

public struct ProDJLinkIntegrationView: View {
    private let integration: DeckInputIntegrationState?

    public init(integration: DeckInputIntegrationState?) {
        self.integration = integration
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                header
                summary
                devices
                capabilityPanel
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 1_020, alignment: .leading)
        }
        .accessibilityIdentifier("lumi.integrations.proDJLink")
    }

    private var header: some View {
        HStack(spacing: LumiSpacing.medium) {
            DeckPlayerStatusIcon()
                .foregroundStyle(LumiColor.accent)
                .frame(width: 44, height: 44)
                .background(LumiColor.accent.opacity(0.14))
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                HStack {
                    Text("Pro DJ Link").font(LumiTypography.sectionTitle)
                    Text("BUILT IN · READ ONLY")
                        .font(LumiTypography.technical)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(LumiColor.surfaceElevated)
                        .clipShape(Capsule())
                }
                Text("Automatic discovery, live transport and exact USB track identity")
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textSecondary)
            }
        }
    }

    private var summary: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    Text("Connection").font(LumiTypography.cardTitle)
                    Spacer()
                    StatusBadge(LocalizedStringKey(connectionLabel), state: connectionState)
                }
                statusRow("Source", "Pro DJ Link")
                statusRow("Network state", integration?.sourceState?.uppercased() ?? "WAITING")
                statusRow("Detected devices", "\(integration?.discoveredPlayers.count ?? 0)")
                statusRow("Traffic", "\(integration?.receivedMessageCount ?? 0) bridge events")
                if let bridgeVersion = integration?.bridgeVersion,
                   let beatLinkVersion = integration?.beatLinkVersion {
                    statusRow("Runtime", "Lumi bridge \(bridgeVersion) · beat-link \(beatLinkVersion)")
                }
                if let error = integration?.lastError {
                    Label(error, systemImage: "exclamationmark.triangle.fill")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.warning)
                }
            }
        }
    }

    @ViewBuilder
    private var devices: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            Text("Detected Equipment").font(LumiTypography.sectionTitle)
            if let players = integration?.discoveredPlayers, !players.isEmpty {
                ForEach(players) { player in
                    LumiPanel {
                        HStack(spacing: LumiSpacing.large) {
                            DeckPlayerStatusIcon()
                                .foregroundStyle(LumiColor.accent)
                                .frame(width: 38, height: 38)
                            VStack(alignment: .leading, spacing: 3) {
                                Text(player.name).font(LumiTypography.cardTitle)
                                Text(equipmentDescription(player))
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            VStack(alignment: .trailing, spacing: 4) {
                                StatusBadge(LocalizedStringKey(compatibilityLabel(player)), state: compatibilityState(player))
                                Text(player.address ?? "Address unavailable")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                        }
                    }
                }
            } else {
                LumiPanel {
                    HStack(spacing: LumiSpacing.medium) {
                        Image(systemName: "dot.radiowaves.left.and.right")
                            .foregroundStyle(LumiColor.textSecondary)
                        VStack(alignment: .leading, spacing: 3) {
                            Text("Waiting for Pro DJ Link equipment").font(LumiTypography.cardTitle)
                            Text("Connect Lumi and the players or mixer to the same wired network.")
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                    }
                }
            }
        }
    }

    private var capabilityPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                Text("Compatibility").font(LumiTypography.cardTitle)
                capability("Device discovery", true)
                capability("Play, pause, position and BPM", integration?.isReceiving == true)
                capability("Master and on-air state", integration?.isReceiving == true)
                capability("USB track identity and safe library matching", integration?.isReceiving == true)
                Label("Compatibility is capability-based. Unknown models remain safely detected without Lumi guessing unsupported metadata.", systemImage: "info.circle")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
        }
    }

    private func statusRow(_ title: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title).foregroundStyle(LumiColor.textSecondary)
            Spacer()
            Text(value).font(LumiTypography.technical).multilineTextAlignment(.trailing)
        }
    }

    private func capability(_ title: String, _ ready: Bool) -> some View {
        Label(title, systemImage: ready ? "checkmark.circle.fill" : "circle.dashed")
            .font(LumiTypography.caption)
            .foregroundStyle(ready ? LumiColor.success : LumiColor.textSecondary)
    }

    private var connectionState: LumiComponentState {
        guard integration?.isProDJLink == true else { return .degraded }
        return integration?.state == "ready" ? .ready : .stale
    }

    private var connectionLabel: String {
        guard integration?.isProDJLink == true else { return "UNAVAILABLE" }
        return integration?.state == "ready" ? "READY" : "DISCOVERING"
    }

    private func equipmentDescription(_ player: ProDJLinkDeviceState) -> String {
        let upper = player.name.uppercased()
        let type = upper.contains("DJM") ? "Mixer" : upper.contains("CDJ") || upper.contains("XDJ") ? "Player" : "Pro DJ Link device"
        return "\(type) · device \(player.playerNumber)"
    }

    private func compatibilityLabel(_ player: ProDJLinkDeviceState) -> String {
        let upper = player.name.uppercased()
        return upper.contains("LUMI-SIM") ? "SIMULATOR VERIFIED" : "DETECTED"
    }

    private func compatibilityState(_ player: ProDJLinkDeviceState) -> LumiComponentState {
        player.name.uppercased().contains("LUMI-SIM") ? .ready : .stale
    }
}

private struct DeckPlayerStatusIcon: View {
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 3).strokeBorder(lineWidth: 1.6)
            RoundedRectangle(cornerRadius: 1).frame(width: 20, height: 7).offset(y: -8)
            Circle().strokeBorder(lineWidth: 1.6).frame(width: 14, height: 14).offset(y: 7)
        }
        .padding(7)
        .accessibilityHidden(true)
    }
}
