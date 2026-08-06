import Foundation
import LumiDesignSystem
import LumiProtocol
import SwiftUI

public struct LiveWorkspaceView: View {
    private let state: LiveWorkspaceState
    private let productVersion: String
    private let allowsScrolling: Bool
    private let showsNavigation: Bool
    private let onPlanMutation: @MainActor (PlanMutationRequest) -> Void
    private let onSessionCommand: @MainActor (SessionCommandRequest) -> Void
    private let onLocalPlayback: @MainActor (LocalPlaybackRequest) -> Void
    private let localPlaybackBrowser: AnyView?
    @Binding private var appearance: AppearancePreference
    @Binding private var keyNotation: KeyNotationPreference
    @State private var selectedPhrase: UInt64 = 0
    @State private var selectedLivePhrase: UInt64?
    @State private var showsTechnicalStatus = false

    private let copy = LiveWorkspaceCopy()

    public init(
        state: LiveWorkspaceState,
        productVersion: String,
        appearance: Binding<AppearancePreference>,
        keyNotation: Binding<KeyNotationPreference>,
        allowsScrolling: Bool = true,
        showsNavigation: Bool = true,
        onPlanMutation: @escaping @MainActor (PlanMutationRequest) -> Void = { _ in },
        onSessionCommand: @escaping @MainActor (SessionCommandRequest) -> Void = { _ in },
        onLocalPlayback: @escaping @MainActor (LocalPlaybackRequest) -> Void = { _ in },
        localPlaybackBrowser: AnyView? = nil
    ) {
        self.state = state
        self.productVersion = productVersion
        self.allowsScrolling = allowsScrolling
        self.showsNavigation = showsNavigation
        self.onPlanMutation = onPlanMutation
        self.onSessionCommand = onSessionCommand
        self.onLocalPlayback = onLocalPlayback
        self.localPlaybackBrowser = localPlaybackBrowser
        _appearance = appearance
        _keyNotation = keyNotation
    }

    public var body: some View {
        Group {
            if showsNavigation {
                HStack(spacing: 0) {
                    sidebar
                    Divider()
                    mainWorkspace
                }
            } else {
                mainWorkspace
            }
        }
        .background(LumiColor.canvas)
        .frame(minWidth: 760, minHeight: 560)
        .accessibilityIdentifier("lumi.live.workspace")
        .overlay {
            if let content = state.content,
               content.sourceMode == "localPlayback",
               let leaderDeckID = content.leaderDeckID {
                LumiSpacebarMonitor {
                    onLocalPlayback(.togglePlayback(deckID: leaderDeckID))
                }
            }
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: copy.appTitle)
                    .font(LumiTypography.screenTitle)
                    .foregroundStyle(LumiColor.textPrimary)
                Text(verbatim: productVersion)
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }

            VStack(spacing: LumiSpacing.xSmall) {
                navigationRow(
                    copy.live,
                    systemImage: "waveform",
                    isSelected: true,
                    isAvailable: true
                )
                navigationRow(
                    copy.plans,
                    systemImage: "list.bullet.rectangle",
                    isSelected: false,
                    isAvailable: false
                )
                navigationRow(
                    copy.library,
                    systemImage: "music.note.list",
                    isSelected: false,
                    isAvailable: true
                )
                navigationRow(
                    copy.integrations,
                    systemImage: "cable.connector",
                    isSelected: false,
                    isAvailable: false
                )
            }

            Spacer()
            navigationRow(
                copy.settings,
                systemImage: "gearshape",
                isSelected: false,
                isAvailable: true
            )
        }
        .padding(LumiSpacing.large)
        .frame(width: 196)
        .background(LumiColor.surface)
        .accessibilityIdentifier("lumi.navigation")
    }

    @ViewBuilder
    private var mainWorkspace: some View {
        if allowsScrolling {
            ScrollView {
                workspaceContent
            }
        } else {
            workspaceContent
                .frame(maxHeight: .infinity, alignment: .top)
        }
    }

    private var workspaceContent: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            header
            if let diagnostic = state.diagnostic {
                diagnosticBanner(diagnostic)
            }
            planInteractionBanner
            sessionInteractionBanner
            deckWorkspace
        }
        .padding(LumiSpacing.large)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var header: some View {
        HStack(spacing: LumiSpacing.large) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: "\(copy.appTitle) \(copy.live)")
                    .font(LumiTypography.screenTitle)
                    .foregroundStyle(LumiColor.textPrimary)
                Text(verbatim: copy.subtitle)
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            deckSourceSelector
            technicalStatusButton
            operationControls
            if allowsScrolling {
                preferenceMenu
            } else {
                preferenceIndicator
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .layoutPriority(2)
    }

    private var technicalStatusButton: some View {
        Button {
            showsTechnicalStatus.toggle()
        } label: {
            HStack(spacing: LumiSpacing.small) {
                Circle()
                    .fill(componentState.color)
                    .frame(width: 8, height: 8)
                Text(verbatim: "Tech · \(conditionLabel)")
                    .font(LumiTypography.metadata.weight(.semibold))
            }
            .padding(.horizontal, LumiSpacing.medium)
            .frame(height: LumiControlMetric.standardHeight)
        }
        .buttonStyle(.bordered)
        .popover(isPresented: $showsTechnicalStatus, arrowEdge: .bottom) {
            technicalStatusPopover
        }
        .accessibilityIdentifier("lumi.technicalStatus.button")
    }

    private var technicalStatusPopover: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack {
                Text(verbatim: "Technical status")
                    .font(LumiTypography.sectionTitle)
                Spacer()
                StatusBadge(key(conditionLabel), state: componentState)
            }
            providerRow(copy.engine, systemImage: "cpu", presentation: state.engine)
            providerRow(
                copy.runtime,
                systemImage: "point.3.connected.trianglepath.dotted",
                presentation: state.runtime
            )
            providerRow(
                copy.deckSource,
                systemImage: "music.quarternote.3",
                presentation: state.source
            )
            providerRow(
                copy.planner,
                systemImage: "list.bullet.rectangle",
                presentation: state.planner
            )
            providerRow(
                copy.outputProvider,
                systemImage: "lightbulb.2",
                presentation: state.output
            )
            Divider()
            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                Text(verbatim: "Recent engine events")
                    .font(LumiTypography.metadata.weight(.semibold))
                if let entries = state.content?.timeline.suffix(5), !entries.isEmpty {
                    ForEach(Array(entries)) { entry in
                        HStack(spacing: LumiSpacing.small) {
                            Text(verbatim: "#\(entry.sequence)")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                            Text(verbatim: entry.type)
                                .font(LumiTypography.metadata)
                            Spacer()
                            Text(verbatim: entry.result.uppercased())
                                .font(LumiTypography.technical.weight(.semibold))
                        }
                    }
                } else {
                    Text(verbatim: copy.waitingTimeline)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
        .padding(LumiSpacing.large)
        .frame(width: 430)
        .background(LumiColor.surface)
        .accessibilityIdentifier("lumi.technicalStatus.popover")
    }

    private var deckSourceSelector: some View {
        HStack(spacing: 2) {
            sourceModeButton("Live Decks", mode: "connectedDecks", icon: "cable.connector")
            sourceModeButton("Local Playback", mode: "localPlayback", icon: "macbook.and.iphone")
        }
        .padding(2)
        .background(LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(LumiColor.border, lineWidth: 1)
        }
        .accessibilityIdentifier("lumi.deckSource.selector")
    }

    private func sourceModeButton(_ title: String, mode: String, icon: String) -> some View {
        let selected = state.content?.sourceMode == mode
        return Button {
            sendWithRevision { .selectDeckSourceMode(mode, expectedStateRevision: $0) }
        } label: {
            Label(title, systemImage: icon)
                .font(LumiTypography.metadata.weight(.semibold))
                .padding(.horizontal, LumiSpacing.small)
                .frame(height: LumiControlMetric.standardHeight - 4)
                .background(selected ? LumiColor.accent.opacity(0.18) : Color.clear)
                .foregroundStyle(selected ? LumiColor.accent : LumiColor.textSecondary)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
        }
        .buttonStyle(.plain)
        .disabled(selected || !canSendSessionCommand)
    }

    private var operationControls: some View {
        HStack(spacing: LumiSpacing.small) {
            OperationControl(
                key(copy.arm),
                systemImage: "shield",
                isSelected: operationState == "armed",
                isEnabled: canSetOperation("armed"),
                action: { setOperation("armed") }
            )
            OperationControl(
                key(copy.start),
                systemImage: "play.fill",
                isSelected: operationState == "live",
                isEnabled: canSetOperation("live"),
                action: { setOperation("live") }
            )
            OperationControl(
                key(copy.pause),
                systemImage: "pause.fill",
                isSelected: operationState == "paused",
                isEnabled: canSetOperation("paused"),
                action: { setOperation("paused") }
            )
            OperationControl(
                key(copy.off),
                systemImage: "stop.fill",
                isSelected: operationState == "off",
                isEnabled: canSetOperation("off"),
                action: { setOperation("off") }
            )
        }
        .accessibilityIdentifier("lumi.operation.controls")
    }

    private var preferenceMenu: some View {
        Menu {
            Picker(copy.appearance, selection: $appearance) {
                ForEach(AppearancePreference.allCases) { value in
                    Text(value.titleKey).tag(value)
                }
            }
            Picker(copy.keyNotation, selection: $keyNotation) {
                ForEach(KeyNotationPreference.allCases) { value in
                    Text(value.titleKey).tag(value)
                }
            }
        } label: {
            Image(systemName: "slider.horizontal.3")
                .frame(width: LumiControlMetric.standardHeight, height: LumiControlMetric.standardHeight)
        }
        .menuStyle(.borderlessButton)
        .accessibilityLabel("Workspace preferences")
        .accessibilityIdentifier("lumi.workspace.preferences")
    }

    private var preferenceIndicator: some View {
        Image(systemName: "slider.horizontal.3")
            .foregroundStyle(LumiColor.textSecondary)
            .frame(
                width: LumiControlMetric.standardHeight,
                height: LumiControlMetric.standardHeight
            )
            .background(LumiColor.surface)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.control)
                    .stroke(LumiColor.border, lineWidth: 1)
            }
            .accessibilityLabel("Workspace preferences")
    }

    private var deckWorkspace: some View {
        Group {
            if let content = state.content {
                VStack(spacing: LumiSpacing.medium) {
                    HStack(alignment: .top, spacing: LumiSpacing.medium) {
                        ForEach([UInt64(1), 2], id: \.self) { deckID in
                            if let deck = content.decks.first(where: { $0.deckID == deckID }) {
                                let isMaster = deck.deckID == content.leaderDeckID
                                let plan = isMaster ? content.livePlan : content.plan
                                let selectedIndex = selectedPhraseIndex(
                                    deck: deck,
                                    plan: plan,
                                    isMaster: isMaster
                                )
                                LiveDeckSurface(
                                    deck: deck,
                                    isMaster: isMaster,
                                    plan: plan,
                                    musicalKey: musicalKey(for: deck),
                                    isLocalPlayback: content.sourceMode == "localPlayback",
                                    selectedPhraseIndex: selectedIndex,
                                    onSelectPhrase: { phraseIndex in
                                        if isMaster {
                                            selectedLivePhrase = phraseIndex
                                        } else {
                                            selectedPhrase = phraseIndex
                                        }
                                    },
                                    onTogglePlayback: {
                                        onLocalPlayback(.togglePlayback(deckID: deck.deckID))
                                    },
                                    onStop: {
                                        onLocalPlayback(.stop(deckID: deck.deckID))
                                    },
                                    onSeek: { progress in
                                        onLocalPlayback(.seek(deckID: deck.deckID, progress: progress))
                                    },
                                    onMakeMaster: {
                                        sendWithRevision {
                                            .setLocalPlaybackLeader(
                                                deck.deckID,
                                                expectedStateRevision: $0
                                            )
                                        }
                                    }
                                ) {
                                    if let plan {
                                        phraseEditor(
                                            plan: plan,
                                            deck: deck,
                                            options: content.planningOptions,
                                            isLive: isMaster,
                                            selectedIndex: selectedIndex
                                        )
                                    } else {
                                        heldDeckPlan(deck)
                                    }
                                }
                                .frame(maxWidth: .infinity)
                                .accessibilityIdentifier(deckID == 1 ? "lumi.deck.a" : "lumi.deck.b")
                            } else {
                                emptyDeckSurface(deckID: deckID, sourceMode: content.sourceMode)
                                    .frame(maxWidth: .infinity)
                            }
                        }
                    }
                    if content.sourceMode == "localPlayback", let localPlaybackBrowser {
                        localPlaybackBrowser
                            .frame(maxHeight: .infinity)
                            .layoutPriority(1)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
            } else {
                placeholder(copy.waitingDecks, systemImage: "waveform.badge.magnifyingglass")
            }
        }
    }

    private func phraseEditor(
        plan: PlanSnapshot,
        deck: DeckSnapshot,
        options: PlanningOptionsSnapshot,
        isLive: Bool,
        selectedIndex: UInt64?
    ) -> some View {
        let cue = selectedIndex.flatMap { index in
            plan.cues.first(where: { $0.phraseIndex == index })
        }
        let phraseState = phraseState(cue: cue, deck: deck, isLive: isLive)
        let editable = canEdit(plan) && phraseState == .planned
        return VStack(alignment: .leading, spacing: LumiSpacing.small) {
            HStack {
                Text(verbatim: isLive ? "Live phrase plan" : "Next-track phrase plan")
                    .font(LumiTypography.metadata.weight(.semibold))
                    .foregroundStyle(Color.white)
                Spacer()
                Text(verbatim: phraseState.label)
                    .font(LumiTypography.technical)
                    .foregroundStyle(phraseState.color)
            }

            if let cue {
                VStack(alignment: .leading, spacing: LumiSpacing.small) {
                    HStack(spacing: LumiSpacing.small) {
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(verbatim: "THEME FROM \(phraseTitle(cue).uppercased())")
                                .font(LumiTypography.technical)
                                .foregroundStyle(Color.white.opacity(0.46))
                            PlanSelectionControl(
                                value: themeName(cue),
                                selectedID: themeID(cue),
                                choices: options.themes.map {
                                    PlanSelectionChoice(id: $0.id, name: $0.name)
                                },
                                isEnabled: editable && themeID(cue) != nil,
                                onSelect: { selectThemeFromPhrase($0, cue: cue, plan: plan) }
                            )
                        }
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(verbatim: "AUTOLOOP")
                                .font(LumiTypography.technical)
                                .foregroundStyle(Color.white.opacity(0.46))
                            PlanSelectionControl(
                                value: autoloopName(cue),
                                selectedID: sceneID(cue),
                                choices: compatibleAutoloops(for: cue, options: options),
                                isEnabled: editable && sceneID(cue) != nil,
                                onSelect: { selectScene($0, cue: cue, plan: plan) }
                            )
                        }
                    }
                    HStack(spacing: LumiSpacing.small) {
                        Text(verbatim: "Selected: \(phraseTitle(cue)) · \(timeRange(cue, bpmMilli: deck.bpmMilli)) · applies on the phrase boundary")
                            .font(LumiTypography.technical)
                            .foregroundStyle(Color.white.opacity(0.54))
                        Spacer()
                        Button {
                            onPlanMutation(
                                .setCueLock(
                                    context: mutationContext(for: plan),
                                    phraseIndex: cue.phraseIndex,
                                    locked: !cue.locked
                                )
                            )
                        } label: {
                            Label(
                                cue.locked ? "Unpin choice" : "Pin choice",
                                systemImage: cue.locked ? "pin.slash" : "pin"
                            )
                        }
                        .buttonStyle(.bordered)
                        .disabled(!editable || themeID(cue) == nil)
                        if !isLive {
                            Button {
                                onPlanMutation(.regeneratePlan(context: mutationContext(for: plan)))
                            } label: {
                                Label(copy.regenerate, systemImage: "arrow.clockwise")
                            }
                            .buttonStyle(.bordered)
                            .disabled(!canEdit(plan))
                        }
                    }
                }
                .padding(LumiSpacing.small)
                .background(LumiColor.accent.opacity(0.07))
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
                .overlay {
                    RoundedRectangle(cornerRadius: LumiRadius.compact)
                        .stroke(LumiColor.accent.opacity(0.25), lineWidth: 1)
                }
            }
        }
        .padding(LumiSpacing.medium)
        .background(Color.white.opacity(0.025))
        .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.1)) }
        .accessibilityIdentifier(isLive ? "lumi.live.phraseEditor" : "lumi.next.phraseEditor")
    }

    private enum PhrasePlanningState: Equatable {
        case completed
        case live
        case planned

        var label: String {
            switch self {
            case .completed: "COMPLETED · READ ONLY"
            case .live: "LIVE · READ ONLY"
            case .planned: "PLANNED · EDITABLE UNTIL PHRASE START"
            }
        }

        var color: Color {
            switch self {
            case .completed: Color.white.opacity(0.46)
            case .live: LumiColor.warning
            case .planned: LumiColor.success
            }
        }
    }

    private func phraseState(
        cue: PlanCueSnapshot?,
        deck: DeckSnapshot,
        isLive: Bool
    ) -> PhrasePlanningState {
        guard isLive, let cue, let currentPhrase = deck.phraseIndex else {
            return .planned
        }
        if cue.phraseIndex < currentPhrase { return .completed }
        if cue.phraseIndex == currentPhrase { return .live }
        return .planned
    }

    private func selectedPhraseIndex(
        deck: DeckSnapshot,
        plan: PlanSnapshot?,
        isMaster: Bool
    ) -> UInt64? {
        guard let plan else { return nil }
        let requested: UInt64? = isMaster ? selectedLivePhrase : selectedPhrase
        if let requested, plan.cues.contains(where: { $0.phraseIndex == requested }) {
            return requested
        }
        if isMaster, let current = deck.phraseIndex {
            return plan.cues.first(where: { $0.phraseIndex > current })?.phraseIndex ?? current
        }
        return plan.cues.first?.phraseIndex
    }

    private func heldDeckPlan(_ deck: DeckSnapshot) -> some View {
        HStack(spacing: LumiSpacing.small) {
            Image(systemName: "pause.circle.fill")
                .foregroundStyle(LumiColor.warning)
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: deck.planEligibility == .autoHeld ? "AUTO HELD" : copy.waitingPlan)
                    .font(LumiTypography.metadata.weight(.semibold))
                    .foregroundStyle(Color.white)
                Text(verbatim: "No complete mapped Lumi phrase timeline is available. The current look is held; manual MIDI remains available.")
                    .font(LumiTypography.technical)
                    .foregroundStyle(Color.white.opacity(0.54))
            }
        }
        .padding(LumiSpacing.medium)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.1)) }
    }

    private func emptyDeckSurface(deckID: UInt64, sourceMode: String) -> some View {
        VStack(spacing: LumiSpacing.medium) {
            Text(verbatim: deckID == 1 ? "DECK A" : "DECK B")
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(LumiColor.accent)
            Image(systemName: sourceMode == "localPlayback" ? "music.note.list" : "cable.connector")
                .font(LumiTypography.screenTitle)
                .foregroundStyle(LumiColor.textSecondary)
            Text(verbatim: sourceMode == "localPlayback" ? "No Library track loaded" : "Waiting for connected deck")
                .font(LumiTypography.cardTitle)
                .foregroundStyle(Color.white)
            Text(verbatim: sourceMode == "localPlayback" ? "Load a track onto this deck from Library." : "Beat Link Trigger connection will appear here without changing the Live workflow.")
                .font(LumiTypography.metadata)
                .foregroundStyle(Color.white.opacity(0.54))
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, minHeight: 330)
        .padding(LumiSpacing.large)
        .background(Color.black)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.panel)
                .stroke(LumiColor.border, lineWidth: 1)
        }
        .accessibilityIdentifier(deckID == 1 ? "lumi.deck.a.empty" : "lumi.deck.b.empty")
    }

    private func navigationRow(
        _ title: String,
        systemImage: String,
        isSelected: Bool,
        isAvailable: Bool
    ) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: systemImage)
                .frame(width: 18)
            Text(verbatim: title)
                .font(LumiTypography.body.weight(isSelected ? .semibold : .regular))
            Spacer()
            if !isSelected && !isAvailable {
                Text(verbatim: copy.comingSoon)
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
        }
        .foregroundStyle(isSelected ? LumiColor.textPrimary : LumiColor.textSecondary)
        .padding(.horizontal, LumiSpacing.medium)
        .frame(minHeight: LumiControlMetric.standardHeight)
        .background(isSelected ? LumiColor.accent.opacity(0.14) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    private func providerRow(
        _ name: String,
        systemImage: String,
        presentation: ProviderPresentation
    ) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: systemImage)
                .foregroundStyle(componentState(for: presentation.condition).color)
                .frame(width: 20)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: name)
                    .font(LumiTypography.body.weight(.medium))
                Text(verbatim: presentation.detail)
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .lineLimit(1)
            }
            Spacer()
            StatusBadge(
                key(providerLabel(presentation.condition)),
                state: componentState(for: presentation.condition)
            )
        }
        .accessibilityElement(children: .combine)
    }

    private func placeholder(_ message: String, systemImage: String) -> some View {
        LumiPanel {
            Label {
                Text(verbatim: message)
                    .font(LumiTypography.body)
            } icon: {
                Image(systemName: systemImage)
            }
            .foregroundStyle(LumiColor.textSecondary)
            .frame(maxWidth: .infinity, minHeight: 72, alignment: .leading)
        }
    }

    private func diagnosticBanner(_ message: String) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: componentState.systemImage)
                .foregroundStyle(componentState.color)
            Text(verbatim: message)
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textPrimary)
            Spacer()
            StatusBadge(key(conditionLabel), state: componentState)
        }
        .padding(LumiSpacing.medium)
        .background(componentState.color.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(componentState.color.opacity(0.35), lineWidth: 1)
        }
        .accessibilityIdentifier("lumi.workspace.diagnostic")
    }

    @ViewBuilder
    private var planInteractionBanner: some View {
        switch state.planInteraction {
        case .idle:
            EmptyView()
        case .submitting:
            interactionBanner(
                copy.savingPlan,
                systemImage: "arrow.triangle.2.circlepath",
                color: LumiColor.accent
            )
        case let .succeeded(message):
            interactionBanner(message, systemImage: "checkmark.circle.fill", color: .green)
        case let .rejected(message):
            interactionBanner(
                message,
                systemImage: "exclamationmark.triangle.fill",
                color: .orange
            )
        }
    }

    @ViewBuilder
    private var sessionInteractionBanner: some View {
        switch state.sessionInteraction {
        case .idle:
            EmptyView()
        case .submitting:
            interactionBanner(
                copy.applyingCommand,
                systemImage: "arrow.triangle.2.circlepath",
                color: LumiColor.accent
            )
        case let .succeeded(message):
            interactionBanner(message, systemImage: "checkmark.circle.fill", color: .green)
        case let .rejected(message):
            interactionBanner(
                message,
                systemImage: "exclamationmark.triangle.fill",
                color: .orange
            )
        }
    }

    private func interactionBanner(
        _ message: String,
        systemImage: String,
        color: Color
    ) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: systemImage)
                .foregroundStyle(color)
            Text(verbatim: message)
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textPrimary)
            Spacer()
        }
        .padding(LumiSpacing.medium)
        .background(color.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(color.opacity(0.35), lineWidth: 1)
        }
        .accessibilityIdentifier("lumi.plan.interaction")
    }

    private var keyFormatter: KeyNotationFormatter {
        KeyNotationFormatter(notation: keyNotation)
    }

    private func musicalKey(for deck: DeckSnapshot) -> String {
        guard deck.keyKnown,
              let pitchClass = pitchClass(named: deck.pitchClass),
              let mode = KeyMode(rawValue: deck.keyMode) else {
            return "—"
        }
        return keyFormatter.string(from: MusicalKey(pitchClass: pitchClass, mode: mode))
    }

    private func pitchClass(named value: String) -> PitchClass? {
        switch value {
        case "c": .c
        case "cSharp": .cSharp
        case "d": .d
        case "dSharp": .dSharp
        case "e": .e
        case "f": .f
        case "fSharp": .fSharp
        case "g": .g
        case "gSharp": .gSharp
        case "a": .a
        case "aSharp": .aSharp
        case "b": .b
        default: nil
        }
    }

    private func selectedCue(in plan: PlanSnapshot) -> PlanCueSnapshot? {
        plan.cues.first(where: { $0.phraseIndex == selectedPhrase }) ?? plan.cues.first
    }

    private func phraseTitle(_ cue: PlanCueSnapshot) -> String {
        if let roleName = cue.libraryResolution?.roleName {
            return roleName
        }
        return switch cue.reason {
        case let .phraseCategoryMatched(phraseKind, _): copy.phrase(phraseKind)
        case .missingPhraseAnalysis: copy.fallback
        }
    }

    private func timeRange(_ cue: PlanCueSnapshot, bpmMilli: UInt64) -> String {
        "\(time(cue.startBeat, bpmMilli: bpmMilli))–\(time(cue.endBeat, bpmMilli: bpmMilli))"
    }

    private func time(_ beat: UInt64, bpmMilli: UInt64) -> String {
        guard bpmMilli > 0 else { return "00:00" }
        let seconds = Int((Double(beat) * 60_000 / Double(bpmMilli)).rounded(.down))
        return String(
            format: "%02d:%02d",
            locale: Locale(identifier: "en_US_POSIX"),
            seconds / 60,
            seconds % 60
        )
    }

    private func cueSummary(_ cue: PlanCueSnapshot) -> String {
        switch cue.action {
        case let .applyLook(_, _, _, sceneName, _, loopBank, loopSlot):
            "\(sceneName) · Loop \(loopBank).\(loopSlot)"
        case .holdCurrentLook:
            copy.hold
        }
    }

    private func themeName(_ cue: PlanCueSnapshot) -> String {
        switch cue.action {
        case let .applyLook(_, themeName, _, _, _, _, _): themeName
        case .holdCurrentLook: copy.unavailable
        }
    }

    private func sceneName(_ cue: PlanCueSnapshot) -> String {
        switch cue.action {
        case let .applyLook(_, _, _, sceneName, _, _, _): sceneName
        case .holdCurrentLook: copy.hold
        }
    }

    private func autoloopName(_ cue: PlanCueSnapshot) -> String {
        cue.libraryResolution?.entryName ?? sceneName(cue)
    }

    private func mutationContext(for plan: PlanSnapshot) -> PlanMutationContext {
        PlanMutationContext(
            planID: plan.planID,
            trackLoadID: plan.trackLoadID,
            expectedPlanRevision: plan.revision
        )
    }

    private func canEdit(_ plan: PlanSnapshot) -> Bool {
        plan.status == "ready"
            && state.planInteraction != .submitting
            && state.sessionInteraction != .submitting
    }

    private var operationState: String {
        state.content?.operationState ?? "off"
    }

    private var canSendSessionCommand: Bool {
        state.content != nil
            && state.sessionInteraction != .submitting
            && state.planInteraction != .submitting
    }

    private func canSetOperation(_ target: String) -> Bool {
        guard canSendSessionCommand else { return false }
        return switch (operationState, target) {
        case ("off", "armed"), ("armed", "live"), ("paused", "live"),
             ("live", "paused"):
            true
        case (_, "off"):
            operationState != "off"
        default:
            false
        }
    }

    private func setOperation(_ target: String) {
        sendWithRevision {
            .setOperationState(target, expectedStateRevision: $0)
        }
    }

    private func sendWithRevision(
        _ request: (UInt64) -> SessionCommandRequest
    ) {
        guard let revision = state.content?.stateRevision else { return }
        onSessionCommand(request(revision))
    }

    private func selectTheme(_ themeID: UInt64, plan: PlanSnapshot) {
        onPlanMutation(
            .selectTheme(context: mutationContext(for: plan), themeID: themeID)
        )
    }

    private func selectThemeFromPhrase(
        _ themeID: UInt64,
        cue: PlanCueSnapshot,
        plan: PlanSnapshot
    ) {
        onPlanMutation(
            .selectThemeFromPhrase(
                context: mutationContext(for: plan),
                phraseIndex: cue.phraseIndex,
                themeID: themeID
            )
        )
    }

    private func selectScene(
        _ sceneID: UInt64,
        cue: PlanCueSnapshot,
        plan: PlanSnapshot
    ) {
        onPlanMutation(
            .selectScene(
                context: mutationContext(for: plan),
                phraseIndex: cue.phraseIndex,
                sceneID: sceneID
            )
        )
    }

    private func themeID(_ cue: PlanCueSnapshot) -> UInt64? {
        if case let .applyLook(themeID, _, _, _, _, _, _) = cue.action {
            return themeID
        }
        return nil
    }

    private func sceneID(_ cue: PlanCueSnapshot) -> UInt64? {
        if case let .applyLook(_, _, sceneID, _, _, _, _) = cue.action {
            return sceneID
        }
        return nil
    }

    private func compatibleAutoloops(
        for cue: PlanCueSnapshot,
        options: PlanningOptionsSnapshot
    ) -> [PlanSelectionChoice] {
        if let choices = cue.libraryResolution?.choices, !choices.isEmpty {
            return choices.map { PlanSelectionChoice(id: $0.id, name: $0.name) }
        }
        guard case let .applyLook(_, _, _, _, category, _, _) = cue.action else {
            return []
        }
        return options.scenes
            .filter { $0.category == category }
            .map { PlanSelectionChoice(id: $0.id, name: $0.name) }
    }

    private var conditionLabel: String {
        switch state.condition {
        case .empty: copy.empty
        case .loading: copy.loading
        case .ready: copy.ready
        case .fallback: copy.fallback
        case .stale: copy.stale
        case .degraded: copy.degraded
        case .disconnected: copy.disconnected
        case .error: copy.error
        }
    }

    private var componentState: LumiComponentState {
        switch state.condition {
        case .empty: .empty
        case .loading: .loading
        case .ready: .ready
        case .fallback, .degraded, .disconnected: .degraded
        case .stale: .stale
        case .error: .error
        }
    }

    private func componentState(for condition: ProviderCondition) -> LumiComponentState {
        switch condition {
        case .empty: .empty
        case .loading: .loading
        case .ready: .ready
        case .stale: .stale
        case .degraded: .degraded
        case .error: .error
        }
    }

    private func providerLabel(_ condition: ProviderCondition) -> String {
        switch condition {
        case .empty: copy.empty
        case .loading: copy.loading
        case .ready: copy.ready
        case .stale: copy.stale
        case .degraded: copy.degraded
        case .error: copy.error
        }
    }

    private func key(_ value: String) -> LocalizedStringKey {
        LocalizedStringKey(value)
    }
}

private struct PlanSelectionChoice: Identifiable {
    let id: UInt64
    let name: String
}

private struct PlanSelectionControl: View {
    let value: String
    let selectedID: UInt64?
    let choices: [PlanSelectionChoice]
    let isEnabled: Bool
    let onSelect: (UInt64) -> Void

    @State private var isPresented = false

    var body: some View {
        Button {
            isPresented.toggle()
        } label: {
            HStack(spacing: LumiSpacing.small) {
                Text(verbatim: value)
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textPrimary)
                Spacer()
                Image(systemName: "chevron.up.chevron.down")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(.horizontal, LumiSpacing.medium)
            .frame(maxWidth: .infinity, minHeight: LumiControlMetric.standardHeight)
            .background(LumiColor.canvas)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.control)
                    .stroke(LumiColor.border, lineWidth: 1)
            }
        }
        .buttonStyle(.plain)
        .disabled(!isEnabled)
        .popover(isPresented: $isPresented, arrowEdge: .trailing) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                ForEach(choices) { choice in
                    Button {
                        isPresented = false
                        onSelect(choice.id)
                    } label: {
                        HStack {
                            Text(verbatim: choice.name)
                            Spacer()
                            if choice.id == selectedID {
                                Image(systemName: "checkmark")
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .disabled(choice.id == selectedID)
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(minHeight: LumiControlMetric.standardHeight)
                }
            }
            .padding(LumiSpacing.small)
            .frame(minWidth: 220)
        }
    }
}

#Preview("Ready · Dark") {
    LiveWorkspaceView(
        state: LiveWorkspaceFixtures.ready,
        productVersion: "0.1.0-dev",
        appearance: .constant(.dark),
        keyNotation: .constant(.camelot)
    )
    .preferredColorScheme(.dark)
    .frame(width: 1_180, height: 820)
}

#Preview("Fallback · Light") {
    LiveWorkspaceView(
        state: LiveWorkspaceFixtures.fallback,
        productVersion: "0.1.0-dev",
        appearance: .constant(.light),
        keyNotation: .constant(.classic)
    )
    .preferredColorScheme(.light)
    .frame(width: 1_180, height: 820)
}
