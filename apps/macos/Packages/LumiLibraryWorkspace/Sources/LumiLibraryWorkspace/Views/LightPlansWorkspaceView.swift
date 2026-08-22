import LumiDesignSystem
import SwiftUI

public struct LightPlansWorkspaceView: View {
    private enum Section: String, CaseIterable, Identifiable {
        case rules = "AutoLoop Rules"
        case preview = "Plan Preview"
        case modifiers = "SoundSwitch Modifiers"
        var id: String { rawValue }
    }

    public let state: LightPlanningState
    public let library: LibraryWorkspaceState
    public let feedback: String?
    public let onSave: (LightPlanningPolicyState) -> Void
    public let onOpenTrack: (UInt64) -> Void
    public let onPreview: (UInt64, UInt64, UInt64, UInt64, LightPlanningPolicyState) -> Void
    public let onOpenLightingOutputs: () -> Void
    public let onSendModifierLearnPulse: (UInt8, UInt8) -> Void

    @State private var section: Section = .rules
    @State private var draft: LightPlanningPolicyState
    @State private var selectedRoleID: String?
    @State private var selectedThemeID: UInt64?
    @State private var selectedTrackID: UInt64?
    @State private var variationSeed: UInt64 = 1

    public init(
        state: LightPlanningState,
        library: LibraryWorkspaceState,
        feedback: String?,
        onSave: @escaping (LightPlanningPolicyState) -> Void,
        onOpenTrack: @escaping (UInt64) -> Void,
        onPreview: @escaping (UInt64, UInt64, UInt64, UInt64, LightPlanningPolicyState) -> Void,
        onOpenLightingOutputs: @escaping () -> Void,
        onSendModifierLearnPulse: @escaping (UInt8, UInt8) -> Void
    ) {
        self.state = state
        self.library = library
        self.feedback = feedback
        self.onSave = onSave
        self.onOpenTrack = onOpenTrack
        self.onPreview = onPreview
        self.onOpenLightingOutputs = onOpenLightingOutputs
        self.onSendModifierLearnPulse = onSendModifierLearnPulse
        _draft = State(initialValue: state.policy)
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            header
            Picker("Light Plans section", selection: $section) {
                ForEach(Section.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 620)
            Group {
                switch section {
                case .rules: rulesWorkspace
                case .preview: previewWorkspace
                case .modifiers: modifiersWorkspace
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .padding(LumiSpacing.xLarge)
        .background(LumiColor.canvas)
        .onChange(of: state.policy) { _, value in draft = value }
        .onAppear {
            selectedRoleID = selectedRoleID ?? activeRoles.first?.id
            selectedThemeID = selectedThemeID ?? catalog?.themes.first?.id
            selectedTrackID = selectedTrackID ?? library.page.tracks.first?.id
            requestPreview()
        }
        .accessibilityIdentifier("lumi.lightPlans.workspace")
    }

    private var catalog: AutoloopCatalogState? { library.autoloopCatalog }
    private var activeRoles: [AutoloopRoleMatrixState] {
        catalog?.roles.filter { !$0.archived } ?? []
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                HStack(spacing: LumiSpacing.xSmall) {
                    Text("Light Plans").font(LumiTypography.screenTitle)
                    LightPlanInfoTip(
                        "Lumi compiles a complete, deterministic lighting plan before playback. "
                        + "Editing these rules never adds work to the time-critical Pro DJ Link or MIDI lanes."
                    )
                }
                Text("Compile musical variation before playback. Live timing lanes stay isolated.")
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 4) {
                Label("Precompiled", systemImage: "checkmark.shield")
                    .foregroundStyle(LumiColor.success)
                Text("Policy revision \(draft.revision)")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
        }
    }

    private var rulesWorkspace: some View {
        HStack(alignment: .top, spacing: LumiSpacing.large) {
            VStack(alignment: .leading, spacing: 4) {
                Text("PHRASE ROLES").font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                ForEach(activeRoles) { role in
                    Button {
                        selectedRoleID = role.id
                    } label: {
                        HStack {
                            Text(role.name)
                            Spacer()
                            Text("\(mappedCandidateCount(role))")
                                .font(LumiTypography.technical)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, LumiSpacing.small)
                        .frame(height: 34)
                        .background(selectedRoleID == role.id ? LumiColor.accent.opacity(0.16) : Color.clear)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .frame(width: 190)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    repeatProtection
                    if let role = activeRoles.first(where: { $0.id == selectedRoleID }) {
                        candidateRules(role)
                    } else {
                        ContentUnavailableView("Select a Phrase Role", systemImage: "music.quarternote.3")
                    }
                }
            }
        }
    }

    private var repeatProtection: some View {
        GroupBox {
            HStack(spacing: LumiSpacing.xLarge) {
                stepper(
                    "Theme cooldown",
                    value: $draft.themeCooldownTracks,
                    suffix: "tracks",
                    help: "The number of previously executed tracks during which Lumi avoids selecting the same Theme again. Explicit choices remain authoritative; Lumi only relaxes this protection when no valid automatic option exists."
                )
                stepper(
                    "AutoLoop cooldown",
                    value: $draft.autoloopCooldownUses,
                    suffix: "role uses",
                    help: "The number of recent uses within the same Phrase Role that exclude an AutoLoop from automatic selection. An Intro never consumes the cooldown history of a Drop."
                )
                stepper(
                    "Whole-plan duplicate",
                    value: $draft.duplicatePlanWindow,
                    suffix: "tracks",
                    help: "The number of previously executed tracks whose complete AutoLoop sequence may not be repeated. Sparse mappings can require a safe fallback, which remains visible in Plan Preview."
                )
                Spacer()
                Button("Save Rules") {
                    draft = materializedPolicyDraft()
                    onSave(draft)
                }
                    .buttonStyle(.borderedProminent)
            }
            if let feedback {
                Text(feedback).font(LumiTypography.technical).foregroundStyle(LumiColor.textSecondary)
            }
        } label: {
            HStack(spacing: LumiSpacing.xSmall) {
                Text("Repeat Protection")
                LightPlanInfoTip(
                    "These protections reduce repetition across consecutive tracks. They affect automatic choices only and are evaluated while the plan is compiled."
                )
            }
        }
    }

    private func stepper(
        _ title: String,
        value: Binding<UInt8>,
        suffix: String,
        help: String
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 4) {
                Text(title).font(LumiTypography.metadata)
                LightPlanInfoTip(help)
            }
            Stepper("\(value.wrappedValue) \(suffix)", value: value, in: 0...8)
                .frame(width: 170)
        }
    }

    private func candidateRules(_ role: AutoloopRoleMatrixState) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                VStack(alignment: .leading) {
                    Text(role.name).font(LumiTypography.sectionTitle)
                    HStack(spacing: LumiSpacing.xSmall) {
                        Text("Selection Weight controls relative use; it is not a playback frequency.")
                            .foregroundStyle(LumiColor.textSecondary)
                        LightPlanInfoTip(
                            "Weight compares eligible AutoLoops with each other. Primary is more likely than Normal, but does not mean it plays on every phrase. Repeat protection and Track Color are applied first."
                        )
                    }
                }
                Spacer()
                Button("Physical mappings") { onOpenLightingOutputs() }
            }
            ForEach(catalog?.themes ?? []) { theme in
                GroupBox(theme.name) {
                    let variants = role.variants.filter { !$0.archived && mappedCell($0, themeID: theme.id) != nil }
                    if variants.isEmpty {
                        Text("No mapped AutoLoops for this Phrase Role.")
                            .foregroundStyle(LumiColor.warning)
                    } else {
                        VStack(spacing: LumiSpacing.small) {
                            ForEach(variants) { variant in
                                candidateRow(role: role, variant: variant, theme: theme)
                            }
                        }
                    }
                }
            }
        }
    }

    private func candidateRow(
        role: AutoloopRoleMatrixState,
        variant: AutoloopVariantState,
        theme: AutoloopThemeState
    ) -> some View {
        let cell = mappedCell(variant, themeID: theme.id)
        let binding = ruleBinding(themeID: theme.id, roleID: role.id, variantID: variant.id)
        return HStack(spacing: LumiSpacing.medium) {
            Toggle("", isOn: binding.enabled).labelsHidden()
            VStack(alignment: .leading, spacing: 2) {
                Text(cell?.name ?? variant.name)
                Text("Bank \(theme.sortOrder) · AutoLoop \(cell?.buttonNumber ?? 0)")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .frame(minWidth: 240, alignment: .leading)
            Picker("Selection Weight", selection: binding.selectionWeight) {
                Text("Rare").tag(UInt8(1))
                Text("Normal").tag(UInt8(2))
                Text("Often").tag(UInt8(3))
                Text("Primary").tag(UInt8(4))
            }
            .frame(width: 150)
            Picker("Track Color", selection: binding.colorBehavior) {
                ForEach(LightPlanColorBehavior.allCases) { Text($0.label).tag($0) }
            }
            .frame(width: 130)
            .help(trackColorBehaviorHelp(binding.wrappedValue.colorBehavior))
            LightPlanInfoTip(trackColorBehaviorHelp(binding.wrappedValue.colorBehavior))
            colorChips(binding)
            Spacer()
        }
        .padding(.vertical, 5)
    }

    private func colorChips(_ binding: Binding<LightPlanAutoloopRule>) -> some View {
        HStack(spacing: 5) {
            if state.trackColors.isEmpty {
                Text("No USB colors")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .help("No Rekordbox track colors are stored in the Lumi Library yet. Resync a trusted OneLibrary USB to import them.")
            }
            ForEach(state.trackColors) { trackColor in
                let rgb = trackColor.rgb
                Button {
                    if binding.wrappedValue.colorRGB.contains(rgb) {
                        binding.wrappedValue.colorRGB.removeAll { $0 == rgb }
                    } else {
                        binding.wrappedValue.colorRGB.append(rgb)
                    }
                } label: {
                    Circle()
                        .fill(Color(rgb: rgb))
                        .frame(width: 16, height: 16)
                        .overlay(Circle().stroke(
                            binding.wrappedValue.colorRGB.contains(rgb) ? Color.white : Color.clear,
                            lineWidth: 2
                        ))
                }
                .buttonStyle(.plain)
                .help("\(trackColor.name) · \(trackColor.trackCount) Library track\(trackColor.trackCount == 1 ? "" : "s")")
            }
        }
        .frame(minWidth: 160, alignment: .leading)
    }

    private var previewWorkspace: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack {
                Picker("Track", selection: $selectedTrackID) {
                    ForEach(library.page.tracks) { track in
                        Text("\(track.title) — \(track.artist)").tag(Optional(track.id))
                    }
                }
                .frame(maxWidth: 420)
                .onChange(of: selectedTrackID) { _, value in
                    if let value {
                        onOpenTrack(value)
                        requestPreview(trackID: value)
                    }
                }
                Picker("Theme", selection: $selectedThemeID) {
                    ForEach(catalog?.themes ?? []) { Text($0.name).tag(Optional($0.id)) }
                }
                .frame(width: 220)
                .onChange(of: selectedThemeID) { _, _ in requestPreview() }
                Button("New variation") {
                    variationSeed &+= 1
                    requestPreview()
                }
                Text("Seed \(variationSeed)").font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                Spacer()
            }
            if let preview = state.preview,
               preview.trackID == selectedTrackID,
               preview.themeID == selectedThemeID {
                ScrollView {
                    LazyVStack(spacing: LumiSpacing.small) {
                        ForEach(preview.phrases) { phrase in
                            previewPhrase(phrase)
                        }
                        HStack(spacing: LumiSpacing.large) {
                            Label("Atmosphere: No Override", systemImage: "moon.stars")
                            Label("Color: No Override", systemImage: "paintpalette")
                            Spacer()
                            Text("Automatic modifiers · POC required")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.warning)
                        }
                        .padding(LumiSpacing.medium)
                        .background(LumiColor.surface)
                        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
                    }
                }
            } else {
                ContentUnavailableView(
                    "Load the selected track",
                    systemImage: "waveform.badge.magnifyingglass",
                    description: Text("Lumi loads its phrases once to compile a complete preview.")
                )
            }
        }
    }

    private func previewPhrase(_ phrase: LightPlanPreviewPhrase) -> some View {
        return HStack(spacing: LumiSpacing.medium) {
            Text("\(phrase.startBeat)–\(phrase.endBeat)")
                .font(LumiTypography.technical).frame(width: 90, alignment: .leading)
            Text(phrase.roleName).frame(width: 160, alignment: .leading)
            Image(systemName: "arrow.right").foregroundStyle(LumiColor.textSecondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(phrase.autoloopName)
                Text("Bank \(selectedThemeID ?? 0) · AutoLoop \(phrase.autoloopNumber)")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            Text("\(phrase.reason) · weight \(phrase.effectiveWeight) · \(phrase.colorInfluence)")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .padding(LumiSpacing.medium)
        .background(LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
    }

    private var modifiersWorkspace: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                GroupBox {
                    HStack {
                        Label("Map and verify Static Looks in Integrations. This workspace only defines when verified looks may be planned.", systemImage: "point.3.connected.trianglepath.dotted")
                        Spacer()
                        Button("Open Lighting Outputs", action: onOpenLightingOutputs)
                            .buttonStyle(.borderedProminent)
                    }
                }
                ForEach(LightPlanModifierKind.allCases) { kind in
                    GroupBox(kind == .atmosphere ? "Static Look Planning" : "Color Overrides") {
                        VStack(alignment: .leading, spacing: LumiSpacing.small) {
                            ForEach(
                                draft.modifiers.indices.filter { draft.modifiers[$0].kind == kind },
                                id: \.self
                            ) { index in
                                let modifierBinding: Binding<LightPlanOutputModifier> = $draft.modifiers[index]
                                HStack {
                                    Toggle("", isOn: modifierBinding.enabled).labelsHidden()
                                    if kind == .atmosphere {
                                        Text(modifierBinding.wrappedValue.displayName)
                                            .frame(minWidth: 180, alignment: .leading)
                                        Text("Ch \(modifierBinding.wrappedValue.midiChannel) · Note \(modifierBinding.wrappedValue.midiNote)")
                                            .font(LumiTypography.technical)
                                            .foregroundStyle(LumiColor.textSecondary)
                                    } else {
                                        TextField(kind.label, text: modifierBinding.displayName)
                                        Stepper("Ch \(modifierBinding.wrappedValue.midiChannel)", value: modifierBinding.midiChannel, in: 1...16)
                                        Stepper("Note \(modifierBinding.wrappedValue.midiNote)", value: modifierBinding.midiNote, in: 0...127)
                                    }
                                    Label(
                                        modifierBinding.wrappedValue.automaticExecutionReady ? "Verified" : "POC required",
                                        systemImage: modifierBinding.wrappedValue.automaticExecutionReady ? "checkmark.circle.fill" : "lock.circle"
                                    )
                                    .foregroundStyle(modifierBinding.wrappedValue.automaticExecutionReady ? LumiColor.success : LumiColor.warning)
                                    if kind == .color {
                                        Button("Learn") {
                                            onSendModifierLearnPulse(
                                                modifierBinding.wrappedValue.midiChannel,
                                                modifierBinding.wrappedValue.midiNote
                                            )
                                        }
                                    }
                                    Button("Add Rule") {
                                        addModifierRule(for: modifierBinding.wrappedValue.id)
                                    }
                                    if kind == .color {
                                        Button(role: .destructive) {
                                            removeModifier(id: modifierBinding.wrappedValue.id)
                                        } label: {
                                            Image(systemName: "trash")
                                        }
                                    }
                                }
                                ForEach(
                                    draft.modifierRules.indices.filter {
                                        draft.modifierRules[$0].modifierID == modifierBinding.wrappedValue.id
                                    },
                                    id: \.self
                                ) { ruleIndex in
                                    modifierRuleRow(
                                        $draft.modifierRules[ruleIndex],
                                        onDelete: { removeModifierRule(at: ruleIndex) }
                                    )
                                }
                            }
                            if kind == .color {
                                Button("Add \(kind.label)") { addModifier(kind) }
                            } else if !draft.modifiers.contains(where: { $0.kind == .atmosphere }) {
                                Text("No Static Looks mapped yet.")
                                    .font(LumiTypography.caption)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                        }
                    }
                }
                HStack {
                    Spacer()
                    Button("Save Planning Rules") {
                        draft = materializedPolicyDraft()
                        onSave(draft)
                    }
                        .buttonStyle(.borderedProminent)
                }
            }
        }
    }

    private func addModifier(_ kind: LightPlanModifierKind) {
        draft.modifiers.append(LightPlanOutputModifier(
            id: "soundswitch-\(kind.rawValue)-\(UUID().uuidString.lowercased())",
            providerKind: "soundswitch",
            kind: kind,
            displayName: "New Color Override",
            enabled: true,
            midiChannel: 14,
            midiNote: 0,
            activationVerified: false,
            releaseVerified: false
        ))
    }

    private func mappedCandidateCount(_ role: AutoloopRoleMatrixState) -> Int {
        role.variants.filter { variant in variant.cells.contains { !$0.isMissing } }.count
    }

    private func mappedCell(_ variant: AutoloopVariantState, themeID: UInt64) -> AutoloopCellState? {
        variant.cells.first { $0.themeID == themeID && !$0.isMissing }
    }

    private func ruleBinding(themeID: UInt64, roleID: String, variantID: String) -> Binding<LightPlanAutoloopRule> {
        Binding {
            draft.rules.first { $0.themeID == themeID && $0.roleID == roleID && $0.variantID == variantID }
                ?? LightPlanAutoloopRule(themeID: themeID, roleID: roleID, variantID: variantID)
        } set: { value in
            if let index = draft.rules.firstIndex(where: { $0.id == value.id }) {
                draft.rules[index] = value
            } else {
                draft.rules.append(value)
            }
        }
    }

    private func requestPreview(trackID: UInt64? = nil) {
        guard let track = library.page.tracks.first(where: { $0.id == (trackID ?? selectedTrackID) }),
              let timelineRevision = track.timelineRevision,
              let themeID = selectedThemeID else { return }
        onPreview(track.id, timelineRevision, themeID, variationSeed, materializedPolicyDraft())
    }

    private func materializedPolicyDraft() -> LightPlanningPolicyState {
        var policy = draft
        guard let catalog else { return policy }
        for role in catalog.roles where !role.archived {
            for variant in role.variants where !variant.archived {
                for cell in variant.cells where !cell.isMissing {
                    let rule = LightPlanAutoloopRule(
                        themeID: cell.themeID,
                        roleID: role.id,
                        variantID: variant.id
                    )
                    if !policy.rules.contains(where: { $0.id == rule.id }) {
                        policy.rules.append(rule)
                    }
                }
            }
        }
        return policy
    }

    private func addModifierRule(for modifierID: String) {
        let usedRoleIDs = Set(
            draft.modifierRules
                .filter { $0.modifierID == modifierID }
                .map(\.roleID)
        )
        guard let roleID = activeRoles.first(where: { !usedRoleIDs.contains($0.id) })?.id
                ?? activeRoles.first?.id else { return }
        draft.modifierRules.append(LightPlanModifierRule(
            modifierID: modifierID,
            roleID: roleID,
            applicationRate: 25,
            selectionWeight: 2,
            cooldownUses: 2,
            scope: .phrase,
            colorBehavior: .neutral,
            colorRGB: []
        ))
    }

    private func removeModifier(id: String) {
        draft.modifierRules.removeAll { $0.modifierID == id }
        draft.modifiers.removeAll { $0.id == id }
    }

    private func removeModifierRule(at index: Int) {
        guard draft.modifierRules.indices.contains(index) else { return }
        draft.modifierRules.remove(at: index)
    }

    private func modifierRuleRow(
        _ binding: Binding<LightPlanModifierRule>,
        onDelete: @escaping () -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            HStack(spacing: LumiSpacing.medium) {
                Picker("Phrase Role", selection: binding.roleID) {
                    ForEach(activeRoles) { Text($0.name).tag($0.id) }
                }.frame(width: 150)
                Picker("Scope", selection: binding.scope) {
                    ForEach(LightPlanModifierScope.allCases) { Text($0.label).tag($0) }
                }.frame(width: 120)
                Stepper("Apply \(binding.wrappedValue.applicationRate)%", value: binding.applicationRate, in: 0...100)
                    .frame(width: 150)
                Picker("Selection Weight", selection: binding.selectionWeight) {
                    Text("Rare").tag(UInt8(1)); Text("Normal").tag(UInt8(2))
                    Text("Often").tag(UInt8(3)); Text("Primary").tag(UInt8(4))
                }.frame(width: 130)
                Stepper("Cooldown \(binding.wrappedValue.cooldownUses)", value: binding.cooldownUses, in: 0...8)
                    .frame(width: 140)
                Spacer()
            }
            HStack(spacing: LumiSpacing.medium) {
                Picker("Track Color", selection: binding.colorBehavior) {
                    ForEach(LightPlanColorBehavior.allCases) { Text($0.label).tag($0) }
                }
                .frame(width: 150)
                .help(trackColorBehaviorHelp(binding.wrappedValue.colorBehavior))
                LightPlanInfoTip(trackColorBehaviorHelp(binding.wrappedValue.colorBehavior))
                modifierColorChips(binding)
                Spacer()
                Button(role: .destructive, action: onDelete) {
                    Label("Remove Rule", systemImage: "trash")
                }
            }
        }
        .padding(.leading, 30)
        .padding(.bottom, LumiSpacing.small)
    }

    private func modifierColorChips(_ binding: Binding<LightPlanModifierRule>) -> some View {
        HStack(spacing: 5) {
            if state.trackColors.isEmpty {
                Text("No USB colors")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            ForEach(state.trackColors) { trackColor in
                let rgb = trackColor.rgb
                Button {
                    if binding.wrappedValue.colorRGB.contains(rgb) {
                        binding.wrappedValue.colorRGB.removeAll { $0 == rgb }
                    } else {
                        binding.wrappedValue.colorRGB.append(rgb)
                    }
                } label: {
                    Circle()
                        .fill(Color(rgb: rgb))
                        .frame(width: 16, height: 16)
                        .overlay(Circle().stroke(
                            binding.wrappedValue.colorRGB.contains(rgb) ? Color.white : Color.clear,
                            lineWidth: 2
                        ))
                }
                .buttonStyle(.plain)
                .help("\(trackColor.name) · \(trackColor.trackCount) Library track\(trackColor.trackCount == 1 ? "" : "s")")
            }
        }
        .frame(minWidth: 160, alignment: .leading)
    }

    private func trackColorBehaviorHelp(_ behavior: LightPlanColorBehavior) -> String {
        switch behavior {
        case .neutral:
            "Neutral ignores the Rekordbox track color for this candidate."
        case .prefer:
            "Prefer boosts this candidate when the playing track has one of the selected Rekordbox colors. It can still be selected for other colors."
        case .only:
            "Only makes this candidate eligible when the playing track has one of the selected Rekordbox colors. A track without a color will not match it."
        }
    }
}

private struct LightPlanInfoTip: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Image(systemName: "info.circle")
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(LumiColor.textSecondary)
            .help(text)
            .accessibilityLabel(text)
    }
}

private extension Color {
    init(rgb: UInt32) {
        self.init(
            red: Double((rgb >> 16) & 0xff) / 255,
            green: Double((rgb >> 8) & 0xff) / 255,
            blue: Double(rgb & 0xff) / 255
        )
    }
}
