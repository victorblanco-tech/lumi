import Foundation
import LumiDesignSystem
import SwiftUI

public struct LiveWorkspaceView: View {
    private let state: LiveWorkspaceState
    private let productVersion: String
    private let allowsScrolling: Bool
    @Binding private var appearance: AppearancePreference
    @Binding private var keyNotation: KeyNotationPreference
    @State private var selectedPhrase: UInt64 = 0

    private let copy = LiveWorkspaceCopy()

    public init(
        state: LiveWorkspaceState,
        productVersion: String,
        appearance: Binding<AppearancePreference>,
        keyNotation: Binding<KeyNotationPreference>,
        allowsScrolling: Bool = true
    ) {
        self.state = state
        self.productVersion = productVersion
        self.allowsScrolling = allowsScrolling
        _appearance = appearance
        _keyNotation = keyNotation
    }

    public var body: some View {
        HStack(spacing: 0) {
            sidebar
            Divider()
            mainWorkspace
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
                navigationRow(copy.live, systemImage: "waveform", isSelected: true)
                navigationRow(copy.plans, systemImage: "list.bullet.rectangle", isSelected: false)
                navigationRow(copy.library, systemImage: "music.note.list", isSelected: false)
                navigationRow(copy.integrations, systemImage: "cable.connector", isSelected: false)
            }

            Spacer()
            navigationRow(copy.settings, systemImage: "gearshape", isSelected: false)
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
        VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
            header
            if let diagnostic = state.diagnostic {
                diagnosticBanner(diagnostic)
            }
            providerPanel
            deckWorkspace
            planWorkspace
        }
        .padding(LumiSpacing.xLarge)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var header: some View {
        HStack(spacing: LumiSpacing.large) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: copy.live)
                    .font(LumiTypography.screenTitle)
                    .foregroundStyle(LumiColor.textPrimary)
                Text(verbatim: copy.subtitle)
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            operationControls
            if allowsScrolling {
                preferenceMenu
            } else {
                preferenceIndicator
            }
        }
    }

    private var operationControls: some View {
        HStack(spacing: LumiSpacing.small) {
            OperationControl(key(copy.arm), systemImage: "shield", isEnabled: false, action: {})
            OperationControl(key(copy.start), systemImage: "play.fill", isEnabled: false, action: {})
            OperationControl(key(copy.pause), systemImage: "pause.fill", isEnabled: false, action: {})
            OperationControl(key(copy.off), systemImage: "stop.fill", isEnabled: false, action: {})
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

    private var providerPanel: some View {
        LumiPanel {
            VStack(spacing: LumiSpacing.medium) {
                providerRow(copy.engine, systemImage: "cpu", presentation: state.engine)
                Divider()
                providerRow(copy.runtime, systemImage: "point.3.connected.trianglepath.dotted", presentation: state.runtime)
                Divider()
                providerRow(copy.deckSource, systemImage: "music.quarternote.3", presentation: state.source)
            }
        }
        .accessibilityIdentifier("lumi.provider.status")
    }

    private var deckWorkspace: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack(spacing: LumiSpacing.medium) {
                Text(verbatim: copy.liveDeckSource)
                    .font(LumiTypography.sectionTitle)
                if let content = state.content {
                    Text(verbatim: content.sourceName)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                StatusBadge(key(deckConditionLabel), state: deckComponentState)
                Spacer()
            }

            if let content = state.content {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    deckCard(content.liveDeck, label: copy.liveDeck, identifier: "lumi.deck.live")
                    deckCard(content.nextDeck, label: copy.nextDeck, identifier: "lumi.deck.next")
                }
            } else {
                placeholder(copy.waitingDecks, systemImage: "waveform.badge.magnifyingglass")
            }
        }
    }

    private var planWorkspace: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack(spacing: LumiSpacing.medium) {
                Text(verbatim: copy.nextPlan)
                    .font(LumiTypography.sectionTitle)
                if let plan = state.content?.plan {
                    Text(verbatim: "Revision \(plan.revision) · Config \(plan.configurationRevision)")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                StatusBadge(key(planConditionLabel), state: planComponentState)
                Spacer()
            }

            if let content = state.content, let plan = content.plan {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    phrasePanel(plan: plan, deck: content.nextDeck)
                    inspectorPanel(plan: plan)
                        .frame(minWidth: 240, idealWidth: 280, maxWidth: 300)
                }
            } else {
                placeholder(copy.waitingPlan, systemImage: "list.bullet.rectangle.portrait")
            }

            Text(verbatim: copy.controlsLater)
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .accessibilityIdentifier("lumi.next.plan")
    }

    private func phrasePanel(plan: PlanSnapshot, deck: DeckSnapshot) -> some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                Text(verbatim: copy.phrasePlan)
                    .font(LumiTypography.sectionTitle)
                ForEach(plan.cues) { cue in
                    PhraseRow(
                        phrase: phraseTitle(cue),
                        range: timeRange(cue, bpmMilli: deck.bpmMilli),
                        scene: cueSummary(cue),
                        isLocked: false,
                        isSelected: selectedPhrase == cue.phraseIndex,
                        action: { selectedPhrase = cue.phraseIndex }
                    )
                    .accessibilityIdentifier("lumi.plan.phrase.\(cue.phraseIndex)")
                }
            }
        }
        .frame(maxWidth: .infinity)
    }

    private func inspectorPanel(plan: PlanSnapshot) -> some View {
        let cue = selectedCue(in: plan)
        return LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                Text(verbatim: copy.inspector)
                    .font(LumiTypography.sectionTitle)
                if let cue {
                    InspectorField(key(copy.theme)) {
                        Text(verbatim: themeName(cue)).font(LumiTypography.body)
                    }
                    InspectorField(key(copy.scene)) {
                        Text(verbatim: sceneName(cue)).font(LumiTypography.body)
                    }
                    InspectorField(key(copy.reason)) {
                        Text(verbatim: reasonSummary(cue)).font(LumiTypography.body)
                    }
                } else {
                    Text(verbatim: copy.waitingPlan)
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
        .frame(maxWidth: .infinity)
        .accessibilityIdentifier("lumi.plan.inspector")
    }

    private func navigationRow(
        _ title: String,
        systemImage: String,
        isSelected: Bool
    ) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            Image(systemName: systemImage)
                .frame(width: 18)
            Text(verbatim: title)
                .font(LumiTypography.body.weight(isSelected ? .semibold : .regular))
            Spacer()
            if !isSelected && title != copy.settings {
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

    private func deckCard(
        _ deck: DeckSnapshot,
        label: String,
        identifier: String
    ) -> some View {
        DeckCard(
            deckLabel: key(label),
            title: deck.title,
            artist: deck.artist,
            bpm: String(
                format: "%.1f",
                locale: Locale(identifier: "en_US_POSIX"),
                Double(deck.bpmMilli) / 1_000
            ),
            musicalKey: musicalKey(for: deck),
            bpmLabel: key(copy.bpm),
            keyLabel: key(copy.key),
            stateLabel: key(deckConditionLabel),
            state: deckComponentState
        )
        .frame(maxWidth: .infinity)
        .accessibilityIdentifier(identifier)
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
        case let .applyLook(_, sceneName, _, loopBank, loopSlot):
            "\(sceneName) · Loop \(loopBank).\(loopSlot)"
        case .holdCurrentLook:
            copy.hold
        }
    }

    private func themeName(_ cue: PlanCueSnapshot) -> String {
        switch cue.action {
        case let .applyLook(themeName, _, _, _, _): themeName
        case .holdCurrentLook: copy.unavailable
        }
    }

    private func sceneName(_ cue: PlanCueSnapshot) -> String {
        switch cue.action {
        case let .applyLook(_, sceneName, _, _, _): sceneName
        case .holdCurrentLook: copy.hold
        }
    }

    private func reasonSummary(_ cue: PlanCueSnapshot) -> String {
        switch cue.reason {
        case let .phraseCategoryMatched(phrase, category):
            "Matched \(copy.phrase(phrase)) to the \(copy.category(category)) scene category."
        case .missingPhraseAnalysis:
            "Phrase analysis unavailable; preserving the current safe look."
        }
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

    private var planConditionLabel: String {
        guard state.content?.plan != nil else { return copy.loading }
        return state.condition == .fallback ? copy.fallback : conditionLabel
    }

    private var deckConditionLabel: String {
        guard state.content != nil else { return conditionLabel }
        return providerLabel(state.source.condition)
    }

    private var deckComponentState: LumiComponentState {
        guard state.content != nil else { return componentState }
        return componentState(for: state.source.condition)
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

    private var planComponentState: LumiComponentState {
        state.content?.plan == nil ? .loading : componentState
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

#Preview("Ready · Dark") {
    LiveWorkspaceView(
        state: LiveWorkspaceFixtures.ready,
        productVersion: "0.0.5-dev",
        appearance: .constant(.dark),
        keyNotation: .constant(.camelot)
    )
    .preferredColorScheme(.dark)
    .frame(width: 1_180, height: 820)
}

#Preview("Fallback · Light") {
    LiveWorkspaceView(
        state: LiveWorkspaceFixtures.fallback,
        productVersion: "0.0.5-dev",
        appearance: .constant(.light),
        keyNotation: .constant(.classic)
    )
    .preferredColorScheme(.light)
    .frame(width: 1_180, height: 820)
}
