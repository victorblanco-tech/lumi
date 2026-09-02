import LumiDesignSystem
import LumiRemoteClient
import SwiftUI

public struct RemoteLiveActions: Sendable {
    public let setOperationState: @MainActor @Sendable (RemoteOperationState) -> Void
    public let setAbletonLinkEnabled: @MainActor @Sendable (Bool) -> Void
    public let setTimingOffset: @MainActor @Sendable (Int) -> Void
    public let selectTheme: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, UInt64) -> Void
    public let selectAutoloop: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, UInt8) -> Void
    public let setCueLock: @MainActor @Sendable (RemoteLightPlan, RemotePlanCue, Bool) -> Void

    public init(
        setOperationState: @escaping @MainActor @Sendable (RemoteOperationState) -> Void,
        setAbletonLinkEnabled: @escaping @MainActor @Sendable (Bool) -> Void,
        setTimingOffset: @escaping @MainActor @Sendable (Int) -> Void,
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
            VStack(spacing: 0) {
                RemoteTopBar(model: model, actions: actions)
                Divider().overlay(LumiColor.border)
                content(isLandscape: geometry.size.width > geometry.size.height)
            }
            .background(LumiColor.canvas)
            .foregroundStyle(LumiColor.textPrimary)
            .sheet(item: $selectedPlanCue) { selection in
                RemotePlanCueSheet(
                    projection: selection.projection,
                    plan: selection.plan,
                    cue: selection.cue,
                    actions: actions
                )
                .presentationDetents([.medium])
            }
        }
    }

    @ViewBuilder
    private func content(isLandscape: Bool) -> some View {
        if let projection = model.projection {
            let players = RemotePlayerOrdering.orderedPlayers(in: projection)
            if isLandscape {
                HStack(spacing: LumiSpacing.small) {
                    ForEach(players) { player in
                        playerSurface(player, projection: projection)
                    }
                }
                .padding(LumiSpacing.small)
            } else {
                ScrollView {
                    LazyVStack(spacing: LumiSpacing.medium) {
                        ForEach(players) { player in
                            playerSurface(player, projection: projection)
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
                "Open Integrations › iPhone Remote on the Mac to approve this iPhone."
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
        projection: RemoteLiveProjection
    ) -> some View {
        let isMaster = projection.leaderPlayerNumber == player.playerNumber
        let plan = isMaster ? projection.livePlan : projection.nextPlan
        return RemotePlayerSurface(
            player: player,
            plan: plan,
            isMaster: isMaster,
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
}

private struct RemoteTopBar: View {
    @Bindable var model: RemoteSessionModel
    let actions: RemoteLiveActions
    @State private var confirmsStoppingShow = false

    var body: some View {
        VStack(spacing: LumiSpacing.small) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Lumi Remote")
                        .font(LumiTypography.sectionTitle)
                    Text(connectionLabel)
                        .font(LumiTypography.caption)
                        .foregroundStyle(connectionColor)
                }
                Spacer()
                if let integrations = model.projection?.integrations {
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
                        .frame(minHeight: 44)
                    }
                    .buttonStyle(.plain)
                    .disabled(!model.controlsEnabled)
                }
            }

            HStack(spacing: LumiSpacing.small) {
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
                            .frame(maxWidth: .infinity, minHeight: 44)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(operationColor(state))
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
                    .disabled(!model.controlsEnabled)
                }
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
                            .frame(minWidth: 54, minHeight: 44)
                    }
                    .disabled(!model.controlsEnabled)
                }
            }
        }
        .padding(.horizontal, LumiSpacing.medium)
        .padding(.vertical, LumiSpacing.small)
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
        .accessibilityElement(children: .combine)
    }

    private func linkLabel(_ status: RemoteIntegrationStatus) -> String {
        guard let bpm = status.abletonLinkBPMMilli else { return "LINK" }
        return String(format: "%.1f", Double(bpm) / 1_000)
    }

    private func offsetLabel(_ value: Int) -> String {
        value == 0 ? "0 ms" : String(format: "%+d ms", value)
    }
}

private struct RemotePlayerSurface: View {
    let player: RemotePlayer
    let plan: RemoteLightPlan?
    let isMaster: Bool
    let operationState: RemoteOperationState
    let controlsEnabled: Bool
    let onSelectCue: (RemotePlanCue) -> Void
    @State private var manualZoomBars: Double?
    @State private var inspectionStartBeat: Double?
    @GestureState private var dragTranslation: CGFloat = 0
    @GestureState private var magnification: CGFloat = 1

    var body: some View {
        let viewport = beatViewport
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("PLAYER \(player.playerNumber)")
                        .font(LumiTypography.technical.weight(.bold))
                    Text(player.hardwareModel ?? "Pro DJ Link Player")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(isMaster ? "MASTER" : "PLAN READY")
                        .font(LumiTypography.technical.weight(.bold))
                        .foregroundStyle(isMaster ? operationColor(operationState) : LumiColor.accent)
                    if isMaster, operationState == .live {
                        Text("LIVE NOW")
                            .font(LumiTypography.caption.weight(.bold))
                            .foregroundStyle(LumiColor.success)
                    }
                }
            }

            HStack(alignment: .firstTextBaseline) {
                HStack(alignment: .firstTextBaseline, spacing: LumiSpacing.small) {
                    if let trackColor = player.track.colorRGB {
                        Circle()
                            .fill(rgbColor(trackColor))
                            .frame(width: 9, height: 9)
                            .accessibilityLabel("Track color")
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        Text(player.track.title)
                            .font(LumiTypography.cardTitle)
                            .lineLimit(1)
                        Text(player.track.artist)
                            .font(LumiTypography.metadata)
                            .foregroundStyle(LumiColor.textSecondary)
                            .lineLimit(1)
                    }
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(String(format: "%.1f BPM", Double(player.transport.effectiveBPMMilli) / 1_000))
                        .font(LumiTypography.technical.weight(.semibold))
                    Text(metadataLabel)
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }

            RemoteWaveform(player: player, viewport: viewport, isMaster: isMaster)
                .frame(minHeight: 96)
                .contentShape(Rectangle())
                .gesture(waveformGestures)
                .overlay(alignment: .bottomTrailing) {
                    if inspectionStartBeat != nil {
                        Button("Follow Live") {
                            inspectionStartBeat = nil
                        }
                        .font(LumiTypography.caption.weight(.semibold))
                        .buttonStyle(.borderedProminent)
                        .tint(LumiColor.accent)
                        .padding(6)
                    }
                }

            RemotePhraseBand(player: player, viewport: viewport)
                .frame(height: 18)

            if let plan {
                RemotePlanBand(
                    player: player,
                    plan: plan,
                    viewport: viewport,
                    controlsEnabled: controlsEnabled,
                    onSelectCue: onSelectCue
                )
                .frame(height: 54)
            } else {
                Text("Waiting for Light Plan")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                    .frame(maxWidth: .infinity, minHeight: 54)
                    .background(LumiColor.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
            }
        }
        .padding(LumiSpacing.medium)
        .background(LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.panel)
                .stroke(isMaster ? operationColor(operationState) : LumiColor.border, lineWidth: isMaster ? 2 : 1)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Player \(player.playerNumber), \(player.hardwareModel ?? "Pro DJ Link Player")")
        .onChange(of: isMaster) { _, newValue in
            inspectionStartBeat = nil
            manualZoomBars = newValue ? 40 : nil
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
        let automatic = isMaster ? min(40, totalBars) : totalBars
        return manualZoomBars ?? automatic
    }

    private var beatViewport: RemoteBeatViewport {
        let total = max(1, Double(player.track.durationBeats))
        let visible = min(total, effectiveVisibleBars * 4)
        let baseStart = automaticViewportStart(visibleBeats: visible, totalBeats: total)
        let translated = -Double(dragTranslation) * visible / 320
        let proposed = (inspectionStartBeat ?? baseStart) + translated
        let start = clampedStart(proposed, visibleBeats: visible, totalBeats: total)
        let playheadFraction = isMaster
            ? (Double(player.transport.beat) - start) / visible
            : nil
        return RemoteBeatViewport(
            startBeat: start,
            endBeat: start + visible,
            totalBeats: total,
            playheadFraction: playheadFraction
        )
    }

    private var waveformGestures: some Gesture {
        SimultaneousGesture(
            DragGesture(minimumDistance: 5)
                .updating($dragTranslation) { value, state, _ in
                    state = value.translation.width
                }
                .onEnded { value in
                    let visible = effectiveVisibleBars * 4
                    let total = max(1, Double(player.track.durationBeats))
                    let current = inspectionStartBeat
                        ?? automaticViewportStart(visibleBeats: visible, totalBeats: total)
                    inspectionStartBeat = clampedStart(
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

    private func automaticViewportStart(visibleBeats: Double, totalBeats: Double) -> Double {
        let proposed = isMaster
            ? Double(player.transport.beat) - visibleBeats * 0.24
            : (totalBeats - visibleBeats) / 2
        return clampedStart(proposed, visibleBeats: visibleBeats, totalBeats: totalBeats)
    }

    private func clampedStart(
        _ value: Double,
        visibleBeats: Double,
        totalBeats: Double
    ) -> Double {
        min(max(0, value), max(0, totalBeats - min(visibleBeats, totalBeats)))
    }
}

private struct RemoteWaveform: View {
    let player: RemotePlayer
    let viewport: RemoteBeatViewport
    let isMaster: Bool

    var body: some View {
        Canvas { context, size in
            let points = player.track.waveform
            guard !points.isEmpty else { return }
            let visibleCount = max(1, min(points.count, Int(ceil(size.width))))
            let columnWidth = size.width / CGFloat(visibleCount)
            for column in 0 ..< visibleCount {
                let fraction = Double(column) / Double(max(1, visibleCount - 1))
                let beat = viewport.startBeat + fraction * viewport.visibleBeats
                let waveformFraction: Double
                if let beatGrid = player.track.beatGrid {
                    let time = timeMillisFor(beat: beat, in: player.track)
                    waveformFraction = Double(time) / Double(max(1, beatGrid.durationMillis))
                } else {
                    waveformFraction = beat / viewport.totalBeats
                }
                let index = min(
                    points.count - 1,
                    max(0, Int(waveformFraction * Double(points.count)))
                )
                let point = points[index]
                let strength = max(CGFloat(point.low), CGFloat(point.mid), CGFloat(point.high)) / 255
                let height = max(2, strength * size.height)
                let rect = CGRect(
                    x: CGFloat(column) * columnWidth,
                    y: (size.height - height) / 2,
                    width: max(1, columnWidth),
                    height: height
                )
                let color = Color(
                    red: Double(point.low) / 255,
                    green: Double(point.mid) / 255,
                    blue: Double(point.high) / 255
                )
                context.fill(Path(rect), with: .color(color))
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
    }

    private func drawBeatgrid(context: GraphicsContext, size: CGSize) {
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
}

private struct RemotePhraseBand: View {
    let player: RemotePlayer
    let viewport: RemoteBeatViewport

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                ForEach(player.track.phrases) { phrase in
                    if viewport.intersects(start: Double(phrase.startBeat), end: Double(phrase.endBeat)) {
                        let clippedStart = max(viewport.startBeat, Double(phrase.startBeat))
                        let clippedEnd = min(viewport.endBeat, Double(phrase.endBeat))
                        let x = viewport.xFraction(for: clippedStart) * geometry.size.width
                        let width = (clippedEnd - clippedStart) / viewport.visibleBeats * geometry.size.width
                        Rectangle()
                            .fill(LumiPhraseColorPalette.defaults.color(for: phrase.roleID ?? phrase.kind))
                            .frame(width: max(1, width))
                            .offset(x: x)
                            .accessibilityLabel(phrase.roleName ?? phrase.kind)
                    }
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 3))
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
                        Button {
                            onSelectCue(cue)
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(cue.autoloopName ?? "Hold")
                                    .font(LumiTypography.caption.weight(.semibold))
                                    .lineLimit(1)
                                Text(cue.themeName ?? plan.themeName ?? "No Theme")
                                    .font(LumiTypography.caption)
                                    .foregroundStyle(LumiColor.textSecondary)
                                    .lineLimit(1)
                            }
                            .padding(.horizontal, 5)
                            .frame(width: max(1, width), height: 54, alignment: .leading)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .disabled(!controlsEnabled || cue.startBeat <= player.transport.beat)
                        .background(LumiColor.surfaceElevated)
                        .overlay(alignment: .leading) {
                            Rectangle().fill(LumiColor.accent).frame(width: 1)
                        }
                        .offset(x: x)
                        .accessibilityLabel(
                            "Phrase \(cue.phraseIndex + 1), \(cue.autoloopName ?? "hold current AutoLoop")"
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

private struct RemotePlanCueSheet: View {
    let projection: RemoteLiveProjection
    let plan: RemoteLightPlan
    let cue: RemotePlanCue
    let actions: RemoteLiveActions
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Phrase \(cue.phraseIndex + 1)") {
                    LabeledContent("Theme", value: cue.themeName ?? plan.themeName ?? "None")
                    LabeledContent("AutoLoop", value: cue.autoloopName ?? "Hold current")
                    LabeledContent("Static Look", value: cue.staticLookName ?? "None")
                }
                Section("Change future plan") {
                    Menu("Theme from this phrase") {
                        ForEach(projection.themeOptions) { theme in
                            Button(theme.name) {
                                actions.selectTheme(plan, cue, theme.id)
                                dismiss()
                            }
                        }
                    }
                    Menu("AutoLoop for this phrase") {
                        ForEach(cue.availableAutoloops) { autoloop in
                            Button(autoloop.name) {
                                actions.selectAutoloop(plan, cue, autoloop.number)
                                dismiss()
                            }
                        }
                    }
                    Button(cue.locked ? "Unlock choice" : "Lock choice") {
                        actions.setCueLock(plan, cue, !cue.locked)
                        dismiss()
                    }
                }
            }
            .navigationTitle("Light Plan")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
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
}

enum RemoteWaveformViewportMath {
    static func committedVisibleBars(
        baseVisibleBars: Double,
        magnification: Double,
        totalBars: Double
    ) -> Double {
        let safeMagnification = max(0.01, magnification)
        return min(max(1, totalBars), max(2, baseVisibleBars / safeMagnification))
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
    let index = beatGrid.timesMillis.partitioningIndex { $0 >= timeMillis }
    return Double(min(index, Int(track.durationBeats)))
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
