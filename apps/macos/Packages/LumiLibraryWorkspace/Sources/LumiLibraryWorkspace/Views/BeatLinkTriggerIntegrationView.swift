import AppKit
import LumiDesignSystem
import SwiftUI

public struct BeatLinkTriggerIntegrationView: View {
    private let integration: DeckInputIntegrationState?
    @State private var copyFeedback: String?

    public init(integration: DeckInputIntegrationState?) {
        self.integration = integration
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                header
                statusPanel
                configurationPanel
                limitationsPanel
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 980, alignment: .leading)
        }
        .accessibilityIdentifier("lumi.settings.integrations.beatLinkTrigger")
    }

    private var header: some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: "cable.connector.horizontal")
                .font(.system(size: 22, weight: .semibold))
                .foregroundStyle(LumiColor.accent)
                .frame(width: 44, height: 44)
                .background(LumiColor.accent.opacity(0.14))
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                HStack {
                    Text("Beat Link Trigger")
                        .font(LumiTypography.sectionTitle)
                    Text("BUILT-IN")
                        .font(LumiTypography.technical)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(LumiColor.surfaceElevated)
                        .clipShape(Capsule())
                }
                Text("Connected deck input · provider-neutral DeckSource adapter")
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textSecondary)
            }
        }
    }

    private var statusPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    Text("MIDI Input Status").font(LumiTypography.cardTitle)
                    Spacer()
                    StatusBadge(
                        integration?.isReceiving == true ? "RECEIVING" : "WAITING",
                        state: integration?.isReceiving == true ? .ready : .degraded
                    )
                }
                statusRow("Virtual destination", integration?.destinationName ?? "Not published")
                statusRow(
                    "Protocol",
                    integration.map { "\($0.protocolName) v\($0.protocolVersion)" } ?? "Unavailable"
                )
                statusRow(
                    "Traffic",
                    "\(integration?.receivedMessageCount ?? 0) MIDI messages · \(integration?.committedFrameCount ?? 0) complete deck frames"
                )
                statusRow(
                    "Validation",
                    "\(integration?.invalidWordCount ?? 0) invalid words · \(integration?.ignoredMessageCount ?? 0) ignored · \(integration?.duplicateFrameCount ?? 0) duplicates"
                )
                statusRow(
                    "Last complete frame",
                    integration?.lastDeckID.map { deck in
                        "Deck \(deck) · sequence \(integration?.lastFrameSequence ?? 0)"
                    } ?? "None"
                )
            }
        }
    }

    private var configurationPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        Text("Beat Link Trigger Setup").font(LumiTypography.cardTitle)
                        Text("Create one trigger for Player 1 and one for Player 2. Both use the same expression; the player number becomes the MIDI channel.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Button("Copy Tracked Update Expression") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(Self.trackedUpdateExpression, forType: .string)
                        copyFeedback = "Expression copied"
                    }
                    .buttonStyle(.borderedProminent)
                }
                HStack(spacing: LumiSpacing.large) {
                    setupStep("1", "Watch", "Player 1 / Player 2")
                    setupStep("2", "MIDI Output", integration?.destinationName ?? "Lumi Deck Input")
                    setupStep("3", "Message / Enabled", "Custom / Never")
                    setupStep("4", "Expression", "Tracked Update")
                }
                if let copyFeedback {
                    Label(copyFeedback, systemImage: "checkmark.circle.fill")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.success)
                }
            }
        }
    }

    private var limitationsPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                Text("Current adapter scope").font(LumiTypography.cardTitle)
                Label("Player identity, load identity, play state, master, on-air, BPM, beat and duration", systemImage: "checkmark.circle.fill")
                Label("Unknown tracks fail safe as External track and remain AUTO HELD", systemImage: "shield.lefthalf.filled")
                Label("Track title, artist, key, RGB waveform and phrases require a later metadata transport or exact Lumi Library match", systemImage: "clock")
            }
            .font(LumiTypography.caption)
        }
    }

    private func statusRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(LumiColor.textSecondary)
            Spacer()
            Text(value).font(LumiTypography.technical).multilineTextAlignment(.trailing)
        }
    }

    private func setupStep(_ number: String, _ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text("\(number) · \(label)")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.accent)
            Text(value).font(LumiTypography.caption.weight(.semibold))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    public static let trackedUpdateExpression = """
    (when trigger-output
      (let [ch (dec device-number)
            rb (long (or rekordbox-id 0))
            loaded (pos? rb)
            flags (+ (if loaded 1 0) (if playing? 2 0)
                     (if tempo-master? 4 0) (if on-air? 8 0))
            source-player (long (or track-source-player 0))
            bpm (long (* (max 0 (or raw-bpm 0)) 10))
            current-beat (long (max 0 (or beat-number 0)))
            duration (long (or track-length 0))
            slot (case track-source-slot :sd-slot 1 :usb-slot 2
                                         :collection 3 :cd-slot 4 0)
            sequence (mod (inc (get @locals :lumi-sequence 0)) 128)
            chunk (fn [value shift] (bit-and (bit-shift-right value shift) 127))]
        (swap! locals assoc :lumi-sequence sequence)
        (doseq [[controller value]
                [[16 flags]
                 [17 (chunk rb 0)] [18 (chunk rb 7)]
                 [19 (chunk rb 14)] [20 (chunk rb 21)]
                 [21 source-player] [22 slot]
                 [23 (chunk bpm 0)] [24 (chunk bpm 7)] [25 (chunk bpm 14)]
                 [26 (chunk current-beat 0)] [27 (chunk current-beat 7)]
                 [28 (chunk current-beat 14)]
                 [29 (chunk duration 0)] [30 (chunk duration 7)]
                 [31 (chunk duration 14)] [32 sequence]
                 [119 1]]]
          (midi/midi-control trigger-output controller value ch))))
    """
}
