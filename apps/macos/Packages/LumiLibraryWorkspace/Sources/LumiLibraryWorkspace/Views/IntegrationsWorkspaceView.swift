import LumiDesignSystem
import SwiftUI

public enum IntegrationsWorkspaceSection: String, CaseIterable, Identifiable, Sendable {
    case overview
    case deckInputs
    case abletonLink
    case lightingOutputs
    case diagnostics

    public var id: String { rawValue }
}

public struct IntegrationsWorkspaceView: View {
    private let library: LibraryWorkspaceState
    private let autoloopFeedback: String?
    private let midiIntegrationFeedback: String?
    private let abletonLinkFeedback: String?
    private let rendersInteractiveControls: Bool
    private let onOpenLibrarySources: @MainActor () -> Void
    private let onAutoloopMutation: @Sendable (AutoloopCatalogMutationRequest) -> Void
    private let onPublishMidi: @Sendable () -> Void
    private let onStopMidi: @Sendable () -> Void
    private let onSetAbletonLinkEnabled: @Sendable (Bool) -> Void
    private let onTestAbletonLinkHelper: @Sendable () -> Void
    private let onSendMidiAddressLearnPulse: @Sendable (String, UInt16) -> Void
    private let onTriggerMidiAutoloop: @Sendable (UInt16, UInt16) -> Void

    @State private var section: IntegrationsWorkspaceSection
    @Binding private var abletonLinkAutoStart: Bool

    public init(
        library: LibraryWorkspaceState,
        initialSection: IntegrationsWorkspaceSection = .overview,
        autoloopFeedback: String? = nil,
        midiIntegrationFeedback: String? = nil,
        abletonLinkFeedback: String? = nil,
        abletonLinkAutoStart: Binding<Bool> = .constant(false),
        rendersInteractiveControls: Bool = true,
        onOpenLibrarySources: @escaping @MainActor () -> Void = {},
        onAutoloopMutation: @escaping @Sendable (AutoloopCatalogMutationRequest) -> Void = { _ in },
        onPublishMidi: @escaping @Sendable () -> Void = {},
        onStopMidi: @escaping @Sendable () -> Void = {},
        onSetAbletonLinkEnabled: @escaping @Sendable (Bool) -> Void = { _ in },
        onTestAbletonLinkHelper: @escaping @Sendable () -> Void = {},
        onSendMidiAddressLearnPulse: @escaping @Sendable (String, UInt16) -> Void = { _, _ in },
        onTriggerMidiAutoloop: @escaping @Sendable (UInt16, UInt16) -> Void = { _, _ in }
    ) {
        self.library = library
        _section = State(initialValue: initialSection)
        _abletonLinkAutoStart = abletonLinkAutoStart
        self.autoloopFeedback = autoloopFeedback
        self.midiIntegrationFeedback = midiIntegrationFeedback
        self.abletonLinkFeedback = abletonLinkFeedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onOpenLibrarySources = onOpenLibrarySources
        self.onAutoloopMutation = onAutoloopMutation
        self.onPublishMidi = onPublishMidi
        self.onStopMidi = onStopMidi
        self.onSetAbletonLinkEnabled = onSetAbletonLinkEnabled
        self.onTestAbletonLinkHelper = onTestAbletonLinkHelper
        self.onSendMidiAddressLearnPulse = onSendMidiAddressLearnPulse
        self.onTriggerMidiAutoloop = onTriggerMidiAutoloop
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()
            HStack(spacing: 0) {
                sectionNavigation
                Divider()
                content
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.integrations")
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text("Integrations")
                .font(LumiTypography.screenTitle)
            Text("Manage the signal chain from live deck input to lighting output.")
                .font(LumiTypography.body)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .padding(.horizontal, LumiSpacing.xLarge)
        .frame(height: 82)
    }

    private var sectionNavigation: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            ForEach(IntegrationsWorkspaceSection.allCases) { value in
                sectionButton(value)
            }
            Spacer()
        }
        .padding(LumiSpacing.large)
        .frame(width: 210)
        .background(LumiColor.surface)
    }

    private func sectionButton(_ value: IntegrationsWorkspaceSection) -> some View {
        Button { section = value } label: {
            HStack(spacing: LumiSpacing.small) {
                sectionIconView(value)
                    .frame(width: 18, height: 18)
                Text(sectionTitle(value))
            }
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: LumiControlMetric.standardHeight)
                .padding(.horizontal, LumiSpacing.small)
                .foregroundStyle(section == value ? LumiColor.accent : LumiColor.textPrimary)
                .background(section == value ? LumiColor.accent.opacity(0.14) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.integrations.section.\(value.rawValue)")
    }

    @ViewBuilder
    private var content: some View {
        switch section {
        case .overview:
            overview
        case .deckInputs:
            ProDJLinkIntegrationView(integration: library.deckInputIntegration)
        case .abletonLink:
            abletonLink
        case .lightingOutputs:
            AutoloopCatalogSettingsView(
                catalog: library.autoloopCatalog,
                midiIntegration: library.midiIntegration,
                midiClockIntegration: library.midiClockIntegration,
                abletonLinkIntegration: library.abletonLinkIntegration,
                feedback: autoloopFeedback,
                midiIntegrationFeedback: midiIntegrationFeedback,
                rendersInteractiveControls: rendersInteractiveControls,
                onMutation: onAutoloopMutation,
                onPublishMidi: onPublishMidi,
                onStopMidi: onStopMidi,
                onSendMidiAddressLearnPulse: onSendMidiAddressLearnPulse,
                onTriggerMidiAutoloop: onTriggerMidiAutoloop
            )
        case .diagnostics:
            diagnostics
        }
    }

    private var overview: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("Signal Chain")
                        .font(LumiTypography.cardTitle)
                    Text("A calm operational summary. Open a component to configure it; use Diagnostics for technical investigation.")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                HStack(spacing: LumiSpacing.medium) {
                    overviewCard(
                        title: "Deck Input",
                        provider: "Pro DJ Link",
                        detail: deckInputDetail,
                        state: deckInputState,
                        actionTitle: "Open Pro DJ Link"
                    ) { section = .deckInputs }
                    overviewCard(
                        title: "Library Source",
                        provider: "USB Sources",
                        detail: usbSourceDetail,
                        state: usbSourceState,
                        actionTitle: "Open in Library"
                    ) { onOpenLibrarySources() }
                    overviewCard(
                        title: "Lighting Output",
                        provider: "SoundSwitch",
                        detail: lightingOutputDetail,
                        state: lightingOutputState,
                        actionTitle: "Open Lighting Outputs"
                    ) { section = .lightingOutputs }
                }
                VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                    Text("PARALLEL SOUNDSWITCH INPUTS")
                        .font(LumiTypography.technical.weight(.bold))
                        .foregroundStyle(LumiColor.textSecondary)
                    HStack(spacing: LumiSpacing.medium) {
                        overviewCard(
                            title: "Beat / BPM Timing",
                            provider: "Ableton Link",
                            detail: abletonLinkDetail,
                            state: abletonLinkState,
                            actionTitle: "Open Ableton Link"
                        ) { section = .abletonLink }
                        overviewCard(
                            title: "AutoLoop Selection",
                            provider: "Lumi Virtual MIDI",
                            detail: lightingOutputDetail,
                            state: lightingOutputState,
                            actionTitle: "Open Lighting Outputs"
                        ) { section = .lightingOutputs }
                        overviewCard(
                            title: "Manual Override",
                            provider: "Control One",
                            detail: "Runs beside Lumi · owned by SoundSwitch",
                            state: .ready,
                            actionTitle: "View Output Mapping"
                        ) { section = .lightingOutputs }
                    }
                }
                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                        Text("Provider Boundaries").font(LumiTypography.cardTitle)
                        Label("Deck state enters Lumi through a provider-neutral DeckSource adapter.", systemImage: "rectangle.and.arrow.down")
                        Label("Lighting commands leave Lumi through a provider-neutral MIDI output profile.", systemImage: "rectangle.and.arrow.up")
                        Label("Trusted USB sources remain read-only; Lumi owns edited phrases and planning data.", systemImage: "lock.shield")
                    }
                    .font(LumiTypography.caption)
                }
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 1_120, alignment: .leading)
        }
        .accessibilityIdentifier("lumi.integrations.overview")
    }

    private var abletonLink: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.large) {
                        HStack(spacing: LumiSpacing.medium) {
                            Image(systemName: "link")
                                .font(.system(size: 28, weight: .semibold))
                                .foregroundStyle(LumiColor.accent)
                            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                                Text("Ableton Link")
                                    .font(LumiTypography.cardTitle)
                                Text("Publishes Lumi's authoritative BPM, beat and bar timing to SoundSwitch.")
                                    .font(LumiTypography.body)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            Circle().fill(abletonLinkState.color).frame(width: 10, height: 10)
                            Text(abletonLinkStatusLabel)
                                .font(LumiTypography.metadata.weight(.semibold))
                            Toggle(
                                "Ableton Link",
                                isOn: Binding(
                                    get: { library.abletonLinkIntegration?.enabled == true },
                                    set: onSetAbletonLinkEnabled
                                )
                            )
                            .labelsHidden()
                            .toggleStyle(.switch)
                            .disabled(!rendersInteractiveControls)
                            .accessibilityIdentifier("lumi.integrations.abletonLink.enabled")
                        }

                        Divider()

                        HStack(spacing: LumiSpacing.xLarge) {
                            linkValue("TIMING SOURCE", library.abletonLinkIntegration?.sourceDescription ?? "Automatic")
                            linkValue("TEMPO", library.abletonLinkIntegration?.bpmDescription ?? "Waiting for source")
                            linkValue("LINK PEERS", "\(library.abletonLinkIntegration?.peers ?? 0)")
                            linkValue("BAR QUANTUM", "4 beats")
                        }
                    }
                }

                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.large) {
                        Text("Configuration")
                            .font(LumiTypography.cardTitle)
                        Toggle("Start Ableton Link when Lumi starts", isOn: $abletonLinkAutoStart)
                            .disabled(!rendersInteractiveControls)
                        Text("Timing source selection is automatic: Local Playback in preparation mode, and the current Pro DJ Link master in Live Decks.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                        Label(
                            "Disabling Link leaves the shared session immediately without stopping SoundSwitch.",
                            systemImage: "checkmark.shield"
                        )
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                    }
                }

                if let feedback = abletonLinkFeedback {
                    Text(feedback)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(
                            feedback.localizedCaseInsensitiveContains("could not")
                                ? LumiColor.destructive : LumiColor.success
                        )
                }
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 900, alignment: .leading)
        }
        .accessibilityIdentifier("lumi.integrations.abletonLink")
    }

    private func linkValue(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text(label)
                .font(LumiTypography.technical.weight(.bold))
                .foregroundStyle(LumiColor.textSecondary)
            Text(value)
                .font(LumiTypography.body.weight(.medium))
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func overviewCard(
        title: String,
        provider: String,
        detail: String,
        state: LumiComponentState,
        actionTitle: String,
        action: @escaping () -> Void
    ) -> some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    Text(title.uppercased())
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Spacer()
                    Circle().fill(state.color).frame(width: 9, height: 9)
                }
                Text(provider).font(LumiTypography.cardTitle)
                Text(detail)
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                    .lineLimit(2)
                Spacer(minLength: 0)
                Button(actionTitle, action: action)
                    .buttonStyle(.bordered)
            }
            .frame(maxWidth: .infinity, minHeight: 154, alignment: .leading)
        }
        .frame(maxWidth: .infinity)
    }

    private var diagnostics: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("Diagnostics")
                        .font(LumiTypography.cardTitle)
                    Text("Current transport health with a safe Ableton Link recovery check. Detailed traffic logs remain a follow-up story.")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                LumiPanel {
                    VStack(spacing: 0) {
                        diagnosticRow("Pro DJ Link", proDJLinkDiagnostic, deckInputState)
                        Divider()
                        diagnosticRow("Pro DJ Link recovery", proDJLinkRecoveryDiagnostic, proDJLinkRecoveryState)
                        Divider()
                        diagnosticRow("Complete deck frames", "\(library.deckInputIntegration?.committedFrameCount ?? 0)", deckInputState)
                        Divider()
                        diagnosticRow(
                            "Exact CDJ positions",
                            exactPositionDiagnostic,
                            library.deckInputIntegration?.positionAuthorityReady == true ? .ready : .stale
                        )
                        Divider()
                        diagnosticRow("Pro DJ Link ingress", proDJLinkIngressDiagnostic, proDJLinkIngressState)
                        Divider()
                        diagnosticRow("Lighting MIDI source", library.midiIntegration?.sourceName ?? "Not published", lightingOutputState)
                        Divider()
                        diagnosticRow("MIDI test pulses", "\(library.midiIntegration?.sentPulseCount ?? 0)", lightingOutputState)
                        Divider()
                        diagnosticRow("AutoLoop realtime output", realtimeMidiDiagnostic, realtimeMidiState)
                        Divider()
                        diagnosticRow("Local Playback clock", clockDiagnostic, clockOutputState)
                        Divider()
                        diagnosticRow("MIDI Clock ticks", "\(library.midiClockIntegration?.sentTickCount ?? 0)", clockOutputState)
                        Divider()
                        diagnosticRow("Ableton Link", abletonLinkDiagnostic, abletonLinkState)
                        Divider()
                        diagnosticRow("Timing anchors", abletonLinkAnchorDiagnostic, abletonLinkState)
                        Divider()
                        diagnosticRow("Timing corrections", abletonLinkCorrectionDiagnostic, abletonLinkState)
                        Divider()
                        diagnosticRow("Realtime engine lane", realtimeTimingLaneDiagnostic, abletonLinkState)
                        Divider()
                        diagnosticRow("Timing safety", abletonLinkSafetyDiagnostic, abletonLinkState)
                        Divider()
                        diagnosticRow("Trusted USB sources", usbSourceDetail, usbSourceState)
                    }
                }
                HStack(spacing: LumiSpacing.medium) {
                    Button("Test Ableton Link Helper", action: onTestAbletonLinkHelper)
                        .buttonStyle(.borderedProminent)
                        .tint(LumiColor.accent)
                        .disabled(library.abletonLinkIntegration?.enabled == true)
                    Text("Available while Lumi is Off. Verifies the bundled helper and pinned version without joining the Link session or sending lighting commands.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                    Spacer()
                }
                if let midiIntegrationFeedback {
                    Text(midiIntegrationFeedback)
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                if let abletonLinkFeedback {
                    Text(abletonLinkFeedback)
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Label("Recovery actions and event logging are deliberately not duplicated on Overview.", systemImage: "wrench.and.screwdriver")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 900, alignment: .leading)
        }
        .accessibilityIdentifier("lumi.integrations.diagnostics")
    }

    private func diagnosticRow(_ label: String, _ value: String, _ state: LumiComponentState) -> some View {
        HStack {
            Circle().fill(state.color).frame(width: 9, height: 9)
            Text(label).foregroundStyle(LumiColor.textSecondary)
            Spacer()
            Text(value).font(LumiTypography.technical)
        }
        .padding(.vertical, LumiSpacing.medium)
    }

    private var deckInputState: LumiComponentState {
        guard let input = library.deckInputIntegration else { return .loading }
        if input.lastError != nil || input.recoveryPending { return .degraded }
        return input.isReady ? .ready : .loading
    }

    private var proDJLinkRecoveryState: LumiComponentState {
        library.deckInputIntegration?.recoveryPending == true ? .degraded : deckInputState
    }

    private var proDJLinkIngressState: LumiComponentState {
        guard let input = library.deckInputIntegration else { return .loading }
        return input.ingressCriticalSaturationCount > 0 ? .degraded : deckInputState
    }

    private var lightingOutputState: LumiComponentState {
        guard let midi = library.midiIntegration, midi.isReady else { return .degraded }
        return midi.realtimeLane?.isHealthy == false ? .degraded : .ready
    }

    private var realtimeMidiState: LumiComponentState {
        guard let lane = library.midiIntegration?.realtimeLane else { return .loading }
        return lane.isHealthy ? .ready : .degraded
    }

    private var realtimeMidiDiagnostic: String {
        guard let lane = library.midiIntegration?.realtimeLane else {
            return "Waiting for realtime output lane"
        }
        let p95 = Double(lane.latencyP95Micros) / 1_000
        let last = Double(lane.lastDispatchLatenessMicros) / 1_000
        return "\(lane.queueDepth)/\(lane.queueCapacity) queued · peak \(lane.queueHighWater) · p95 \(p95.formatted(.number.precision(.fractionLength(1)))) ms · last \(last.formatted(.number.precision(.fractionLength(1)))) ms · \(lane.lateDispatchCount) late · \(lane.saturationCount) saturation"
    }

    private var clockOutputState: LumiComponentState {
        library.midiClockIntegration?.isPublished == true ? .ready : .degraded
    }

    private var clockDiagnostic: String {
        guard let clock = library.midiClockIntegration else { return "Not published" }
        return "\(clock.sourceName) · \(clock.state.uppercased()) · \(clock.bpmDescription)"
    }

    private var abletonLinkState: LumiComponentState {
        guard let link = library.abletonLinkIntegration else { return .loading }
        if link.lastError != nil { return .degraded }
        if !link.enabled { return .empty }
        return link.isAvailable ? .ready : .loading
    }

    private var abletonLinkStatusLabel: String {
        guard let link = library.abletonLinkIntegration else { return "Unavailable" }
        if link.lastError != nil { return "Problem" }
        if !link.enabled { return "Off" }
        switch link.state {
        case "running": return "In Sync"
        case "ready": return "On · Waiting"
        case "starting": return "Starting"
        default: return link.state.capitalized
        }
    }

    private var abletonLinkDetail: String {
        guard let link = library.abletonLinkIntegration else { return "Starting managed timing provider" }
        if let error = link.lastError { return error }
        if !link.enabled { return "Off · available when needed" }
        return "\(link.sourceDescription) · \(link.bpmDescription) · \(link.peers) peer\(link.peers == 1 ? "" : "s")"
    }

    private var abletonLinkDiagnostic: String {
        guard let link = library.abletonLinkIntegration else { return "Status unavailable" }
        let version = link.helperVersion.map { " · helper \($0)" } ?? ""
        let phase = link.phaseErrorMicros.map { " · phase \($0) µs" } ?? ""
        let age = link.lastBeatAgeMillis.map { " · tempo update \($0) ms ago" } ?? ""
        let reanchor = link.lastReanchor.map { " · re-anchor \($0)" } ?? ""
        return "\(link.state.uppercased()) · \(link.provider)\(version) · \(link.peers) peers\(age)\(phase)\(reanchor)"
    }

    private var abletonLinkAnchorDiagnostic: String {
        guard let link = library.abletonLinkIntegration else { return "Status unavailable" }
        return "\(link.appliedAnchorCount) applied / \(link.receivedAnchorCount) received · \(link.coalescedAnchorCount) safely coalesced"
    }

    private var abletonLinkCorrectionDiagnostic: String {
        guard let link = library.abletonLinkIntegration else { return "Status unavailable" }
        return "\(link.hardReanchorCount) hard · \(link.softCorrectionCount) soft · max \(link.maxAbsPhaseErrorMicros) µs"
    }

    private var abletonLinkSafetyDiagnostic: String {
        guard let link = library.abletonLinkIntegration else { return "Status unavailable" }
        return "\(link.failClosedCount) fail-closed holds · \(link.failureCount) provider failures"
    }

    private var realtimeTimingLaneDiagnostic: String {
        guard let link = library.abletonLinkIntegration else { return "Status unavailable" }
        let maximumMillis = Double(link.enginePumpMaxLatenessMicros) / 1_000
        return "\(link.enginePumpCount) ticks · \(link.enginePumpStarvationCount) late · max +\(maximumMillis.formatted(.number.precision(.fractionLength(1)))) ms"
    }

    private var proDJLinkRecoveryDiagnostic: String {
        guard let input = library.deckInputIntegration else { return "Status unavailable" }
        if input.recoveryPending {
            return "Retrying automatically · \(input.restartCount) completed restarts"
        }
        return "Ready · \(input.restartCount) automatic restarts"
    }

    private var proDJLinkIngressDiagnostic: String {
        guard let input = library.deckInputIntegration else { return "Status unavailable" }
        guard input.isProDJLink, input.ingressQueueCapacity > 0 else {
            return "Waiting for direct Pro DJ Link bridge"
        }
        let p95Millis = Double(input.ingressSourceAgeP95Micros) / 1_000
        let maxMillis = Double(input.ingressSourceAgeMaxMicros) / 1_000
        return "\(input.ingressQueueDepth)/\(input.ingressQueueCapacity) queued · peak \(input.ingressQueueHighWater) · source p95 \(p95Millis.formatted(.number.precision(.fractionLength(1)))) ms · max \(maxMillis.formatted(.number.precision(.fractionLength(1)))) ms · \(input.ingressCoalescedMessageCount) coalesced · \(input.ingressCriticalSaturationCount) critical saturation"
    }

    private var exactPositionDiagnostic: String {
        guard let input = library.deckInputIntegration, input.isProDJLink else {
            return "Waiting for Pro DJ Link"
        }
        if input.positionAuthorityReady {
            return "READY · \(input.authoritativePositionCount) mapped · \(input.positionDiscontinuityCount) hotcues/seeks safely invalidated"
        }
        return "WAITING · automatic output held"
    }

    private var deckInputDetail: String {
        let frames = library.deckInputIntegration?.committedFrameCount ?? 0
        let players = library.deckInputIntegration?.discoveredPlayers.count ?? 0
        return frames > 0 ? "Ready · \(players) devices · \(frames) events" : "Discovering · waiting for equipment"
    }

    private var lightingOutputDetail: String {
        guard let midi = library.midiIntegration else { return "Not published" }
        return midi.isReady ? "Ready · \(midi.sourceName)" : "\(midi.state) · \(midi.sourceName)"
    }

    private var usbSourceState: LumiComponentState {
        if library.rekordboxDevices.contains(where: { $0.conflictTracks > 0 }) { return .degraded }
        return library.rekordboxDevices.isEmpty ? .empty : .ready
    }

    private var usbSourceDetail: String {
        let devices = library.rekordboxDevices
        guard !devices.isEmpty else { return "No trusted USB sources" }
        let protected = devices.reduce(UInt64(0)) { $0 + $1.protectedTracks }
        let conflicts = devices.reduce(UInt64(0)) { $0 + $1.conflictTracks }
        if conflicts > 0 { return "\(devices.count) trusted · \(conflicts) changes need review" }
        if protected > 0 { return "\(devices.count) trusted · \(protected) older versions protected" }
        return "\(devices.count) trusted · all synchronized safely"
    }

    private var proDJLinkDiagnostic: String {
        guard let input = library.deckInputIntegration, input.isProDJLink else {
            return "Unavailable"
        }
        return "\(input.sourceState ?? input.state) · \(input.discoveredPlayers.count) devices · \(input.receivedMessageCount) events"
    }

    private func sectionTitle(_ value: IntegrationsWorkspaceSection) -> String {
        switch value {
        case .overview: "Overview"
        case .deckInputs: "Pro DJ Link"
        case .abletonLink: "Ableton Link"
        case .lightingOutputs: "Lighting Outputs"
        case .diagnostics: "Diagnostics"
        }
    }

    private func sectionIcon(_ value: IntegrationsWorkspaceSection) -> String {
        switch value {
        case .overview: "point.3.connected.trianglepath.dotted"
        case .deckInputs: "play.square.stack.fill"
        case .abletonLink: "link"
        case .lightingOutputs: "lightbulb.2.fill"
        case .diagnostics: "stethoscope"
        }
    }

    @ViewBuilder
    private func sectionIconView(_ value: IntegrationsWorkspaceSection) -> some View {
        if value == .deckInputs {
            DeckPlayerIcon()
        } else {
            Image(systemName: sectionIcon(value))
        }
    }
}

private struct DeckPlayerIcon: View {
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 2.5)
                .strokeBorder(lineWidth: 1.5)
            RoundedRectangle(cornerRadius: 1)
                .frame(width: 10, height: 4)
                .offset(y: -4)
            Circle()
                .strokeBorder(lineWidth: 1.5)
                .frame(width: 7, height: 7)
                .offset(y: 4)
        }
        .padding(1)
        .accessibilityHidden(true)
    }
}
