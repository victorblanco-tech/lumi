import LumiDesignSystem
import SwiftUI

public enum IntegrationsWorkspaceSection: String, CaseIterable, Identifiable, Sendable {
    case overview
    case deckInputs
    case lightingOutputs
    case diagnostics

    public var id: String { rawValue }
}

public struct IntegrationsWorkspaceView: View {
    private let library: LibraryWorkspaceState
    private let autoloopFeedback: String?
    private let midiIntegrationFeedback: String?
    private let rendersInteractiveControls: Bool
    private let onOpenLibrarySources: @MainActor () -> Void
    private let onAutoloopMutation: @Sendable (AutoloopCatalogMutationRequest) -> Void
    private let onPublishMidi: @Sendable () -> Void
    private let onStopMidi: @Sendable () -> Void
    private let onSendMidiAddressLearnPulse: @Sendable (String, UInt16) -> Void
    private let onTriggerMidiAutoloop: @Sendable (UInt16, UInt16) -> Void

    @State private var section: IntegrationsWorkspaceSection

    public init(
        library: LibraryWorkspaceState,
        initialSection: IntegrationsWorkspaceSection = .overview,
        autoloopFeedback: String? = nil,
        midiIntegrationFeedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        onOpenLibrarySources: @escaping @MainActor () -> Void = {},
        onAutoloopMutation: @escaping @Sendable (AutoloopCatalogMutationRequest) -> Void = { _ in },
        onPublishMidi: @escaping @Sendable () -> Void = {},
        onStopMidi: @escaping @Sendable () -> Void = {},
        onSendMidiAddressLearnPulse: @escaping @Sendable (String, UInt16) -> Void = { _, _ in },
        onTriggerMidiAutoloop: @escaping @Sendable (UInt16, UInt16) -> Void = { _, _ in }
    ) {
        self.library = library
        _section = State(initialValue: initialSection)
        self.autoloopFeedback = autoloopFeedback
        self.midiIntegrationFeedback = midiIntegrationFeedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onOpenLibrarySources = onOpenLibrarySources
        self.onAutoloopMutation = onAutoloopMutation
        self.onPublishMidi = onPublishMidi
        self.onStopMidi = onStopMidi
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
                            actionTitle: "Open Diagnostics"
                        ) { section = .diagnostics }
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
                    Text("Current transport health. Traffic inspection, logs and recovery controls are planned as a dedicated follow-up story.")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                LumiPanel {
                    VStack(spacing: 0) {
                        diagnosticRow("Pro DJ Link", proDJLinkDiagnostic, deckInputState)
                        Divider()
                        diagnosticRow("Complete deck frames", "\(library.deckInputIntegration?.committedFrameCount ?? 0)", deckInputState)
                        Divider()
                        diagnosticRow("Lighting MIDI source", library.midiIntegration?.sourceName ?? "Not published", lightingOutputState)
                        Divider()
                        diagnosticRow("MIDI test pulses", "\(library.midiIntegration?.sentPulseCount ?? 0)", lightingOutputState)
                        Divider()
                        diagnosticRow("Local Playback clock", clockDiagnostic, clockOutputState)
                        Divider()
                        diagnosticRow("MIDI Clock ticks", "\(library.midiClockIntegration?.sentTickCount ?? 0)", clockOutputState)
                        Divider()
                        diagnosticRow("Ableton Link", abletonLinkDiagnostic, abletonLinkState)
                        Divider()
                        diagnosticRow("Trusted USB sources", usbSourceDetail, usbSourceState)
                    }
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
        library.deckInputIntegration?.isReceiving == true ? .ready : .degraded
    }

    private var lightingOutputState: LumiComponentState {
        library.midiIntegration?.isReady == true ? .ready : .degraded
    }

    private var clockOutputState: LumiComponentState {
        library.midiClockIntegration?.isPublished == true ? .ready : .degraded
    }

    private var clockDiagnostic: String {
        guard let clock = library.midiClockIntegration else { return "Not published" }
        return "\(clock.sourceName) · \(clock.state.uppercased()) · \(clock.bpmDescription)"
    }

    private var abletonLinkState: LumiComponentState {
        library.abletonLinkIntegration?.isAvailable == true ? .ready : .degraded
    }

    private var abletonLinkDetail: String {
        guard let link = library.abletonLinkIntegration else { return "Starting managed timing provider" }
        if let error = link.lastError { return error }
        return "\(link.sourceDescription) · \(link.bpmDescription) · \(link.peers) peer\(link.peers == 1 ? "" : "s")"
    }

    private var abletonLinkDiagnostic: String {
        guard let link = library.abletonLinkIntegration else { return "Status unavailable" }
        let version = link.helperVersion.map { " · helper \($0)" } ?? ""
        let phase = link.phaseErrorMicros.map { " · phase \($0) µs" } ?? ""
        let age = link.lastBeatAgeMillis.map { " · beat \($0) ms ago" } ?? ""
        let reanchor = link.lastReanchor.map { " · re-anchor \($0)" } ?? ""
        return "\(link.state.uppercased()) · \(link.provider)\(version) · \(link.peers) peers\(age)\(phase)\(reanchor)"
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
        case .lightingOutputs: "Lighting Outputs"
        case .diagnostics: "Diagnostics"
        }
    }

    private func sectionIcon(_ value: IntegrationsWorkspaceSection) -> String {
        switch value {
        case .overview: "point.3.connected.trianglepath.dotted"
        case .deckInputs: "play.square.stack.fill"
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
