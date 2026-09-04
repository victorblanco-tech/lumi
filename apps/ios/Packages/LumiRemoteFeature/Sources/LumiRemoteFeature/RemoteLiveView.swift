import LumiDesignSystem
import LumiRemoteClient
import SwiftUI
#if os(iOS)
import UIKit
#endif

public struct RemoteLiveActions: Sendable {
    public let setOperationState: @MainActor @Sendable (RemoteOperationState) -> Void
    public let setAbletonLinkEnabled: @MainActor @Sendable (Bool) -> Void
    public let setTimingOffset: @MainActor @Sendable (Int) -> Void
    public let changePhraseRole: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, String) -> Void
    public let selectTheme: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, UInt64) -> Void
    public let selectAutoloop: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, UInt8) -> Void
    public let setCueLock: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, Bool) -> Void

    public init(
        setOperationState: @escaping @MainActor @Sendable (RemoteOperationState) -> Void,
        setAbletonLinkEnabled: @escaping @MainActor @Sendable (Bool) -> Void,
        setTimingOffset: @escaping @MainActor @Sendable (Int) -> Void,
        changePhraseRole: @escaping @MainActor @Sendable (
            RemoteLightPlan,
            RemotePlanCue,
            String
        ) -> Void,
        selectTheme: @escaping @MainActor @Sendable (
            RemoteLightPlan,
            RemotePlanCue,
            UInt64
        ) -> Void,
        selectAutoloop: @escaping @MainActor @Sendable (
            RemoteLightPlan,
            RemotePlanCue,
            UInt8
        ) -> Void,
        setCueLock: @escaping @MainActor @Sendable (
            RemoteLightPlan,
            RemotePlanCue,
            Bool
        ) -> Void
    ) {
        self.setOperationState = setOperationState
        self.setAbletonLinkEnabled = setAbletonLinkEnabled
        self.setTimingOffset = setTimingOffset
        self.changePhraseRole = changePhraseRole
        self.selectTheme = selectTheme
        self.selectAutoloop = selectAutoloop
        self.setCueLock = setCueLock
    }
}

public struct RemoteLiveView: View {
    @Bindable private var model: RemoteSessionModel
    private let actions: RemoteLiveActions
    @State private var selectedPlanCue: SelectedPlanCue?

    public init(model: RemoteSessionModel, actions: RemoteLiveActions) {
        self.model = model
        self.actions = actions
    }

    public var body: some View {
        GeometryReader { geometry in
            let isLandscape = geometry.size.width > geometry.size.height
            VStack(spacing: 0) {
                RemoteTopBar(model: model, actions: actions, isLandscape: isLandscape)
                Divider().overlay(LumiColor.border)
                content(isLandscape: isLandscape)
            }
            .background(LumiColor.canvas)
            .foregroundStyle(LumiColor.textPrimary)
            .sheet(item: $selectedPlanCue) { selection in
                RemotePlanCueSheet(
                    projection: selection.projection,
                    plan: selection.plan,
                    initialCue: selection.cue,
                    controlsEnabled: model.controlsEnabled,
                    actions: actions
                )
                .presentationDetents([.medium])
            }
            .sensoryFeedback(.success, trigger: model.acceptedCommandFeedbackRevision)
            .sensoryFeedback(.error, trigger: model.rejectedCommandFeedbackRevision)
        }
    }

    @ViewBuilder
    private func content(isLandscape: Bool) -> some View {
        if let projection = model.projection {
            let slots = RemotePlayerOrdering.visibleSlots(
                in: projection,
                isLandscape: isLandscape
            )
            if isLandscape {
                HStack(spacing: LumiSpacing.small) {
                    ForEach(slots) { slot in
                        slotSurface(slot, projection: projection, isLandscape: true)
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                    }
                }
                .padding(LumiSpacing.small)
            } else {
                ScrollView {
                    LazyVStack(spacing: LumiSpacing.medium) {
                        ForEach(slots) { slot in
                            slotSurface(slot, projection: projection, isLandscape: false)
                        }
                    }
                    .padding(LumiSpacing.small)
                }
            }
        } else {
            remoteEmptyState
        }
    }

    private var remoteEmptyState: some View {
        let content: (title: String, icon: String, description: String) = switch model.connectionPhase {
        case .discovering:
            (
                "Finding Lumi",
                "network",
                "Keep this iPhone and the Lumi Mac on the same local network."
            )
        case .pairing:
            (
                "Lumi Mac Found",
                "iphone.and.arrow.forward",
                model.pairingShortCode.map {
                    "Confirm code \($0) on the Mac, then approve this iPhone in Integrations › iPhone Remote."
                } ?? "Create a pairing code in Integrations › iPhone Remote on the Mac, then scan it with the iPhone Camera."
            )
        case let .incompatible(required, received):
            (
                "Update Required",
                "exclamationmark.triangle",
                "This app uses Remote protocol \(required); the Mac announced \(received)."
            )
        case .connected, .reconnecting:
            (
                "Waiting for Live Players",
                "hifispeaker.2",
                "Lumi will show detected Pro DJ Link Players here."
            )
        case .unavailable:
            (
                "Lumi Mac Unavailable",
                "wifi.exclamationmark",
                model.lastError ?? "Check the local network and Remote Gateway on the Mac."
            )
        }
        return ContentUnavailableView(
            content.title,
            systemImage: content.icon,
            description: Text(content.description)
        )
        .foregroundStyle(LumiColor.textSecondary)
    }

    private func playerSurface(
        _ player: RemotePlayer,
        projection: RemoteLiveProjection,
        isLandscape: Bool
    ) -> some View {
        let isMaster = projection.leaderPlayerNumber == player.playerNumber
        let plan = [projection.livePlan, projection.nextPlan]
            .compactMap { $0 }
            .first {
                $0.playerNumber == player.playerNumber
                    && $0.trackLoadID == player.trackLoadID
            }
        return RemotePlayerSurface(
            player: player,
            plan: plan,
            isMaster: isMaster,
            isLandscape: isLandscape,
            operationState: projection.operationState,
            controlsEnabled: model.controlsEnabled,
            onSelectCue: { cue in
                guard let plan else { return }
                selectedPlanCue = SelectedPlanCue(
                    projection: projection,
                    plan: plan,
                    cue: cue
                )
            }
        )
    }

    @ViewBuilder
    private func slotSurface(
        _ slot: RemotePlayerSlot,
        projection: RemoteLiveProjection,
        isLandscape: Bool
    ) -> some View {
        if let player = slot.player {
            playerSurface(
                player,
                projection: projection,
                isLandscape: isLandscape
            )
        } else {
            RemoteEmptyPlayerSurface(
                playerNumber: slot.playerNumber,
                isLandscape: isLandscape
            )
        }
    }
}

private struct RemoteTopBar: View {
    @Bindable var model: RemoteSessionModel
    let actions: RemoteLiveActions
    let isLandscape: Bool
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var confirmsStoppingShow = false

    var body: some View {
        VStack(spacing: isLandscape ? 3 : LumiSpacing.small) {
            if isLandscape && !dynamicTypeSize.isAccessibilitySize {
                HStack(spacing: LumiSpacing.small) {
                    identity
                        .frame(width: 132, alignment: .leading)
                    integrationStatus
                    Spacer(minLength: 4)
                    operationControls(compact: true)
                    timingOffsetControl(compact: true)
                }
            } else {
                HStack {
                    identity
                    Spacer()
                    integrationStatus
                }
                HStack(spacing: LumiSpacing.small) {
                    operationControls(compact: false)
                    timingOffsetControl(compact: false)
                }
            }
            commandFeedback
        }
        .padding(.horizontal, LumiSpacing.medium)
        .padding(.vertical, isLandscape ? 5 : LumiSpacing.small)
        .background(LumiColor.surface)
        .alert("Stop the live show?", isPresented: $confirmsStoppingShow) {
            Button("Keep Running", role: .cancel) {}
            Button("Stop Show", role: .destructive) {
                actions.setOperationState(.off)
            }
        } message: {
            Text("Lumi will stop sending new lighting choices until you arm and start it again.")
        }
    }

    private var identity: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text("Lumi Remote")
                .font(LumiTypography.sectionTitle)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
            Text(connectionLabel)
                .font(LumiTypography.caption)
                .foregroundStyle(connectionColor)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
        }
    }

    @ViewBuilder
    private var integrationStatus: some View {
        let integrations = visibleIntegrations
        HStack(spacing: isLandscape ? 7 : LumiSpacing.small) {
            integrationBadge("PDL", integrations.proDJLink)
            integrationBadge("LIGHT", integrations.lightOutput)
            Button {
                actions.setAbletonLinkEnabled(!integrations.abletonLinkEnabled)
            } label: {
                HStack(spacing: 4) {
                    Circle()
                        .fill(healthColor(integrations.abletonLink))
                        .frame(width: 7, height: 7)
                    Text(linkLabel(integrations))
                }
                .font(LumiTypography.technical.weight(.semibold))
                .frame(minHeight: isLandscape ? 34 : 44)
            }
            .buttonStyle(.plain)
            .disabled(!model.controlsEnabled)
            .accessibilityLabel("Ableton Link")
            .accessibilityValue(linkAccessibilityValue(integrations))
            .accessibilityHint(
                integrations.abletonLinkEnabled
                    ? "Double tap to turn Ableton Link off"
                    : "Double tap to turn Ableton Link on"
            )
        }
    }

    private var visibleIntegrations: RemoteIntegrationStatus {
        guard case .connected = model.connectionPhase,
              let integrations = model.projection?.integrations else {
            return RemoteIntegrationStatus(
                proDJLink: .unavailable,
                lightOutput: .unavailable,
                abletonLink: .unavailable,
                abletonLinkEnabled: false,
                abletonLinkBPMMilli: nil,
                timingOffsetMillis: model.projection?.integrations.timingOffsetMillis ?? 0,
                pendingTimingOffsetMillis: nil
            )
        }
        return integrations
    }

    private func operationControls(compact: Bool) -> some View {
        HStack(spacing: compact ? 5 : LumiSpacing.small) {
            ForEach(RemoteOperationState.allCases, id: \.self) { state in
                Button {
                    if state == .off, model.projection?.operationState == .live {
                        confirmsStoppingShow = true
                    } else {
                        actions.setOperationState(state)
                    }
                } label: {
                    Text(operationLabel(state))
                        .font(LumiTypography.caption.weight(.bold))
                        .frame(
                            minWidth: compact ? 54 : 0,
                            maxWidth: compact ? 64 : .infinity,
                            minHeight: compact ? 34 : 44
                        )
                        .background(
                            RoundedRectangle(cornerRadius: LumiRadius.control)
                                .fill(operationColor(state).opacity(
                                    model.projection?.operationState == state ? 0.18 : 0.04
                                ))
                        )
                        .overlay {
                            RoundedRectangle(cornerRadius: LumiRadius.control)
                                .stroke(
                                    model.projection?.operationState == state
                                        ? operationColor(state)
                                        : LumiColor.border,
                                    lineWidth: model.projection?.operationState == state ? 1.5 : 1
                                )
                        }
                        .frame(maxWidth: compact ? 64 : .infinity, minHeight: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(operationColor(state))
                .disabled(!model.controlsEnabled)
                .accessibilityLabel(operationAccessibilityLabel(state))
                .accessibilityValue(
                    model.projection?.operationState == state ? "Current mode" : "Available"
                )
                .accessibilityHint("Double tap to change the Lumi show mode")
            }
        }
    }

    @ViewBuilder
    private func timingOffsetControl(compact: Bool) -> some View {
        if let integrations = model.projection?.integrations {
            Menu {
                ForEach(Array(stride(from: -100, through: 100, by: 10)), id: \.self) { value in
                    Button(offsetLabel(value)) {
                        actions.setTimingOffset(value)
                    }
                }
            } label: {
                Text(offsetLabel(integrations.timingOffsetMillis))
                    .font(LumiTypography.technical.weight(.semibold))
                    .frame(minWidth: 54, minHeight: compact ? 34 : 44)
                    .frame(minHeight: 44)
                    .contentShape(Rectangle())
            }
            .disabled(!model.controlsEnabled)
            .accessibilityLabel("Lighting timing offset")
            .accessibilityValue(offsetLabel(integrations.timingOffsetMillis))
            .accessibilityHint("Double tap to choose an offset")
        }
    }

    private var commandFeedback: some View {
        HStack(spacing: 5) {
            if let error = model.lastError {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(LumiColor.warning)
                Text(error)
                    .lineLimit(1)
            } else if !model.pendingCommandIDs.isEmpty {
                ProgressView()
                    .controlSize(.mini)
                Text("Applying change…")
            } else {
                Text("Ready")
                    .opacity(0)
                    .accessibilityHidden(true)
            }
            Spacer(minLength: 0)
        }
        .font(LumiTypography.caption)
        .foregroundStyle(LumiColor.textSecondary)
        .frame(height: isLandscape ? 11 : 16)
        .accessibilityElement(children: .combine)
    }

    private var connectionLabel: String {
        switch model.connectionPhase {
        case let .connected(macName): "Connected · \(macName)"
        case let .reconnecting(macName, _): "Reconnecting · \(macName)"
        case .discovering: "Finding Lumi on the local network"
        case .pairing: "Pairing"
        case .incompatible: "Update required"
        case .unavailable: "Mac unavailable"
        }
    }

    private var connectionColor: Color {
        model.controlsEnabled ? LumiColor.success : LumiColor.warning
    }

    private func integrationBadge(
        _ label: String,
        _ health: RemoteIntegrationHealth
    ) -> some View {
        HStack(spacing: 4) {
            Circle().fill(healthColor(health)).frame(width: 7, height: 7)
            Text(label)
        }
        .font(LumiTypography.technical.weight(.semibold))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(integrationAccessibilityLabel(label)), \(healthAccessibilityLabel(health))")
    }

    private func linkLabel(_ status: RemoteIntegrationStatus) -> String {
        guard let bpm = status.abletonLinkBPMMilli else { return "LINK" }
        return String(format: "%.1f", Double(bpm) / 1_000)
    }

    private func offsetLabel(_ value: Int) -> String {
        value == 0 ? "0 ms" : String(format: "%+d ms", value)
    }

    private func operationAccessibilityLabel(_ state: RemoteOperationState) -> String {
        switch state {
        case .off: "Off"
        case .armed: "Arm"
        case .live: "Start"
        case .paused: "Pause"
        }
    }

    private func integrationAccessibilityLabel(_ label: String) -> String {
        switch label {
        case "PDL": "Pro DJ Link"
        case "LIGHT": "Light Output"
        default: label
        }
    }

    private func healthAccessibilityLabel(_ health: RemoteIntegrationHealth) -> String {
        switch health {
        case .ready: "ready"
        case .starting: "starting"
        case .degraded: "needs attention"
        case .unavailable: "unavailable"
        }
    }

    private func linkAccessibilityValue(_ status: RemoteIntegrationStatus) -> String {
        let enabled = status.abletonLinkEnabled ? "on" : "off"
        let health = healthAccessibilityLabel(status.abletonLink)
        guard let bpm = status.abletonLinkBPMMilli else {
            return "\(enabled), \(health)"
        }
        return String(
            format: "%@, %@, %.1f BPM",
            enabled,
            health,
            Double(bpm) / 1_000
        )
    }
}

private struct RemotePlayerSurface: View {
    let player: RemotePlayer
    let plan: RemoteLightPlan?
    let isMaster: Bool
    let isLandscape: Bool
    let operationState: RemoteOperationState
    let controlsEnabled: Bool
    let onSelectCue: (RemotePlanCue) -> Void
    @State private var manualZoomBars: Double?
    @State private var inspectionStartBeat: Double?
    @GestureState private var dragTranslation: CGFloat = 0
    @GestureState private var magnification: CGFloat = 1

    var body: some View {
        playerCard
    }

    private var playerCard: some View {
        return VStack(alignment: .leading, spacing: isLandscape ? 4 : LumiSpacing.small) {
            playerHeader

            RemoteWaveform(
                player: player,
                visibleBeats: effectiveVisibleBars * 4,
                inspectionStartBeat: isMaster ? nil : inspectionStartBeat,
                dragTranslation: isMaster ? 0 : dragTranslation,
                isMaster: isMaster
            )
                .frame(height: isLandscape ? nil : 96)
                .frame(
                    minHeight: isLandscape ? 102 : nil,
                    maxHeight: isLandscape ? .infinity : nil
                )
                .contentShape(Rectangle())
                .gesture(waveformGestures)
                .overlay(alignment: .bottomTrailing) {
                    if !isMaster, inspectionStartBeat != nil {
                        Button("Follow Live") {
                            inspectionStartBeat = nil
                        }
                        .font(LumiTypography.caption.weight(.semibold))
                        .buttonStyle(.borderedProminent)
                        .tint(LumiColor.accent)
                        .padding(6)
                    }
                }

            movingPlanBands
        }
        .padding(isLandscape ? LumiSpacing.small : LumiSpacing.medium)
        .frame(maxHeight: isLandscape ? .infinity : nil, alignment: .top)
        .background(LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.panel)
                .stroke(isMaster ? operationColor(operationState) : LumiColor.border, lineWidth: isMaster ? 2 : 1)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Player \(player.playerNumber), \(player.hardwareModel ?? "Pro DJ Link Player")")
        .accessibilityAction(named: "Adjust next phrase") {
            guard let cue = plan?.cues.first(where: {
                $0.startBeat > player.transport.beat
            }) else { return }
            onSelectCue(cue)
        }
        .onChange(of: isMaster) { _, newValue in
            inspectionStartBeat = nil
            manualZoomBars = newValue ? 40 : nil
        }
    }

    private var movingPlanBands: some View {
        TimelineView(
            .animation(
                minimumInterval: 1.0 / 30.0,
                paused: !RemotePlanTimelineCadence.shouldAnimate(
                    isMaster: isMaster,
                    isPlaying: player.transport.playing
                )
            )
        ) { timeline in
            let viewport = beatViewport(at: timeline.date)
            VStack(alignment: .leading, spacing: isLandscape ? 4 : LumiSpacing.small) {
                RemotePhraseBand(
                    player: player,
                    plan: plan,
                    viewport: viewport,
                    onSelectCue: onSelectCue
                )
                .frame(height: isLandscape ? 20 : 18)

                if let plan {
                    RemotePlanBand(
                        player: player,
                        plan: plan,
                        viewport: viewport,
                        controlsEnabled: controlsEnabled,
                        onSelectCue: onSelectCue
                    )
                    .frame(height: isLandscape ? 46 : 54)
                } else {
                    Text("Waiting for Light Plan")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                        .frame(maxWidth: .infinity, minHeight: isLandscape ? 46 : 54)
                        .background(LumiColor.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
                }
            }
        }
    }

    @ViewBuilder
    private var playerHeader: some View {
        if isLandscape {
            HStack(alignment: .center, spacing: 8) {
                VStack(alignment: .leading, spacing: 1) {
                    Text("PLAYER \(player.playerNumber)")
                        .font(LumiTypography.technical.weight(.bold))
                    Text(player.hardwareModel ?? "Pro DJ Link Player")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                        .lineLimit(1)
                }
                .frame(width: 72, alignment: .leading)

                trackIdentity
                    .frame(maxWidth: .infinity, alignment: .leading)

                VStack(alignment: .trailing, spacing: 1) {
                    Text(String(format: "%.1f BPM", Double(player.transport.effectiveBPMMilli) / 1_000))
                        .font(LumiTypography.technical.weight(.semibold))
                    Text(metadataLabel)
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                        .lineLimit(1)
                }

                roleBadge
                    .frame(width: 62, alignment: .trailing)
            }
            .frame(height: 38)
        } else {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("PLAYER \(player.playerNumber)")
                        .font(LumiTypography.technical.weight(.bold))
                    Text(player.hardwareModel ?? "Pro DJ Link Player")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                roleBadge
            }

            HStack(alignment: .firstTextBaseline) {
                trackIdentity
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(String(format: "%.1f BPM", Double(player.transport.effectiveBPMMilli) / 1_000))
                        .font(LumiTypography.technical.weight(.semibold))
                    Text(metadataLabel)
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
    }

    private var trackIdentity: some View {
        HStack(alignment: .firstTextBaseline, spacing: isLandscape ? 5 : LumiSpacing.small) {
            if let trackColor = player.track.colorRGB {
                Circle()
                    .fill(rgbColor(trackColor))
                    .frame(width: 9, height: 9)
                    .accessibilityLabel("Track color")
            }
            VStack(alignment: .leading, spacing: 1) {
                Text(player.track.title)
                    .font(isLandscape ? LumiTypography.sectionTitle : LumiTypography.cardTitle)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
                Text(player.track.artist)
                    .font(isLandscape ? LumiTypography.caption : LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
            }
        }
    }

    private var roleBadge: some View {
        VStack(alignment: .trailing, spacing: 1) {
            Text(isMaster ? "MASTER" : "PLAN READY")
                .font(LumiTypography.technical.weight(.bold))
                .foregroundStyle(isMaster ? operationColor(operationState) : LumiColor.accent)
                .lineLimit(1)
            if isMaster, operationState == .live {
                Text("LIVE NOW")
                    .font(LumiTypography.caption.weight(.bold))
                    .foregroundStyle(LumiColor.success)
                    .lineLimit(1)
            }
        }
    }

    private var metadataLabel: String {
        let key = player.track.key.isEmpty ? "Key —" : player.track.key
        guard let duration = player.track.beatGrid?.durationMillis,
              let position = player.transport.positionMillis else {
            return key
        }
        let remaining = duration > position ? duration - position : 0
        return "\(key) · −\(formatDuration(remaining))"
    }

    private var effectiveVisibleBars: Double {
        let totalBars = max(1, Double(player.track.durationBeats) / 4)
        return min(totalBars, max(2, baseVisibleBars / Double(magnification)))
    }

    private var baseVisibleBars: Double {
        let totalBars = max(1, Double(player.track.durationBeats) / 4)
        let automatic = RemoteWaveformViewportMath.automaticVisibleBars(
            isMaster: isMaster,
            totalBars: totalBars
        )
        return manualZoomBars ?? automatic
    }

    private func beatViewport(at date: Date) -> RemoteBeatViewport {
        let total = max(1, Double(player.track.durationBeats))
        let visible = min(total, effectiveVisibleBars * 4)
        let visualBeat = RemoteTransportInterpolation.visualBeat(
            player: player,
            atUnixMillis: UInt64(max(0, date.timeIntervalSince1970 * 1_000))
        )
        let start = RemoteWaveformViewportMath.resolvedStartBeat(
            currentBeat: visualBeat,
            visibleBeats: visible,
            totalBeats: total,
            isMaster: isMaster,
            inspectionStartBeat: inspectionStartBeat,
            dragTranslation: Double(dragTranslation)
        )
        let playheadFraction = isMaster
            ? (visualBeat - start) / visible
            : nil
        return RemoteBeatViewport(
            startBeat: start,
            endBeat: start + visible,
            totalBeats: total,
            currentBeat: visualBeat,
            playheadFraction: playheadFraction
        )
    }

    private var waveformGestures: some Gesture {
        SimultaneousGesture(
            DragGesture(minimumDistance: 5)
                .updating($dragTranslation) { value, state, _ in
                    guard !isMaster else { return }
                    state = value.translation.width
                }
                .onEnded { value in
                    guard !isMaster else { return }
                    let visible = effectiveVisibleBars * 4
                    let total = max(1, Double(player.track.durationBeats))
                    let current = inspectionStartBeat
                        ?? automaticViewportStart(
                            currentBeat: Double(player.transport.beat),
                            visibleBeats: visible,
                            totalBeats: total
                        )
                    inspectionStartBeat = RemoteWaveformViewportMath.clampedStartBeat(
                        current - Double(value.translation.width) * visible / 320,
                        visibleBeats: visible,
                        totalBeats: total
                    )
                },
            MagnifyGesture()
                .updating($magnification) { value, state, _ in
                    state = value.magnification
                }
                .onEnded { value in
                    // GestureState still contains the live magnification while
                    // onEnded executes. Commit from the stable pre-gesture
                    // viewport once, otherwise every pinch is applied twice.
                    let totalBars = max(1, Double(player.track.durationBeats) / 4)
                    manualZoomBars = RemoteWaveformViewportMath.committedVisibleBars(
                        baseVisibleBars: baseVisibleBars,
                        magnification: Double(value.magnification),
                        totalBars: totalBars
                    )
            }
        )
    }

    private func automaticViewportStart(
        currentBeat: Double,
        visibleBeats: Double,
        totalBeats: Double
    ) -> Double {
        RemoteWaveformViewportMath.automaticStartBeat(
            currentBeat: currentBeat,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            isMaster: isMaster
        )
    }

}

private struct RemoteEmptyPlayerSurface: View {
    let playerNumber: UInt8
    let isLandscape: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            HStack {
                VStack(alignment: .leading, spacing: 1) {
                    Text("PLAYER \(playerNumber)")
                        .font(LumiTypography.technical.weight(.bold))
                    Text("Pro DJ Link Player")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Text("WAITING")
                    .font(LumiTypography.technical.weight(.bold))
                    .foregroundStyle(LumiColor.textSecondary)
            }

            ContentUnavailableView(
                "Waiting for track",
                systemImage: "cable.connector",
                description: Text("Load a track on Player \(playerNumber).")
            )
            .foregroundStyle(LumiColor.textSecondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .padding(isLandscape ? LumiSpacing.small : LumiSpacing.medium)
        .frame(
            maxWidth: .infinity,
            minHeight: isLandscape ? 202 : 248,
            maxHeight: isLandscape ? .infinity : nil,
            alignment: .top
        )
        .background(LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.panel)
                .stroke(LumiColor.border, lineWidth: 1)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Player \(playerNumber), waiting for a connected track")
    }
}

private struct RemoteWaveform: View {
    let player: RemotePlayer
    let visibleBeats: Double
    let inspectionStartBeat: Double?
    let dragTranslation: CGFloat
    let isMaster: Bool
#if os(iOS)
    @State private var rasterImage: CGImage?
    @State private var renderedContentKey: RemoteWaveformRasterContentKey?
#endif

    var body: some View {
#if os(iOS)
        GeometryReader { _ in
            ZStack {
                Color.black
                if let rasterImage {
                    RemoteWaveformLayerView(
                        rasterImage: rasterImage,
                        player: player,
                        visibleBeats: visibleBeats,
                        inspectionStartBeat: inspectionStartBeat,
                        dragTranslation: dragTranslation,
                        isMaster: isMaster
                    )
                }
            }
            .task(id: rasterKey) {
                let requestedKey = rasterKey
                if renderedContentKey != requestedKey.contentKey {
                    // A previous track must never flash while the next RGB
                    // raster is prepared. The black backing remains stable.
                    rasterImage = nil
                    renderedContentKey = nil
                }
                let points = player.track.waveform
                let track = player.track
                let rendered = await Task.detached(priority: .utility) {
                    Self.makeRasterImage(points: points, track: track)
                }.value
                guard !Task.isCancelled, requestedKey == rasterKey else { return }
                rasterImage = rendered
                renderedContentKey = requestedKey.contentKey
            }
        }
        .background(.black)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
#else
        let viewport = currentViewport
        Canvas { context, size in
            let points = player.track.waveform
            guard !points.isEmpty else { return }
            let visibleCount = max(1, Int(ceil(size.width)))
            for column in 0 ..< visibleCount {
                let fraction = Double(column) / Double(max(1, visibleCount - 1))
                let beat = viewport.startBeat + fraction * viewport.visibleBeats
                guard beat >= 0, beat <= viewport.totalBeats else { continue }
                let waveformFraction: Double
                if let beatGrid = player.track.beatGrid {
                    let time = timeMillisFor(beat: beat, in: player.track)
                    waveformFraction = Double(time) / Double(max(1, beatGrid.durationMillis))
                } else {
                    waveformFraction = beat / viewport.totalBeats
                }
                guard let sample = RemoteWaveformSampling.sample(
                    points: points,
                    trackProgress: waveformFraction
                ) else { continue }
                let amplitude = CGFloat(sample.amplitude) * size.height * 0.43
                let x = CGFloat(column) + 0.5
                var line = Path()
                line.move(to: CGPoint(x: x, y: size.height / 2 - amplitude))
                line.addLine(to: CGPoint(x: x, y: size.height / 2 + amplitude))
                let color = Color(
                    red: sample.red,
                    green: sample.green,
                    blue: sample.blue
                ).opacity(0.98)
                context.stroke(line, with: .color(color), lineWidth: 1)
            }
            drawBeatgrid(context: context, size: size)
            if isMaster, let fraction = viewport.playheadFraction,
               (0 ... 1).contains(fraction) {
                let playhead = CGRect(
                    x: CGFloat(fraction) * size.width - 1,
                    y: 0,
                    width: 2,
                    height: size.height
                )
                context.fill(Path(playhead), with: .color(.white))
            }
        }
        .background(.black)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
        .overlay(alignment: .topLeading) {
            GeometryReader { geometry in
                ForEach(player.track.hotCues) { cue in
                    let beat = beatFor(timeMillis: cue.timeMillis, in: player.track)
                    if viewport.contains(beat) {
                        let x = viewport.xFraction(for: beat) * geometry.size.width
                        Text(hotCueLetter(cue.index))
                            .font(LumiTypography.hotCueLetter)
                            .foregroundStyle(rgbColor(cue.colorRGB))
                            .position(x: x, y: 8)
                            .accessibilityLabel("Hot Cue \(hotCueLetter(cue.index))")
                    }
                }
            }
        }
#endif
    }

#if !os(iOS)
    private var currentViewport: RemoteBeatViewport {
        let total = max(1, Double(player.track.durationBeats))
        let visible = min(total, max(1, visibleBeats))
        let visualBeat = RemoteTransportInterpolation.visualBeat(
            player: player,
            atUnixMillis: UInt64(max(0, Date().timeIntervalSince1970 * 1_000))
        )
        let start = RemoteWaveformViewportMath.resolvedStartBeat(
            currentBeat: visualBeat,
            visibleBeats: visible,
            totalBeats: total,
            isMaster: isMaster,
            inspectionStartBeat: inspectionStartBeat,
            dragTranslation: Double(dragTranslation)
        )
        return RemoteBeatViewport(
            startBeat: start,
            endBeat: start + visible,
            totalBeats: total,
            currentBeat: visualBeat,
            playheadFraction: isMaster ? RemoteWaveformViewportMath.livePlayheadFraction : nil
        )
    }

    private func drawBeatgrid(context: GraphicsContext, size: CGSize) {
        let viewport = currentViewport
        let first = Int(floor(viewport.startBeat))
        let last = Int(ceil(viewport.endBeat))
        guard last - first <= 512 else { return }
        for beat in first ... last where beat >= 0 {
            let fraction = viewport.xFraction(for: Double(beat))
            guard (0 ... 1).contains(fraction) else { continue }
            let isBar = beat % 4 == 0
            let line = CGRect(
                x: fraction * size.width,
                y: 0,
                width: isBar ? 1.2 : 0.6,
                height: size.height
            )
            context.fill(
                Path(line),
                with: .color(.white.opacity(isBar ? 0.28 : 0.12))
            )
        }
    }
#endif

#if os(iOS)
    private var rasterKey: RemoteWaveformRasterKey {
        RemoteWaveformRasterKey(player: player)
    }

    nonisolated private static func makeRasterImage(
        points: [RemoteWaveformPoint],
        track: RemoteTrack
    ) -> CGImage? {
        guard !points.isEmpty else { return nil }
        // Remain below iPhone GPU texture limits while retaining substantially
        // more detail than one on-screen column per sample. Zooming translates
        // this stable full-track raster instead of repainting every frame.
        let width = min(16_384, max(points.count, 8_192))
        let height = 192
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }

        let center = Double(height) / 2
        let maximumAmplitude = Double(height) * 0.43
        context.setLineWidth(1)
        for pixel in 0 ..< width {
            if pixel.isMultiple(of: 256), Task.isCancelled { return nil }
            let beat = Double(pixel) / Double(max(1, width - 1))
                * Double(max(1, track.durationBeats))
            let trackProgress: Double
            if let beatGrid = track.beatGrid {
                trackProgress = Double(timeMillisFor(beat: beat, in: track))
                    / Double(max(1, beatGrid.durationMillis))
            } else {
                trackProgress = beat / Double(max(1, track.durationBeats))
            }
            guard let sample = RemoteWaveformSampling.sample(
                points: points,
                trackProgress: trackProgress
            ) else { continue }
            let amplitude = sample.amplitude * maximumAmplitude
            context.setStrokeColor(
                red: sample.red,
                green: sample.green,
                blue: sample.blue,
                alpha: 0.98
            )
            let x = Double(pixel) + 0.5
            context.move(to: CGPoint(x: x, y: center - amplitude))
            context.addLine(to: CGPoint(x: x, y: center + amplitude))
            context.strokePath()
        }

        let beatsPerBar = Int(max(1, track.beatGrid?.beatsPerBar ?? 4))
        for beat in 0 ... Int(max(1, track.durationBeats)) {
            let x = Double(beat) / Double(max(1, track.durationBeats))
                * Double(width)
            let isBar = beat.isMultiple(of: beatsPerBar)
            context.setLineWidth(isBar ? 1.2 : 0.6)
            context.setStrokeColor(
                red: 1,
                green: 1,
                blue: 1,
                alpha: isBar ? 0.30 : 0.09
            )
            context.move(to: CGPoint(x: x, y: 0))
            context.addLine(to: CGPoint(x: x, y: Double(height)))
            context.strokePath()
        }
        return context.makeImage()
    }
#endif
}

#if os(iOS)
private struct RemoteWaveformRasterContentKey: Hashable {
    let trackLoadID: UInt64
    let trackID: UInt64?
    let pointCount: Int
    let firstPoint: UInt32?
    let lastPoint: UInt32?
}

private struct RemoteWaveformRasterKey: Hashable {
    let contentKey: RemoteWaveformRasterContentKey
    let durationBeats: UInt64
    let beatGridCount: Int
    let firstBeatTimeMillis: UInt64?
    let lastBeatTimeMillis: UInt64?

    init(player: RemotePlayer) {
        func packed(_ point: RemoteWaveformPoint?) -> UInt32? {
            point.map {
                UInt32($0.low) << 16 | UInt32($0.mid) << 8 | UInt32($0.high)
            }
        }
        contentKey = RemoteWaveformRasterContentKey(
            trackLoadID: player.trackLoadID,
            trackID: player.track.trackID,
            pointCount: player.track.waveform.count,
            firstPoint: packed(player.track.waveform.first),
            lastPoint: packed(player.track.waveform.last)
        )
        durationBeats = player.track.durationBeats
        beatGridCount = player.track.beatGrid?.timesMillis.count ?? 0
        firstBeatTimeMillis = player.track.beatGrid?.timesMillis.first
        lastBeatTimeMillis = player.track.beatGrid?.timesMillis.last
    }
}

private struct RemoteWaveformLayerView: UIViewRepresentable {
    let rasterImage: CGImage
    let player: RemotePlayer
    let visibleBeats: Double
    let inspectionStartBeat: Double?
    let dragTranslation: CGFloat
    let isMaster: Bool

    func makeUIView(context: Context) -> RemoteWaveformLayerHostView {
        RemoteWaveformLayerHostView()
    }

    func updateUIView(_ uiView: RemoteWaveformLayerHostView, context: Context) {
        uiView.update(
            rasterImage: rasterImage,
            player: player,
            visibleBeats: visibleBeats,
            inspectionStartBeat: inspectionStartBeat,
            dragTranslation: dragTranslation,
            isMaster: isMaster
        )
    }

    static func dismantleUIView(_ uiView: RemoteWaveformLayerHostView, coordinator: ()) {
        uiView.stopRendering()
    }
}

private final class RemoteWaveformLayerHostView: UIView {
    private let waveformLayer = CALayer()
    private let playheadLayer = CALayer()
    private let playheadCapLayer = CALayer()
    // UIKit guarantees this view's lifetime work happens on the main thread.
    // Swift 6 treats deinit as nonisolated, so keep the display-link reference
    // explicitly unsafe-nonisolated while still touching it only from UIKit.
    nonisolated(unsafe) private var displayLink: CADisplayLink?
    private var rasterImage: CGImage?
    private var player: RemotePlayer?
    private var visibleBeats = 1.0
    private var inspectionStartBeat: Double?
    private var dragTranslation: CGFloat = 0
    private var isMaster = false
    private var hotCues: [RemoteHotCue] = []
    private var hotCueLayers: [(line: CALayer, badge: CATextLayer)] = []
    private var appliedBoundsSize = CGSize.zero
    private var staticGeometryNeedsUpdate = true

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        backgroundColor = .black
        layer.masksToBounds = true
        waveformLayer.anchorPoint = .zero
        waveformLayer.contentsGravity = .resize
        waveformLayer.magnificationFilter = .linear
        waveformLayer.minificationFilter = .linear
        playheadLayer.anchorPoint = .zero
        playheadLayer.backgroundColor = UIColor.white.cgColor
        playheadLayer.shadowColor = UIColor.black.cgColor
        playheadLayer.shadowOpacity = 0.5
        playheadLayer.shadowRadius = 1
        playheadCapLayer.anchorPoint = .zero
        playheadCapLayer.backgroundColor = UIColor.white.cgColor
        layer.addSublayer(waveformLayer)
        layer.addSublayer(playheadLayer)
        layer.addSublayer(playheadCapLayer)

        let displayLink = CADisplayLink(target: self, selector: #selector(renderFrame))
        let nativeMaximum = Float(max(60, UIScreen.main.maximumFramesPerSecond))
        displayLink.preferredFrameRateRange = CAFrameRateRange(
            minimum: 30,
            maximum: nativeMaximum,
            preferred: nativeMaximum
        )
        displayLink.add(to: .main, forMode: .common)
        displayLink.isPaused = true
        self.displayLink = displayLink
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(applicationDidBecomeActive),
            name: UIApplication.didBecomeActiveNotification,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(applicationWillResignActive),
            name: UIApplication.willResignActiveNotification,
            object: nil
        )
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        nil
    }

    deinit {
        displayLink?.invalidate()
        NotificationCenter.default.removeObserver(self)
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        updateRenderingState()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard abs(appliedBoundsSize.width - bounds.width) > 0.5
                || abs(appliedBoundsSize.height - bounds.height) > 0.5 else {
            return
        }
        appliedBoundsSize = bounds.size
        staticGeometryNeedsUpdate = true
        renderFrame()
    }

    func update(
        rasterImage: CGImage,
        player: RemotePlayer,
        visibleBeats: Double,
        inspectionStartBeat: Double?,
        dragTranslation: CGFloat,
        isMaster: Bool
    ) {
        let imageChanged = self.rasterImage !== rasterImage
        let hotCuesChanged = hotCues != player.track.hotCues
        let transportDiscontinuity = self.player?.trackLoadID != player.trackLoadID
            || self.player?.transport.discontinuityRevision
                != player.transport.discontinuityRevision
            || self.player?.transport.playing != player.transport.playing
        let geometryChanged = imageChanged
            || hotCuesChanged
            || self.visibleBeats != max(1, visibleBeats)
            || self.isMaster != isMaster
            || self.player?.track.durationBeats != player.track.durationBeats
        self.rasterImage = rasterImage
        self.player = player
        self.visibleBeats = max(1, visibleBeats)
        self.inspectionStartBeat = inspectionStartBeat
        self.dragTranslation = dragTranslation
        self.isMaster = isMaster
        if imageChanged {
            waveformLayer.contents = rasterImage
            waveformLayer.contentsScale = UIScreen.main.scale
        }
        if hotCuesChanged {
            hotCues = player.track.hotCues
            rebuildHotCueLayers()
        }
        staticGeometryNeedsUpdate = staticGeometryNeedsUpdate || geometryChanged
        updateRenderingState()
        // Continuous network anchors only replace interpolation evidence. The
        // display link consumes them on the next VSync; an asynchronous socket
        // callback must not insert an extra, off-cycle layer movement.
        if geometryChanged || transportDiscontinuity || !isMaster || !player.transport.playing {
            renderFrame()
        }
    }

    func stopRendering() {
        displayLink?.invalidate()
        displayLink = nil
    }

    @objc private func applicationDidBecomeActive() {
        updateRenderingState()
        renderFrame()
    }

    @objc private func applicationWillResignActive() {
        displayLink?.isPaused = true
    }

    private func updateRenderingState() {
        // Prepared Players render a fixed overview. Only the live Master needs
        // a display-rate clock, so a two-Player transition never doubles the
        // waveform animation work.
        displayLink?.isPaused = window == nil
            || !isMaster
            || player?.transport.playing != true
    }

    @objc private func renderFrame() {
        guard let player, bounds.width > 0, bounds.height > 0 else { return }
        let totalBeats = max(1, Double(player.track.durationBeats))
        let visibleBeats = min(totalBeats, self.visibleBeats)
        let currentBeat = RemoteTransportInterpolation.visualBeat(
            player: player,
            atUnixMillis: UInt64(max(0, Date().timeIntervalSince1970 * 1_000))
        )
        let startBeat = RemoteWaveformViewportMath.resolvedStartBeat(
            currentBeat: currentBeat,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            isMaster: isMaster,
            inspectionStartBeat: inspectionStartBeat,
            dragTranslation: Double(dragTranslation)
        )
        let width = bounds.width
        let height = bounds.height
        let fullTrackWidth = width * CGFloat(totalBeats / visibleBeats)
        let waveformX = -width * CGFloat(startBeat / visibleBeats)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        updateStaticGeometryIfNeeded(
            fullTrackWidth: fullTrackWidth,
            height: height,
            playheadX: width * CGFloat(RemoteWaveformViewportMath.livePlayheadFraction),
            track: player.track
        )
        waveformLayer.position = CGPoint(x: waveformX, y: 0)
        CATransaction.commit()
    }

    private func updateStaticGeometryIfNeeded(
        fullTrackWidth: CGFloat,
        height: CGFloat,
        playheadX: CGFloat,
        track: RemoteTrack
    ) {
        guard staticGeometryNeedsUpdate else { return }
        staticGeometryNeedsUpdate = false
        waveformLayer.bounds = CGRect(x: 0, y: 0, width: fullTrackWidth, height: height)
        layoutHotCueLayers(
            fullTrackWidth: fullTrackWidth,
            height: height,
            track: track
        )
        playheadLayer.isHidden = !isMaster
        playheadCapLayer.isHidden = !isMaster
        playheadLayer.bounds = CGRect(x: 0, y: 0, width: 2, height: height)
        playheadLayer.position = CGPoint(x: playheadX - 1, y: 0)
        playheadCapLayer.bounds = CGRect(x: 0, y: 0, width: 7, height: 7)
        playheadCapLayer.position = CGPoint(x: playheadX - 3.5, y: 0)
    }

    private func rebuildHotCueLayers() {
        hotCueLayers.forEach {
            $0.line.removeFromSuperlayer()
            $0.badge.removeFromSuperlayer()
        }
        hotCueLayers = hotCues.map { cue in
            let color = UIColor(
                red: CGFloat((cue.colorRGB >> 16) & 0xff) / 255,
                green: CGFloat((cue.colorRGB >> 8) & 0xff) / 255,
                blue: CGFloat(cue.colorRGB & 0xff) / 255,
                alpha: 1
            )
            let line = CALayer()
            line.anchorPoint = .zero
            line.backgroundColor = color.withAlphaComponent(0.72).cgColor
            let badge = CATextLayer()
            badge.anchorPoint = .zero
            badge.string = hotCueLetter(cue.index)
            badge.font = UIFont.monospacedSystemFont(ofSize: 9, weight: .semibold)
            badge.fontSize = 9
            badge.alignmentMode = .center
            badge.foregroundColor = UIColor.black.withAlphaComponent(0.82).cgColor
            badge.backgroundColor = color.cgColor
            badge.cornerRadius = 3
            badge.contentsScale = UIScreen.main.scale
            waveformLayer.addSublayer(line)
            waveformLayer.addSublayer(badge)
            return (line, badge)
        }
    }

    private func layoutHotCueLayers(
        fullTrackWidth: CGFloat,
        height: CGFloat,
        track: RemoteTrack
    ) {
        for (index, layers) in hotCueLayers.enumerated() where index < hotCues.count {
            let cue = hotCues[index]
            let beat = beatFor(timeMillis: cue.timeMillis, in: track)
            let x = fullTrackWidth * CGFloat(beat / max(1, Double(track.durationBeats)))
            layers.line.bounds = CGRect(x: 0, y: 0, width: 1, height: height)
            layers.line.position = CGPoint(x: x - 0.5, y: 0)
            layers.badge.bounds = CGRect(x: 0, y: 0, width: 16, height: 16)
            layers.badge.position = CGPoint(x: x - 8, y: 1)
        }
    }
}
#endif

enum RemotePlanTimelineCadence {
    /// Only the live Master needs an independent display clock. Planned decks
    /// still refresh from their bounded transport projections, but do not add
    /// a second 30 Hz SwiftUI timeline while both players are running.
    static func shouldAnimate(isMaster: Bool, isPlaying: Bool) -> Bool {
        isMaster && isPlaying
    }
}

private struct RemotePhraseBand: View {
    let player: RemotePlayer
    let plan: RemoteLightPlan?
    let viewport: RemoteBeatViewport
    let onSelectCue: (RemotePlanCue) -> Void

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                ForEach(player.track.phrases) { phrase in
                    if viewport.intersects(start: Double(phrase.startBeat), end: Double(phrase.endBeat)) {
                        let clippedStart = max(viewport.startBeat, Double(phrase.startBeat))
                        let clippedEnd = min(viewport.endBeat, Double(phrase.endBeat))
                        let x = viewport.xFraction(for: clippedStart) * geometry.size.width
                        let width = (clippedEnd - clippedStart) / viewport.visibleBeats * geometry.size.width
                        ZStack(alignment: .leading) {
                            Rectangle()
                                .fill(
                                    phrase.colorRGB.map(rgbColor)
                                        ?? LumiPhraseColorPalette.defaults.color(
                                            for: phrase.roleID ?? phrase.kind
                                        )
                                )
                            if width >= 34 {
                                Text(phrase.roleName ?? phrase.kind)
                                    .font(.system(size: 8, weight: .bold, design: .rounded))
                                    .foregroundStyle(.white.opacity(0.92))
                                    .lineLimit(1)
                                    .padding(.horizontal, 3)
                            }
                        }
                        .frame(width: max(1, width))
                        .overlay {
                            if let plan,
                               let cue = plan.cues.first(where: { $0.phraseIndex == phrase.index }) {
                                let status = RemotePlanCuePresentation.status(
                                    for: cue,
                                    in: plan.cues,
                                    currentBeat: viewport.currentBeat
                                )
                                if status == .active || status == .next {
                                    Rectangle()
                                        .fill(status.color.opacity(status == .active ? 0.17 : 0.10))
                                        .overlay {
                                            Rectangle()
                                                .stroke(status.color.opacity(0.95), lineWidth: status == .active ? 1.5 : 1)
                                        }
                                }
                            }
                        }
                        .offset(x: x)
                        .accessibilityLabel(phrase.roleName ?? phrase.kind)
                    }
                }
            }
            .contentShape(Rectangle())
            .gesture(
                SpatialTapGesture().onEnded { value in
                    guard let plan, geometry.size.width > 0 else { return }
                    let fraction = min(1, max(0, value.location.x / geometry.size.width))
                    let tappedBeat = viewport.startBeat
                        + Double(fraction) * viewport.visibleBeats
                    guard let phrase = player.track.phrases.first(where: {
                        Double($0.startBeat) <= tappedBeat && tappedBeat < Double($0.endBeat)
                    }),
                    let cue = plan.cues.first(where: { $0.phraseIndex == phrase.index }) else {
                        return
                    }
                    onSelectCue(cue)
                }
            )
        }
        .clipShape(RoundedRectangle(cornerRadius: 3))
        .accessibilityHint("Tap a phrase to inspect or adjust its future Light Plan choice")
    }
}

private struct RemotePlanBand: View {
    let player: RemotePlayer
    let plan: RemoteLightPlan
    let viewport: RemoteBeatViewport
    let controlsEnabled: Bool
    let onSelectCue: (RemotePlanCue) -> Void

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                ForEach(plan.cues) { cue in
                    if viewport.intersects(start: Double(cue.startBeat), end: Double(cue.endBeat)) {
                        let clippedStart = max(viewport.startBeat, Double(cue.startBeat))
                        let clippedEnd = min(viewport.endBeat, Double(cue.endBeat))
                        let x = viewport.xFraction(for: clippedStart) * geometry.size.width
                        let width = (clippedEnd - clippedStart) / viewport.visibleBeats * geometry.size.width
                        let status = RemotePlanCuePresentation.status(
                            for: cue,
                            in: plan.cues,
                            currentBeat: viewport.currentBeat
                        )
                        Button {
                            onSelectCue(cue)
                        } label: {
                            VStack(alignment: .leading, spacing: 1) {
                                if status == .active || status == .next {
                                    if width >= 52 {
                                        HStack(spacing: 3) {
                                            Circle()
                                                .fill(status.color)
                                                .frame(width: 5, height: 5)
                                            Text(status.label)
                                                .font(.system(size: 7, weight: .bold, design: .rounded))
                                                .foregroundStyle(status.color)
                                                .lineLimit(1)
                                            Spacer(minLength: 0)
                                            if controlsEnabled,
                                               cue.startBeat > player.transport.beat,
                                               width >= 72 {
                                                Image(systemName: "slider.horizontal.3")
                                                    .font(.system(size: 7, weight: .bold))
                                                    .foregroundStyle(LumiColor.accent)
                                            }
                                        }
                                    } else {
                                        Text(status.label)
                                            .font(.system(size: 6, weight: .bold, design: .rounded))
                                            .foregroundStyle(status.color)
                                            .lineLimit(1)
                                            .fixedSize(horizontal: true, vertical: false)
                                    }
                                }
                                Text(cue.autoloopName ?? "Hold")
                                    .font(LumiTypography.caption.weight(.semibold))
                                    .lineLimit(1)
                                Text(cue.themeName ?? plan.themeName ?? "No Theme")
                                    .font(LumiTypography.caption)
                                    .foregroundStyle(LumiColor.textSecondary)
                                    .lineLimit(1)
                            }
                            .padding(.horizontal, 5)
                            .frame(width: max(1, width), alignment: .leading)
                            .frame(maxHeight: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .background(status.backgroundColor)
                        .overlay(alignment: .top) {
                            Rectangle()
                                .fill(status.color.opacity(status.topLineOpacity))
                                .frame(height: status == .active ? 3 : status == .next ? 2 : 1)
                        }
                        .overlay(alignment: .topTrailing) {
                            if controlsEnabled,
                               cue.startBeat > player.transport.beat,
                               status != .next,
                               width >= 52 {
                                Image(systemName: "slider.horizontal.3")
                                    .font(.system(size: 7, weight: .bold))
                                    .foregroundStyle(LumiColor.accent)
                                    .padding(4)
                            }
                        }
                        .overlay {
                            if status == .active || status == .next {
                                Rectangle()
                                    .stroke(status.color.opacity(0.88), lineWidth: status == .active ? 1.5 : 1)
                            }
                        }
                        .shadow(
                            color: status.color.opacity(status == .active ? 0.34 : status == .next ? 0.20 : 0),
                            radius: status == .active ? 4 : status == .next ? 2 : 0
                        )
                        .offset(x: x)
                        .accessibilityLabel(
                            "\(status.label), phrase \(cue.phraseIndex + 1), \(cue.autoloopName ?? "hold current AutoLoop")"
                        )
                        .accessibilityHint(
                            cue.startBeat > player.transport.beat
                                ? "Tap to adjust this future phrase"
                                : "Tap to inspect this phrase"
                        )
                    }
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
    }
}

private struct RemoteBeatViewport: Equatable {
    let startBeat: Double
    let endBeat: Double
    let totalBeats: Double
    let currentBeat: Double
    let playheadFraction: Double?

    var visibleBeats: Double { max(1, endBeat - startBeat) }

    func xFraction(for beat: Double) -> CGFloat {
        CGFloat((beat - startBeat) / visibleBeats)
    }

    func contains(_ beat: Double) -> Bool {
        beat >= startBeat && beat <= endBeat
    }

    func intersects(start: Double, end: Double) -> Bool {
        end > startBeat && start < endBeat
    }
}

enum RemotePlanCueVisualStatus: Equatable {
    case completed
    case active
    case next
    case planned

    var label: String {
        switch self {
        case .completed: "DONE"
        case .active: "ACTIVE"
        case .next: "NEXT"
        case .planned: "PLANNED"
        }
    }

    var color: Color {
        switch self {
        case .active: Color(red: 0.92, green: 0.20, blue: 0.26)
        case .next: LumiColor.accent
        case .completed: LumiColor.success.opacity(0.65)
        case .planned: LumiColor.textSecondary.opacity(0.72)
        }
    }

    var backgroundColor: Color {
        switch self {
        case .active: color.opacity(0.15)
        case .next: color.opacity(0.11)
        case .completed: LumiColor.surfaceElevated.opacity(0.48)
        case .planned: LumiColor.surfaceElevated
        }
    }

    var topLineOpacity: Double {
        switch self {
        case .active, .next: 1
        case .completed: 0.32
        case .planned: 0.38
        }
    }
}

enum RemotePlanCuePresentation {
    static func status(
        for cue: RemotePlanCue,
        in cues: [RemotePlanCue],
        currentBeat: Double
    ) -> RemotePlanCueVisualStatus {
        if Double(cue.endBeat) <= currentBeat { return .completed }
        if Double(cue.startBeat) <= currentBeat && currentBeat < Double(cue.endBeat) {
            return .active
        }
        let next = cues
            .filter { Double($0.startBeat) > currentBeat }
            .min {
                if $0.startBeat == $1.startBeat {
                    return $0.phraseIndex < $1.phraseIndex
                }
                return $0.startBeat < $1.startBeat
            }
        if next?.phraseIndex == cue.phraseIndex { return .next }
        return .planned
    }
}

private struct RemotePlanCueSheet: View {
    let projection: RemoteLiveProjection
    let plan: RemoteLightPlan
    let initialCue: RemotePlanCue
    let controlsEnabled: Bool
    let actions: RemoteLiveActions
    @Environment(\.dismiss) private var dismiss

    init(
        projection: RemoteLiveProjection,
        plan: RemoteLightPlan,
        initialCue: RemotePlanCue,
        controlsEnabled: Bool,
        actions: RemoteLiveActions
    ) {
        self.projection = projection
        self.plan = plan
        self.initialCue = initialCue
        self.controlsEnabled = controlsEnabled
        self.actions = actions
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Selected phrase") {
                    LabeledContent("Phrase", value: phraseLabel(cue))
                    Menu {
                        ForEach(projection.phraseRoleOptions) { role in
                            Button {
                                actions.changePhraseRole(plan, cue, role.id)
                                dismiss()
                            } label: {
                                if role.id == currentPhraseRoleID {
                                    Label(role.name, systemImage: "checkmark")
                                } else {
                                    Text(role.name)
                                }
                            }
                            .disabled(role.id == currentPhraseRoleID)
                        }
                    } label: {
                        selectionRow("Phrase Type", value: currentPhraseRoleName)
                    }
                    .disabled(
                        !canEdit
                            || currentPhraseRoleID == nil
                            || projection.phraseRoleOptions.isEmpty
                    )
                    LabeledContent("Status", value: phraseStateLabel)
                    if let staticLookName = cue.staticLookName {
                        LabeledContent("Static Look", value: staticLookName)
                    }
                }

                Section("Light Plan") {
                    Menu {
                        ForEach(projection.themeOptions) { theme in
                            Button(theme.name) {
                                actions.selectTheme(plan, cue, theme.id)
                                dismiss()
                            }
                        }
                    } label: {
                        selectionRow(
                            "Theme / Bank",
                            value: cue.themeName ?? plan.themeName ?? "None"
                        )
                    }
                    .disabled(!canEdit)

                    Menu {
                        ForEach(cue.availableAutoloops) { autoloop in
                            Button("Bank \(autoloop.bankNumber) · \(autoloop.name)") {
                                actions.selectAutoloop(plan, cue, autoloop.number)
                                dismiss()
                            }
                        }
                    } label: {
                        selectionRow(
                            "AutoLoop",
                            value: cue.autoloopName ?? "Hold current"
                        )
                    }
                    .disabled(!canEdit || cue.availableAutoloops.isEmpty)

                    Button {
                        actions.setCueLock(plan, cue, !cue.locked)
                        dismiss()
                    } label: {
                        Label(
                            cue.locked ? "Unlock choice" : "Lock choice",
                            systemImage: cue.locked ? "lock.open" : "lock"
                        )
                    }
                    .disabled(!canEdit)
                }

                if !canEdit {
                    Section {
                        Label(editingUnavailableReason, systemImage: "info.circle")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                }
            }
            .navigationTitle("Adjust Light Plan")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private var cue: RemotePlanCue {
        initialCue
    }

    private var currentPhrase: RemotePhrase? {
        player?.track.phrases.first(where: { $0.index == cue.phraseIndex })
    }

    private var currentPhraseRoleID: String? { currentPhrase?.roleID }

    private var currentPhraseRoleName: String {
        currentPhrase?.roleName ?? currentPhrase?.kind ?? "Unmapped"
    }

    private var player: RemotePlayer? {
        projection.players.first {
            $0.playerNumber == plan.playerNumber && $0.trackLoadID == plan.trackLoadID
        }
    }

    private var canEdit: Bool {
        RemotePlanCueEditing.canEdit(
            cue: cue,
            currentBeat: player?.transport.beat,
            controlsEnabled: controlsEnabled
        )
    }

    private var phraseStateLabel: String {
        switch RemotePlanCueEditing.phase(cue: cue, currentBeat: player?.transport.beat) {
        case .unavailable: "Unavailable"
        case .completed: "Completed"
        case .live: "Live · locked"
        case .planned: cue.locked ? "Planned · locked" : "Planned · adjustable"
        }
    }

    private var editingUnavailableReason: String {
        guard controlsEnabled else {
            return "This iPhone is in Viewer mode. Transfer Controller access from Lumi on the Mac to make changes."
        }
        return "A phrase can only be adjusted before it starts. Running and completed phrases remain locked."
    }

    private func phraseLabel(_ candidate: RemotePlanCue) -> String {
        let name = player?.track.phrases.first(where: { $0.index == candidate.phraseIndex })
            .map { $0.roleName ?? $0.kind }
            ?? "Phrase"
        return "\(candidate.phraseIndex + 1) · \(name)"
    }

    private func selectionRow(_ title: String, value: String) -> some View {
        HStack {
            Text(title)
            Spacer()
            Text(value)
                .foregroundStyle(LumiColor.textSecondary)
                .lineLimit(1)
            Image(systemName: "chevron.up.chevron.down")
                .font(.caption2)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .contentShape(Rectangle())
    }
}

private struct SelectedPlanCue: Identifiable {
    let projection: RemoteLiveProjection
    let plan: RemoteLightPlan
    let cue: RemotePlanCue
    var id: String { "\(plan.planID)-\(cue.phraseIndex)-\(plan.revision)" }
}

public enum RemotePlayerOrdering {
    public static func orderedPlayers(in projection: RemoteLiveProjection) -> [RemotePlayer] {
        projection.players.sorted { left, right in
            let leftMaster = left.playerNumber == projection.leaderPlayerNumber
            let rightMaster = right.playerNumber == projection.leaderPlayerNumber
            if leftMaster != rightMaster { return leftMaster }
            return left.playerNumber < right.playerNumber
        }
    }

    static func visibleSlots(
        in projection: RemoteLiveProjection,
        isLandscape: Bool
    ) -> [RemotePlayerSlot] {
        let byNumber = Dictionary(
            uniqueKeysWithValues: projection.players.map { ($0.playerNumber, $0) }
        )
        var preferredNumbers: [UInt8] = []
        func append(_ number: UInt8?) {
            guard let number,
                  (1 ... 4).contains(number),
                  !preferredNumbers.contains(number) else { return }
            preferredNumbers.append(number)
        }

        append(projection.leaderPlayerNumber)
        append(projection.livePlan?.playerNumber)
        append(projection.nextPlan?.playerNumber)
        for number in byNumber.keys.sorted() {
            append(number)
        }
        if preferredNumbers.count == 1, let only = preferredNumbers.first {
            append(only.isMultiple(of: 2) ? only - 1 : only + 1)
        }
        for number in UInt8(1) ... UInt8(4) where preferredNumbers.count < 2 {
            append(number)
        }

        var slots = preferredNumbers.prefix(2).map {
            RemotePlayerSlot(playerNumber: $0, player: byNumber[$0])
        }
        if isLandscape {
            slots.sort { $0.playerNumber < $1.playerNumber }
        } else {
            slots.sort { left, right in
                let leftMaster = left.playerNumber == projection.leaderPlayerNumber
                let rightMaster = right.playerNumber == projection.leaderPlayerNumber
                if leftMaster != rightMaster { return leftMaster }
                return left.playerNumber < right.playerNumber
            }
        }
        return slots
    }
}

struct RemotePlayerSlot: Equatable, Identifiable {
    var id: UInt8 { playerNumber }
    let playerNumber: UInt8
    let player: RemotePlayer?
}

enum RemotePlanCuePhase: Equatable {
    case unavailable
    case completed
    case live
    case planned
}

enum RemotePlanCueEditing {
    static func phase(
        cue: RemotePlanCue,
        currentBeat: UInt64?
    ) -> RemotePlanCuePhase {
        guard let currentBeat else { return .unavailable }
        if cue.endBeat <= currentBeat { return .completed }
        if cue.startBeat <= currentBeat { return .live }
        return .planned
    }

    static func canEdit(
        cue: RemotePlanCue,
        currentBeat: UInt64?,
        controlsEnabled: Bool
    ) -> Bool {
        controlsEnabled && phase(cue: cue, currentBeat: currentBeat) == .planned
    }
}

enum RemoteWaveformSampling {
    static func sample(
        points: [RemoteWaveformPoint],
        trackProgress: Double
    ) -> LumiRGBWaveformSample? {
        guard !points.isEmpty, (0 ... 1).contains(trackProgress) else { return nil }
        let progress = trackProgress
        let position = progress * Double(max(0, points.count - 1))
        let lower = Int(position.rounded(.down))
        let upper = min(points.count - 1, lower + 1)
        let fraction = position - Double(lower)
        let first = points[lower]
        let second = points[upper]
        func mix(_ lhs: UInt8, _ rhs: UInt8) -> Double {
            (Double(lhs) + (Double(rhs) - Double(lhs)) * fraction) / 255
        }
        return LumiRGBWaveformSample(
            low: mix(first.low, second.low),
            mid: mix(first.mid, second.mid),
            high: mix(first.high, second.high)
        )
    }
}

enum RemoteWaveformViewportMath {
    static let defaultLiveVisibleBars = 40.0
    static let livePlayheadFraction = 0.22
    static let dragReferenceWidth = 320.0

    static func automaticVisibleBars(
        isMaster: Bool,
        totalBars: Double
    ) -> Double {
        let safeTotal = max(1, totalBars)
        return isMaster ? min(defaultLiveVisibleBars, safeTotal) : safeTotal
    }

    static func automaticStartBeat(
        currentBeat: Double,
        visibleBeats: Double,
        totalBeats: Double,
        isMaster: Bool
    ) -> Double {
        guard isMaster else {
            return min(
                max(0, (totalBeats - visibleBeats) / 2),
                max(0, totalBeats - min(visibleBeats, totalBeats))
            )
        }
        // Live follow deliberately permits negative and beyond-track viewport
        // bounds. The empty area renders black and keeps the playhead fixed.
        return currentBeat - visibleBeats * livePlayheadFraction
    }

    static func resolvedStartBeat(
        currentBeat: Double,
        visibleBeats: Double,
        totalBeats: Double,
        isMaster: Bool,
        inspectionStartBeat: Double?,
        dragTranslation: Double
    ) -> Double {
        if isMaster {
            // Live Players never leave follow mode. Pinch changes only the
            // visible span; the playhead remains at one physical position.
            return automaticStartBeat(
                currentBeat: currentBeat,
                visibleBeats: visibleBeats,
                totalBeats: totalBeats,
                isMaster: true
            )
        }
        let automatic = automaticStartBeat(
            currentBeat: currentBeat,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            isMaster: false
        )
        let proposed = (inspectionStartBeat ?? automatic)
            - dragTranslation * visibleBeats / dragReferenceWidth
        return clampedStartBeat(
            proposed,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats
        )
    }

    static func clampedStartBeat(
        _ value: Double,
        visibleBeats: Double,
        totalBeats: Double
    ) -> Double {
        min(max(0, value), max(0, totalBeats - min(visibleBeats, totalBeats)))
    }

    static func committedVisibleBars(
        baseVisibleBars: Double,
        magnification: Double,
        totalBars: Double
    ) -> Double {
        let safeMagnification = max(0.01, magnification)
        return min(max(1, totalBars), max(2, baseVisibleBars / safeMagnification))
    }
}

enum RemoteTransportInterpolation {
    /// Presentation-only interpolation. The bounded prediction prevents a
    /// stale connection from making the phone appear live indefinitely.
    static func visualBeat(player: RemotePlayer, atUnixMillis now: UInt64) -> Double {
        let anchor = player.transport
        guard anchor.playing else { return Double(anchor.beat) }
        let elapsedMillis = min(750, now.saturatingSubtracting(anchor.observedAtUnixMillis))
        if let positionMillis = anchor.positionMillis,
           player.track.originalBPMMilli > 0 {
            let playbackRate = Double(anchor.effectiveBPMMilli)
                / Double(player.track.originalBPMMilli)
            let predictedPosition = Double(positionMillis)
                + Double(elapsedMillis) * playbackRate
            return beatFor(
                timeMillis: UInt64(max(0, predictedPosition)),
                in: player.track
            )
        }
        return Double(anchor.beat)
            + Double(elapsedMillis) * Double(anchor.effectiveBPMMilli) / 60_000_000
    }
}

private func operationLabel(_ state: RemoteOperationState) -> String {
    switch state {
    case .off: "OFF"
    case .armed: "ARM"
    case .live: "START"
    case .paused: "PAUSE"
    }
}

private func operationColor(_ state: RemoteOperationState) -> Color {
    switch state {
    case .off: LumiColor.textPrimary
    case .armed, .paused: LumiColor.warning
    case .live: LumiColor.destructive
    }
}

private func healthColor(_ health: RemoteIntegrationHealth) -> Color {
    switch health {
    case .ready: LumiColor.success
    case .starting: LumiColor.warning
    case .degraded: LumiColor.destructive
    case .unavailable: LumiColor.textSecondary
    }
}

private func rgbColor(_ rgb: UInt32) -> Color {
    Color(
        red: Double((rgb >> 16) & 0xFF) / 255,
        green: Double((rgb >> 8) & 0xFF) / 255,
        blue: Double(rgb & 0xFF) / 255
    )
}

private func hotCueLetter(_ index: UInt8) -> String {
    UnicodeScalar(64 + Int(index)).map(String.init) ?? "?"
}

private func beatFor(timeMillis: UInt64, in track: RemoteTrack) -> Double {
    guard let beatGrid = track.beatGrid, !beatGrid.timesMillis.isEmpty else {
        let duration = max(1, track.beatGrid?.durationMillis ?? 1)
        return Double(timeMillis) / Double(duration) * Double(max(1, track.durationBeats))
    }
    let upperIndex = beatGrid.timesMillis.partitioningIndex { $0 >= timeMillis }
    if upperIndex == 0 { return 0 }
    if upperIndex >= beatGrid.timesMillis.count {
        return Double(min(upperIndex, Int(track.durationBeats)))
    }
    let lowerIndex = upperIndex - 1
    let lowerTime = beatGrid.timesMillis[lowerIndex]
    let upperTime = beatGrid.timesMillis[upperIndex]
    guard upperTime > lowerTime else { return Double(upperIndex) }
    let fraction = Double(timeMillis.saturatingSubtracting(lowerTime))
        / Double(upperTime - lowerTime)
    return min(Double(track.durationBeats), Double(lowerIndex) + fraction)
}

private func timeMillisFor(beat: Double, in track: RemoteTrack) -> UInt64 {
    guard let beatGrid = track.beatGrid, !beatGrid.timesMillis.isEmpty else { return 0 }
    let boundedBeat = min(max(0, beat), Double(max(1, track.durationBeats)))
    let lowerIndex = Int(floor(boundedBeat))
    let upperIndex = lowerIndex + 1
    guard lowerIndex < beatGrid.timesMillis.count else {
        return UInt64(
            boundedBeat / Double(max(1, track.durationBeats))
                * Double(beatGrid.durationMillis)
        )
    }
    let lowerTime = beatGrid.timesMillis[lowerIndex]
    guard upperIndex < beatGrid.timesMillis.count else { return lowerTime }
    let upperTime = beatGrid.timesMillis[upperIndex]
    let fraction = boundedBeat - Double(lowerIndex)
    return lowerTime + UInt64(Double(upperTime - lowerTime) * fraction)
}

private func formatDuration(_ milliseconds: UInt64) -> String {
    let totalSeconds = milliseconds / 1_000
    return String(format: "%d:%02d", totalSeconds / 60, totalSeconds % 60)
}

private extension RandomAccessCollection {
    func partitioningIndex(where predicate: (Element) -> Bool) -> Int {
        var lower = startIndex
        var upper = endIndex
        while lower != upper {
            let count = distance(from: lower, to: upper)
            let middle = index(lower, offsetBy: count / 2)
            if predicate(self[middle]) {
                upper = middle
            } else {
                lower = index(after: middle)
            }
        }
        return distance(from: startIndex, to: lower)
    }
}

private extension UInt64 {
    func saturatingSubtracting(_ other: UInt64) -> UInt64 {
        self >= other ? self - other : 0
    }
}
