import Foundation
import LumiDesignSystem
import SwiftUI

struct FoundationView: View {
    @ObservedObject var engineStatus: EngineStatusModel
    @Bindable var preferences: LumiPreferences

    @State private var selectedPhrase = 0

    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
    }

    var body: some View {
        ZStack {
            LumiColor.canvas.ignoresSafeArea()

            ScrollView {
                VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                    appHeader
                    preferencesPanel
                    enginePanel
                    runtimePanel
                    deckSourceWorkspace
                    planPreview
                }
                .padding(LumiSpacing.xLarge)
            }
        }
        .frame(minWidth: 760, minHeight: 560)
    }

    private var appHeader: some View {
        HStack(alignment: .firstTextBaseline, spacing: LumiSpacing.medium) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text("app.title")
                    .font(LumiTypography.screenTitle)
                    .foregroundStyle(LumiColor.textPrimary)
                Text("design.preview.subtitle")
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            Text(verbatim: productVersion)
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private var preferencesPanel: some View {
        LumiPanel {
            HStack(spacing: LumiSpacing.xLarge) {
                preferencePicker(
                    "preference.appearance.label",
                    selection: $preferences.appearance,
                    values: AppearancePreference.allCases,
                    title: { $0.titleKey }
                )
                preferencePicker(
                    "preference.key.label",
                    selection: $preferences.keyNotation,
                    values: KeyNotationPreference.allCases,
                    title: { $0.titleKey }
                )
                Spacer()
            }
        }
    }

    private var enginePanel: some View {
        LumiPanel {
            ProviderStatus(
                name: "engine.provider.name",
                detail: enginePresentation.detail,
                stateLabel: enginePresentation.label,
                state: enginePresentation.state
            )
        }
    }

    private var runtimePanel: some View {
        LumiPanel {
            ProviderStatus(
                name: "runtime.provider.name",
                detail: runtimePresentation.detail,
                stateLabel: runtimePresentation.label,
                state: runtimePresentation.state
            )
        }
    }

    private var deckSourceWorkspace: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack {
                Text("deck.source.workspace")
                    .font(LumiTypography.sectionTitle)
                if case let .ready(engine) = engineStatus.state {
                    Text(verbatim: engine.deckSource.providerKind.capitalized)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                StatusBadge(deckSourceBadge.label, state: deckSourceBadge.state)
                Spacer()
            }

            if case let .ready(engine) = engineStatus.state {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    ForEach(orderedDecks(from: engine)) { deck in
                        deckCard(
                            deck,
                            isLeader: deck.deckID == engine.leaderDeckID
                        )
                    }
                }
            } else {
                LumiPanel {
                    Text("deck.source.waiting")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
    }

    private var planPreview: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack {
                Text("design.preview.planWorkspace")
                    .font(LumiTypography.sectionTitle)
                StatusBadge("design.preview.sampleData", state: .empty)
                Spacer()
            }

            HStack(alignment: .top, spacing: LumiSpacing.large) {
                phrasePanel
                inspectorPanel
                    .frame(minWidth: 260, idealWidth: 300, maxWidth: 320)
            }

            operationPreview
            statePreview
        }
    }

    private var phrasePanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                Text("design.preview.phrasePlan")
                    .font(LumiTypography.sectionTitle)
                PhraseRow(
                    phrase: "Intro",
                    range: "00:00–00:32",
                    scene: "Soft Motion · Loop 1",
                    isLocked: false,
                    isSelected: selectedPhrase == 0,
                    action: { selectedPhrase = 0 }
                )
                PhraseRow(
                    phrase: "Breakdown",
                    range: "00:32–01:04",
                    scene: "Neon Pulse · Loop 3",
                    isLocked: true,
                    isSelected: selectedPhrase == 1,
                    action: { selectedPhrase = 1 }
                )
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var inspectorPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                Text("design.preview.inspector")
                    .font(LumiTypography.sectionTitle)
                InspectorField("design.preview.theme") {
                    Text("design.preview.themeValue")
                        .font(LumiTypography.body)
                }
                InspectorField("design.preview.scene") {
                    Text(selectedPhrase == 0 ? "Soft Motion" : "Neon Pulse")
                        .font(LumiTypography.body)
                }
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var operationPreview: some View {
        HStack(spacing: LumiSpacing.small) {
            OperationControl(
                "operation.arm",
                systemImage: "shield",
                isEnabled: false,
                action: {}
            )
            OperationControl(
                "operation.start",
                systemImage: "play.fill",
                isEnabled: false,
                action: {}
            )
            OperationControl(
                "operation.pause",
                systemImage: "pause.fill",
                isEnabled: false,
                action: {}
            )
            OperationControl(
                "operation.off",
                systemImage: "stop.fill",
                isEnabled: false,
                action: {}
            )
            Text("design.preview.controlsLater")
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private var statePreview: some View {
        HStack(spacing: LumiSpacing.small) {
            ForEach(LumiComponentState.allCases, id: \.self) { state in
                StatusBadge(state.titleKey, state: state)
            }
        }
    }

    private var keyFormatter: KeyNotationFormatter {
        KeyNotationFormatter(notation: preferences.keyNotation)
    }

    private var deckSourceBadge: (label: LocalizedStringKey, state: LumiComponentState) {
        guard case let .ready(engine) = engineStatus.state else {
            return ("design.state.loading", .loading)
        }
        return engine.deckSource.status == "ready"
            ? ("design.state.ready", .ready)
            : ("design.state.degraded", .degraded)
    }

    private func orderedDecks(from engine: EngineReadyViewState) -> [EngineDeckViewState] {
        engine.decks.sorted { left, right in
            if left.deckID == engine.leaderDeckID { return true }
            if right.deckID == engine.leaderDeckID { return false }
            return left.deckID < right.deckID
        }
    }

    private func deckCard(_ deck: EngineDeckViewState, isLeader: Bool) -> some View {
        DeckCard(
            deckLabel: isLeader ? "design.preview.liveDeck" : "design.preview.nextDeck",
            title: deck.title,
            artist: deck.artist,
            bpm: String(
                format: "%.1f",
                locale: Locale(identifier: "en_US_POSIX"),
                Double(deck.bpmMilli) / 1_000
            ),
            musicalKey: musicalKey(for: deck),
            stateLabel: "design.state.ready",
            state: .ready
        )
        .frame(maxWidth: .infinity)
    }

    private func musicalKey(for deck: EngineDeckViewState) -> String {
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

    private var enginePresentation: EngineProviderPresentation {
        switch engineStatus.state {
        case .stopped:
            EngineProviderPresentation(
                detail: String(localized: "engine.status.stopped"),
                label: "design.state.empty",
                state: .empty
            )
        case .starting:
            EngineProviderPresentation(
                detail: String(localized: "engine.status.starting"),
                label: "design.state.loading",
                state: .loading
            )
        case let .connecting(endpoint):
            EngineProviderPresentation(
                detail: endpoint,
                label: "engine.status.connecting",
                state: .loading
            )
        case let .ready(engine):
            EngineProviderPresentation(
                detail: "\(engine.endpoint) · engine \(engine.engineVersion) · protocol v\(engine.protocolVersion) · snapshot #\(engine.snapshotSequence)",
                label: "engine.status.ready",
                state: .ready
            )
        case .disconnected:
            EngineProviderPresentation(
                detail: String(localized: "engine.status.disconnected"),
                label: "design.state.degraded",
                state: .degraded
            )
        case let .failed(message):
            EngineProviderPresentation(
                detail: message,
                label: "design.state.error",
                state: .error
            )
        }
    }

    private var runtimePresentation: EngineProviderPresentation {
        guard case let .ready(engine) = engineStatus.state else {
            return EngineProviderPresentation(
                detail: String(localized: "runtime.detail.waiting"),
                label: "design.state.loading",
                state: .loading
            )
        }

        let runtime = engine.runtimeCore
        let format = String(localized: "runtime.detail.ready")
        let detail = String.localizedStringWithFormat(
            format,
            runtime.processedEvents,
            runtime.queueDepth,
            runtime.queueCapacity,
            engine.stateRevision,
            runtime.lastDecision
        )
        return EngineProviderPresentation(
            detail: detail,
            label: "runtime.status.serialized",
            state: runtime.health == "ready" ? .ready : .degraded
        )
    }

    private func preferencePicker<Value: Hashable>(
        _ label: LocalizedStringKey,
        selection: Binding<Value>,
        values: [Value],
        title: @escaping (Value) -> LocalizedStringKey
    ) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            Text(label)
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(LumiColor.textSecondary)
            Picker(label, selection: selection) {
                ForEach(values, id: \.self) { value in
                    Text(title(value)).tag(value)
                }
            }
            .labelsHidden()
            .frame(minWidth: 140)
        }
    }
}

private struct EngineProviderPresentation {
    let detail: String
    let label: LocalizedStringKey
    let state: LumiComponentState
}

#Preview("Dark") {
    FoundationView(
        engineStatus: EngineStatusModel(),
        preferences: LumiPreferences()
    )
    .preferredColorScheme(.dark)
}

#Preview("Light") {
    FoundationView(
        engineStatus: EngineStatusModel(),
        preferences: LumiPreferences()
    )
    .preferredColorScheme(.light)
}
