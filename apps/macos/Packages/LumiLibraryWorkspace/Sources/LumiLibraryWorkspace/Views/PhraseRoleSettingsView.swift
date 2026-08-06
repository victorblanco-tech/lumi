import LumiDesignSystem
import SwiftUI

public enum PhraseRoleSettingsSection: String, CaseIterable, Identifiable, Sendable {
    case general
    case phraseModel
    case planningDefaults

    public var id: String { rawValue }
}

public struct PhraseRoleSettingsView: View {
    private let settings: PhraseRoleSettingsState?
    @Binding private var appearance: AppearancePreference
    @Binding private var keyNotation: KeyNotationPreference
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (PhraseRoleMutationRequest) -> Void

    @State private var section: PhraseRoleSettingsSection
    @State private var selectedRoleID: String?
    @State private var renameDraft = ""
    @State private var newRoleName = ""
    @State private var showsAddRole = false

    public init(
        settings: PhraseRoleSettingsState?,
        appearance: Binding<AppearancePreference>,
        keyNotation: Binding<KeyNotationPreference>,
        initialSection: PhraseRoleSettingsSection = .phraseModel,
        feedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        onMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in }
    ) {
        self.settings = settings
        _appearance = appearance
        _keyNotation = keyNotation
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onMutation = onMutation
        _section = State(initialValue: initialSection)
        _selectedRoleID = State(initialValue: settings?.roles.first?.id)
        _renameDraft = State(initialValue: settings?.roles.first?.name ?? "")
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
        .accessibilityIdentifier("lumi.settings")
        .alert(copy("settings.addRole"), isPresented: $showsAddRole) {
            TextField(copy("settings.roleName"), text: $newRoleName)
            Button(copy("settings.cancel"), role: .cancel) {
                newRoleName = ""
            }
            Button(copy("settings.add")) {
                let name = newRoleName.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !name.isEmpty else { return }
                onMutation(.add(displayName: name))
                newRoleName = ""
            }
        } message: {
            Text(copy("settings.addRoleDetail"))
        }
        .onChange(of: settings?.revision) { _, _ in
            synchronizeSelection()
        }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(copy("settings.title"))
                    .font(LumiTypography.screenTitle)
                Text(copy("settings.subtitle"))
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            if let revision = settings?.revision {
                Label("R\(revision)", systemImage: "checkmark.circle.fill")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.success)
                    .accessibilityLabel("Phrase-role settings revision \(revision) saved")
            }
        }
        .padding(.horizontal, LumiSpacing.xLarge)
        .frame(height: 82)
    }

    private var sectionNavigation: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            ForEach(PhraseRoleSettingsSection.allCases) { value in
                settingsSectionButton(
                    value,
                    title: sectionTitle(value),
                    icon: sectionIcon(value)
                )
            }
            Spacer()
        }
        .padding(LumiSpacing.large)
        .frame(width: 210)
        .background(LumiColor.surface)
    }

    private func sectionTitle(_ value: PhraseRoleSettingsSection) -> String {
        switch value {
        case .general: copy("settings.general")
        case .phraseModel: "Phrase Model"
        case .planningDefaults: "Planning Defaults"
        }
    }

    private func sectionIcon(_ value: PhraseRoleSettingsSection) -> String {
        switch value {
        case .general: "slider.horizontal.3"
        case .phraseModel: "text.badge.checkmark"
        case .planningDefaults: "point.3.connected.trianglepath.dotted"
        }
    }

    private func settingsSectionButton(
        _ value: PhraseRoleSettingsSection,
        title: String,
        icon: String
    ) -> some View {
        Button {
            section = value
        } label: {
            Label(title, systemImage: icon)
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: LumiControlMetric.standardHeight)
                .padding(.horizontal, LumiSpacing.small)
                .foregroundStyle(section == value ? LumiColor.accent : LumiColor.textPrimary)
                .background(section == value ? LumiColor.accent.opacity(0.14) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.settings.section.\(value.rawValue)")
    }

    @ViewBuilder
    private var content: some View {
        switch section {
        case .general:
            generalSettings
        case .phraseModel:
            phraseRoleSettings
        case .planningDefaults:
            planningDefaults
        }
    }

    private var planningDefaults: some View {
        ContentUnavailableView(
            "Planning Defaults",
            systemImage: "point.3.connected.trianglepath.dotted",
            description: Text("Default theme-selection and planning policies will be configured here in a later epic.")
        )
        .accessibilityIdentifier("lumi.settings.planningDefaults")
    }

    private var generalSettings: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                sectionHeading(copy("settings.general"), copy("settings.generalDetail"))
                LumiPanel {
                    VStack(spacing: LumiSpacing.large) {
                        settingPicker(
                            title: copy("settings.appearance"),
                            detail: copy("settings.appearanceDetail"),
                            selection: $appearance,
                            values: AppearancePreference.allCases,
                            titleForValue: { $0.titleKey }
                        )
                        Divider()
                        settingPicker(
                            title: copy("settings.keyNotation"),
                            detail: copy("settings.keyNotationDetail"),
                            selection: $keyNotation,
                            values: KeyNotationPreference.allCases,
                            titleForValue: { $0.titleKey }
                        )
                    }
                }
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 820, alignment: .leading)
        }
    }

    private var phraseRoleSettings: some View {
        Group {
            if let settings {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    HStack {
                        sectionHeading(
                            "Phrase Model",
                            "Maintain Lumi-owned phrase types. Display names may change; stable IDs and track references do not."
                        )
                        Spacer()
                        Button {
                            showsAddRole = true
                        } label: {
                            Label(copy("settings.addRole"), systemImage: "plus")
                        }
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("lumi.settings.roles.add")
                    }
                    HStack(spacing: LumiSpacing.large) {
                        roleList(settings)
                            .frame(width: 360)
                        roleInspector(settings)
                            .frame(maxWidth: .infinity)
                    }
                    if let feedback {
                        feedbackView(feedback)
                    }
                }
                .padding(LumiSpacing.xLarge)
            } else {
                unavailableSettings
            }
        }
    }

    private func roleList(_ settings: PhraseRoleSettingsState) -> some View {
        LumiPanel {
            if rendersInteractiveControls {
                ScrollView {
                    LazyVStack(spacing: LumiSpacing.xSmall) {
                        roleRows(settings.roles)
                    }
                }
            } else {
                VStack(spacing: LumiSpacing.xSmall) {
                    roleRows(settings.roles)
                }
            }
        }
    }

    @ViewBuilder
    private func roleRows(_ roles: [PhraseRoleDefinition]) -> some View {
        ForEach(roles) { role in
            Button {
                selectedRoleID = role.id
                renameDraft = role.name
            } label: {
                HStack(spacing: LumiSpacing.medium) {
                    Image(systemName: role.archived ? "archivebox.fill" : "circle.fill")
                        .font(.system(size: role.archived ? 12 : 7))
                        .foregroundStyle(role.archived ? LumiColor.textSecondary : LumiColor.accent)
                        .frame(width: 18)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(role.name)
                            .font(LumiTypography.body.weight(.semibold))
                            .foregroundStyle(LumiColor.textPrimary)
                        Text(role.id)
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Text("\(role.usage.trackCount)")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Image(systemName: "chevron.right")
                        .foregroundStyle(LumiColor.textSecondary)
                }
                .padding(.horizontal, LumiSpacing.medium)
                .frame(height: rendersInteractiveControls ? 54 : 48)
                .background(selectedRoleID == role.id ? LumiColor.accent.opacity(0.13) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(role.name), stable ID \(role.id), used by \(role.usage.trackCount) tracks")
            .accessibilityIdentifier("lumi.settings.role.\(role.id)")
        }
    }

    @ViewBuilder
    private func roleInspector(_ settings: PhraseRoleSettingsState) -> some View {
        let role = settings.roles.first { $0.id == selectedRoleID } ?? settings.roles.first
        if let role {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    HStack {
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(role.name)
                                .font(LumiTypography.cardTitle)
                            Text("Stable ID · \(role.id)")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        if role.archived {
                            Label(copy("settings.archived"), systemImage: "archivebox.fill")
                                .foregroundStyle(LumiColor.warning)
                        }
                    }

                    Divider()
                    Text(copy("settings.displayName"))
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(LumiColor.textSecondary)
                    if rendersInteractiveControls {
                        HStack {
                            TextField(copy("settings.roleName"), text: $renameDraft)
                                .textFieldStyle(.roundedBorder)
                                .accessibilityIdentifier("lumi.settings.role.name")
                            Button(copy("settings.saveName")) {
                                onMutation(.rename(roleID: role.id, displayName: renameDraft))
                            }
                            .disabled(renameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || renameDraft == role.name)
                        }
                    } else {
                        staticControl(role.name)
                    }

                    HStack {
                        Button {
                            onMutation(.moveEarlier(roleID: role.id))
                        } label: {
                            Label(copy("settings.moveEarlier"), systemImage: "arrow.up")
                        }
                        .disabled(role.sortOrder == 1)
                        Button {
                            onMutation(.moveLater(roleID: role.id))
                        } label: {
                            Label(copy("settings.moveLater"), systemImage: "arrow.down")
                        }
                        .disabled(Int(role.sortOrder) == settings.roles.count)
                        Spacer()
                        Button(role.archived ? copy("settings.restore") : copy("settings.archive")) {
                            onMutation(role.archived ? .restore(roleID: role.id) : .archive(roleID: role.id))
                        }
                        .tint(role.archived ? LumiColor.accent : LumiColor.warning)
                    }
                    .buttonStyle(.bordered)

                    Divider()
                    usageDiagnostics(role)
                    Spacer()
                    Label(copy("settings.stableIDDetail"), systemImage: "link.badge.plus")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
    }

    private func usageDiagnostics(_ role: PhraseRoleDefinition) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            Text(copy("settings.usage"))
                .font(LumiTypography.caption.weight(.semibold))
            HStack(spacing: LumiSpacing.medium) {
                usageMetric(copy("settings.tracks"), role.usage.trackCount)
                usageMetric(copy("settings.phrases"), role.usage.phraseCount)
                usageMetric(copy("settings.catalogRows"), role.usage.catalogRowCount)
            }
            if role.usage.affectedTracks.isEmpty {
                Text(copy("settings.unusedRole"))
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            } else {
                ForEach(role.usage.affectedTracks.prefix(5)) { track in
                    HStack {
                        Image(systemName: "music.note")
                        Text(track.title)
                        Spacer()
                        Text("\(track.phraseCount) \(copy("settings.phrases").lowercased())")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                }
            }
        }
    }

    private var unavailableSettings: some View {
        ContentUnavailableView(
            copy("settings.unavailable"),
            systemImage: "exclamationmark.triangle",
            description: Text(copy("settings.unavailableDetail"))
        )
    }

    private func sectionHeading(_ title: String, _ detail: String) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text(title)
                .font(LumiTypography.cardTitle)
            Text(detail)
                .font(LumiTypography.body)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private func usageMetric(_ title: String, _ value: UInt64) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text("\(value)")
                .font(LumiTypography.cardTitle)
            Text(title)
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .padding(LumiSpacing.medium)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func feedbackView(_ value: String) -> some View {
        Label(value, systemImage: "checkmark.circle")
            .font(LumiTypography.caption)
            .foregroundStyle(value.lowercased().contains("could not") ? LumiColor.warning : LumiColor.success)
            .accessibilityIdentifier("lumi.settings.feedback")
    }

    private func staticControl(_ value: String) -> some View {
        HStack {
            Text(value)
            Spacer()
            Image(systemName: "chevron.up.chevron.down")
        }
        .font(LumiTypography.body)
        .padding(.horizontal, LumiSpacing.medium)
        .frame(height: LumiControlMetric.standardHeight)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func staticLocalizedControl(_ value: LocalizedStringKey) -> some View {
        HStack {
            Text(value)
            Spacer()
            Image(systemName: "chevron.up.chevron.down")
        }
        .font(LumiTypography.body)
        .padding(.horizontal, LumiSpacing.medium)
        .frame(height: LumiControlMetric.standardHeight)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func settingPicker<Value: Hashable & Identifiable>(
        title: String,
        detail: String,
        selection: Binding<Value>,
        values: [Value],
        titleForValue: @escaping (Value) -> LocalizedStringKey
    ) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(title)
                    .font(LumiTypography.body.weight(.semibold))
                Text(detail)
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            if rendersInteractiveControls {
                Picker(title, selection: selection) {
                    ForEach(values) { value in
                        Text(titleForValue(value)).tag(value)
                    }
                }
                .labelsHidden()
                .frame(width: 220)
            } else {
                staticLocalizedControl(titleForValue(selection.wrappedValue))
                    .frame(width: 220)
            }
        }
    }

    private func copy(_ key: String) -> String {
        LibraryWorkspaceLocalization.value(key)
    }

    private func synchronizeSelection() {
        guard let settings else { return }
        if !settings.roles.contains(where: { $0.id == selectedRoleID }),
           let first = settings.roles.first {
            selectedRoleID = first.id
            renameDraft = first.name
        }
    }
}
