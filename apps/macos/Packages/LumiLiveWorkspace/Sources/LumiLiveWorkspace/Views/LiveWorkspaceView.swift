import Foundation
import LumiDesignSystem
import SwiftUI

public struct LiveWorkspaceView: View {
    private let state: LiveWorkspaceState
    private let productVersion: String
    private let allowsScrolling: Bool
    private let showsNavigation: Bool
    private let onPlanMutation: @MainActor (PlanMutationRequest) -> Void
    private let onSessionCommand: @MainActor (SessionCommandRequest) -> Void
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
        onSessionCommand: @escaping @MainActor (SessionCommandRequest) -> Void = { _ in }
    ) {
        self.state = state
        self.productVersion = productVersion
        self.allowsScrolling = allowsScrolling
        self.showsNavigation = showsNavigation
        self.onPlanMutation = onPlanMutation
        self.onSessionCommand = onSessionCommand
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
        .frame(maxWidth: .infinity, alignment: .leading)
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
            technicalStatusButton
            if state.content?.sourceName.lowercased() == "simulator" {
                if allowsScrolling {
                    simulatorMenu
                } else {
                    simulatorIndicator
                }
            }
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

    private var simulatorMenu: some View {
        Menu {
            Button(copy.loadDemo) {
                sendWithRevision { .loadDemo(expectedStateRevision: $0) }
            }
            .disabled(!canSendSessionCommand)
            Menu(copy.speed) {
                ForEach([UInt64(1), 4, 16, 64], id: \.self) { speed in
                    Button("\(speed)×") {
                        sendWithRevision {
                            .setSimulationSpeed(speed, expectedStateRevision: $0)
                        }
                    }
                    .disabled(
                        !canSendSessionCommand || state.content?.simulation.speed == speed
                    )
                }
            }
            Button(
                state.content?.simulation.paused == true ? copy.resumeDemo : copy.pauseDemo
            ) {
                let playing = state.content?.simulation.paused == true
                sendWithRevision {
                    .setSimulationPlayback(playing, expectedStateRevision: $0)
                }
            }
            .disabled(!canSendSessionCommand)
            Divider()
            Button(copy.nextTrack) {
                sendWithRevision { .advanceToNextTrack(expectedStateRevision: $0) }
            }
            .disabled(!canSendSessionCommand)
            Button(copy.resetDemo) {
                sendWithRevision { .resetDemo(expectedStateRevision: $0) }
            }
            .disabled(!canSendSessionCommand)
        } label: {
            Label("Demo", systemImage: "waveform.path.ecg")
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .accessibilityIdentifier("lumi.simulator.menu")
    }

    private var simulatorIndicator: some View {
        Label("Demo", systemImage: "waveform.path.ecg")
            .font(LumiTypography.metadata.weight(.semibold))
            .foregroundStyle(LumiColor.textSecondary)
            .padding(.horizontal, LumiSpacing.medium)
            .frame(height: LumiControlMetric.standardHeight)
            .background(LumiColor.surface)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.control)
                    .stroke(LumiColor.border, lineWidth: 1)
            }
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
                HStack(alignment: .top, spacing: LumiSpacing.medium) {
                    ForEach(content.decks) { deck in
                        let isMaster = deck.deckID == content.leaderDeckID
                        let plan = content.plan?.deckID == deck.deckID ? content.plan : nil
                        LiveDeckSurface(
                            deck: deck,
                            isMaster: isMaster,
                            plan: plan,
                            musicalKey: musicalKey(for: deck)
                        ) {
                            if isMaster {
                                remainingLiveTrackPlan(deck: deck)
                            } else if let plan {
                                nextTrackPlan(
                                    plan: plan,
                                    deck: deck,
                                    options: content.planningOptions
                                )
                            } else {
                                waitingDeckPlan
                            }
                        }
                        .frame(maxWidth: .infinity)
                        .accessibilityIdentifier(deck.deckID == 1 ? "lumi.deck.a" : "lumi.deck.b")
                    }
                }
            } else {
                placeholder(copy.waitingDecks, systemImage: "waveform.badge.magnifyingglass")
            }
        }
    }

    private func remainingLiveTrackPlan(deck: DeckSnapshot) -> some View {
        let activeIndex = deck.phraseIndex
        let selected = selectedLivePhrase.flatMap { selectedIndex in
            deck.phrases.first(where: { $0.index == selectedIndex })
        } ?? deck.phrases.first(where: { phrase in
            guard let activeIndex else { return true }
            return phrase.index > activeIndex
        })

        return VStack(alignment: .leading, spacing: LumiSpacing.small) {
            HStack {
                Text(verbatim: "Remaining live-track plan")
                    .font(LumiTypography.metadata.weight(.semibold))
                    .foregroundStyle(Color.white)
                Spacer()
                Text(verbatim: "Future phrases")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.accent)
            }

            ForEach(deck.phrases) { phrase in
                let isActive = phrase.index == activeIndex
                let isPast = activeIndex.map { phrase.index < $0 } ?? false
                let isFuture = activeIndex.map { phrase.index > $0 } ?? true
                Button {
                    if isFuture { selectedLivePhrase = phrase.index }
                } label: {
                    HStack(spacing: LumiSpacing.small) {
                        Text(verbatim: phrase.kind.capitalized)
                            .font(LumiTypography.metadata.weight(.semibold))
                            .frame(width: 82, alignment: .leading)
                        Text(verbatim: isActive ? "Current output plan" : "Auto-select at boundary")
                            .font(LumiTypography.metadata)
                            .foregroundStyle(Color.white.opacity(isPast ? 0.38 : 0.76))
                            .lineLimit(1)
                        Spacer()
                        Text(verbatim: livePhraseStatus(
                            phrase: phrase,
                            activeIndex: activeIndex,
                            currentBeat: deck.beat
                        ))
                        .font(LumiTypography.technical.weight(.semibold))
                        .foregroundStyle(isActive ? LumiColor.destructive : Color.white.opacity(0.55))
                    }
                    .padding(.horizontal, LumiSpacing.small)
                    .frame(minHeight: 34)
                    .background {
                        RoundedRectangle(cornerRadius: LumiRadius.compact)
                            .fill(
                                selected?.index == phrase.index
                                    ? LumiColor.accent.opacity(0.14)
                                    : isActive
                                        ? LumiColor.destructive.opacity(0.12)
                                        : Color.white.opacity(0.035)
                            )
                    }
                    .overlay {
                        RoundedRectangle(cornerRadius: LumiRadius.compact)
                            .stroke(
                                selected?.index == phrase.index
                                    ? LumiColor.accent.opacity(0.72)
                                    : Color.white.opacity(0.08),
                                lineWidth: 1
                            )
                    }
                }
                .buttonStyle(.plain)
                .disabled(!isFuture)
            }

            if let selected {
                HStack(spacing: LumiSpacing.small) {
                    compactPlanField(
                        label: "THEME FROM \(selected.kind.uppercased())",
                        value: "Auto-select"
                    )
                    compactPlanField(label: "AUTOLOOP", value: "Auto-select")
                    compactPlanField(label: "APPLY", value: "At phrase start")
                }
                Text(verbatim: "The current phrase stays live; planned changes apply at the selected boundary.")
                    .font(LumiTypography.technical)
                    .foregroundStyle(Color.white.opacity(0.46))
            }
        }
        .padding(LumiSpacing.medium)
        .background(Color.white.opacity(0.025))
        .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.1)) }
        .accessibilityIdentifier("lumi.live.remainingPlan")
    }

    private func nextTrackPlan(
        plan: PlanSnapshot,
        deck: DeckSnapshot,
        options: PlanningOptionsSnapshot
    ) -> some View {
        let cue = selectedCue(in: plan)
        return VStack(alignment: .leading, spacing: LumiSpacing.small) {
            HStack {
                Text(verbatim: "Next-track plan")
                    .font(LumiTypography.metadata.weight(.semibold))
                    .foregroundStyle(Color.white)
                Spacer()
                Text(verbatim: "Editable until transition")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.success)
            }

            ForEach(plan.cues) { planCue in
                Button {
                    selectedPhrase = planCue.phraseIndex
                } label: {
                    HStack(spacing: LumiSpacing.small) {
                        Text(verbatim: phraseTitle(planCue))
                            .font(LumiTypography.metadata.weight(.semibold))
                            .frame(width: 92, alignment: .leading)
                        Text(verbatim: cueSummary(planCue))
                            .font(LumiTypography.metadata)
                            .foregroundStyle(Color.white.opacity(0.76))
                            .lineLimit(1)
                        Spacer()
                        Text(verbatim: planCue.locked ? "LOCKED" : "AUTO")
                            .font(LumiTypography.technical.weight(.semibold))
                            .foregroundStyle(
                                planCue.locked ? LumiColor.accent : Color.white.opacity(0.48)
                            )
                    }
                    .padding(.horizontal, LumiSpacing.small)
                    .frame(minHeight: 34)
                    .background(
                        selectedPhrase == planCue.phraseIndex
                            ? LumiColor.accent.opacity(0.14)
                            : Color.white.opacity(0.035)
                    )
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
                    .overlay {
                        RoundedRectangle(cornerRadius: LumiRadius.compact)
                            .stroke(
                                selectedPhrase == planCue.phraseIndex
                                    ? LumiColor.accent.opacity(0.72)
                                    : Color.white.opacity(0.08),
                                lineWidth: 1
                            )
                    }
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("lumi.plan.phrase.\(planCue.phraseIndex)")
            }

            if let cue {
                VStack(alignment: .leading, spacing: LumiSpacing.small) {
                    HStack(spacing: LumiSpacing.small) {
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(verbatim: "THEME")
                                .font(LumiTypography.technical)
                                .foregroundStyle(Color.white.opacity(0.46))
                            PlanSelectionControl(
                                value: themeName(cue),
                                selectedID: themeID(cue),
                                choices: options.themes.map {
                                    PlanSelectionChoice(id: $0.id, name: $0.name)
                                },
                                isEnabled: canEdit(plan) && themeID(cue) != nil,
                                onSelect: { selectTheme($0, plan: plan) }
                            )
                        }
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(verbatim: "AUTOLOOP")
                                .font(LumiTypography.technical)
                                .foregroundStyle(Color.white.opacity(0.46))
                            PlanSelectionControl(
                                value: sceneName(cue),
                                selectedID: sceneID(cue),
                                choices: compatibleScenes(for: cue, options: options).map {
                                    PlanSelectionChoice(id: $0.id, name: $0.name)
                                },
                                isEnabled: canEdit(plan) && sceneID(cue) != nil,
                                onSelect: { selectScene($0, cue: cue, plan: plan) }
                            )
                        }
                    }
                    HStack(spacing: LumiSpacing.small) {
                        Text(verbatim: "Selected: \(phraseTitle(cue)) · \(timeRange(cue, bpmMilli: deck.bpmMilli))")
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
                                cue.locked ? copy.unlockCue : copy.lockCue,
                                systemImage: cue.locked ? "lock.open" : "lock"
                            )
                        }
                        .buttonStyle(.bordered)
                        .disabled(!canEdit(plan) || themeID(cue) == nil)
                        Button {
                            onPlanMutation(.regeneratePlan(context: mutationContext(for: plan)))
                        } label: {
                            Label(copy.regenerate, systemImage: "arrow.clockwise")
                        }
                        .buttonStyle(.bordered)
                        .disabled(!canEdit(plan))
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
        .accessibilityIdentifier("lumi.next.plan")
    }

    private var waitingDeckPlan: some View {
        Text(verbatim: copy.waitingPlan)
            .font(LumiTypography.metadata)
            .foregroundStyle(Color.white.opacity(0.54))
            .padding(LumiSpacing.medium)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .top) { Divider().overlay(Color.white.opacity(0.1)) }
    }

    private func compactPlanField(label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text(verbatim: label)
                .font(LumiTypography.technical)
                .foregroundStyle(Color.white.opacity(0.44))
                .lineLimit(1)
            Text(verbatim: value)
                .font(LumiTypography.metadata.weight(.semibold))
                .foregroundStyle(Color.white.opacity(0.84))
                .lineLimit(1)
        }
        .padding(LumiSpacing.small)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.white.opacity(0.04))
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.compact))
    }

    private func livePhraseStatus(
        phrase: DeckPhraseSnapshot,
        activeIndex: UInt64?,
        currentBeat: UInt64
    ) -> String {
        guard let activeIndex else {
            return "in \(phrase.startBeat) beats"
        }
        if phrase.index < activeIndex { return "PAST" }
        if phrase.index == activeIndex { return "LIVE · LOCKED" }
        return "in \(phrase.startBeat > currentBeat ? phrase.startBeat - currentBeat : 0) beats"
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
        guard let pitchClass = pitchClass(named: deck.pitchClass),
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
        switch cue.reason {
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

    private func compatibleScenes(
        for cue: PlanCueSnapshot,
        options: PlanningOptionsSnapshot
    ) -> [SceneOptionSnapshot] {
        guard case let .applyLook(_, _, _, _, category, _, _) = cue.action else {
            return []
        }
        return options.scenes.filter { $0.category == category }
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
