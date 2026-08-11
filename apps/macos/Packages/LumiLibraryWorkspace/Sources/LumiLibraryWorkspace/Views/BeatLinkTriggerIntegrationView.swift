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
                    Text("Legacy MIDI Fallback")
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
                        Text("Legacy MIDI Fallback Setup").font(LumiTypography.cardTitle)
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
                Label("Exact Device Library matches hydrate title, artist, key, RGB waveform, beatgrid and the Lumi phrase plan", systemImage: "checkmark.shield.fill")
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
            raw-position (playback-time status)
            position-known? (some? raw-position)
            current-position (long (max 0 (or raw-position 0)))
            sampled-position (* 100 (quot current-position 100))
            flags (+ (if loaded 1 0) (if playing? 2 0)
                     (if tempo-master? 4 0) (if on-air? 8 0)
                     (if position-known? 16 0))
            source-player (long (or track-source-player 0))
            raw-track-bpm (double (max 0 (or raw-bpm 0)))
            simulating? (some? util/*simulating*)
            sim-meta (:metadata util/*simulating*)
            pitch-scale (double (max 0.000001 pitch-multiplier))
            track-bpm (long (Math/round
                              (if simulating?
                                (/ (* raw-track-bpm 10.0) pitch-scale)
                                (* raw-track-bpm 10.0))))
            effective-bpm (long (Math/round
                                  (if simulating?
                                    (* raw-track-bpm 10.0)
                                    (* (double (max 0.0 (or effective-tempo 0.0))) 1000.0))))
            duration (long (or track-length 0))
            sim-separator (str (char 31))
            sim-identity (when simulating?
                           (str (clojure.string/lower-case (clojure.string/trim (str (or (:title sim-meta) ""))))
                                sim-separator
                                (clojure.string/lower-case (clojure.string/trim (str (or (:artist sim-meta) ""))))
                                sim-separator (long (/ track-bpm 10))
                                sim-separator duration))
            sim-signature (if simulating?
                            (long (bit-and 0xffffffff (.hashCode ^String sim-identity)))
                            0)
            current-beat (long (max 0 (or beat-number 0)))
            slot (case track-source-slot :sd-slot 1 :usb-slot 2
                                         :collection 3 :cd-slot 4 0)
            frame-key [flags rb source-player slot track-bpm current-beat
                       duration effective-bpm sim-signature sampled-position]
            now-ms (System/currentTimeMillis)
            last-sent-ms (long (or (:lumi-last-sent-ms @locals) 0))
            send-frame? (or (not= frame-key (:lumi-last-frame @locals))
                            (>= (- now-ms last-sent-ms) 1000))
            sequence (mod (inc (get @locals :lumi-sequence 0)) 128)
            chunk (fn [value shift] (bit-and (bit-shift-right value shift) 127))]
        (when send-frame?
          (swap! locals assoc
                 :lumi-sequence sequence
                 :lumi-last-frame frame-key
                 :lumi-last-sent-ms now-ms)
          (doseq [[controller value]
                  [[16 flags]
                   [17 (chunk rb 0)] [18 (chunk rb 7)]
                   [19 (chunk rb 14)] [20 (chunk rb 21)]
                   [21 source-player] [22 slot]
                   [23 (chunk track-bpm 0)] [24 (chunk track-bpm 7)] [25 (chunk track-bpm 14)]
                   [26 (chunk current-beat 0)] [27 (chunk current-beat 7)]
                   [28 (chunk current-beat 14)]
                   [29 (chunk duration 0)] [30 (chunk duration 7)]
                   [31 (chunk duration 14)] [32 sequence]
                   [33 (chunk effective-bpm 0)] [34 (chunk effective-bpm 7)]
                   [35 (chunk effective-bpm 14)]
                   [36 (chunk sim-signature 0)] [37 (chunk sim-signature 7)]
                   [38 (chunk sim-signature 14)] [39 (chunk sim-signature 21)]
                   [40 (chunk sim-signature 28)]
                   [41 (chunk sampled-position 0)] [42 (chunk sampled-position 7)]
                   [43 (chunk sampled-position 14)]
                   [119 4]]]
            (midi/midi-control trigger-output controller value ch)))))
    """
}
