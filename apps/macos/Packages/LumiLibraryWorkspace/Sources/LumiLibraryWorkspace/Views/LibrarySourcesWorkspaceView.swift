import LumiDesignSystem
import SwiftUI

public struct LibrarySourcesWorkspaceView: View {
    private let library: LibraryWorkspaceState
    private let settings: PhraseRoleSettingsState?
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (PhraseRoleMutationRequest) -> Void

    @State private var selectedProviderKind: String?

    public init(
        library: LibraryWorkspaceState,
        settings: PhraseRoleSettingsState?,
        feedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        onMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in }
    ) {
        self.library = library
        self.settings = settings
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onMutation = onMutation
        _selectedProviderKind = State(initialValue: settings?.mappingProfiles.first?.providerKind)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                header
                rekordboxSource
                activeSource
                sourceMappings
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 980, alignment: .leading)
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.library.sources")
        .onChange(of: settings?.revision) { _, _ in synchronizeProvider() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text("Sources & Import")
                .font(LumiTypography.screenTitle)
            Text("Connect local music-library sources, inspect import state and configure source-specific initial phrase mapping.")
                .font(LumiTypography.body)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private var rekordboxSource: some View {
        LumiPanel {
            HStack(alignment: .top, spacing: LumiSpacing.large) {
                sourceIcon("r.square.fill", state: .empty)
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    HStack {
                        Text("Rekordbox 7")
                            .font(LumiTypography.cardTitle)
                        Text("LOCAL · READ ONLY")
                            .font(LumiTypography.technical)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(LumiColor.surfaceElevated)
                            .clipShape(Capsule())
                    }
                    Text("Not configured")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Text("Direct, read-only import is the next integration milestone. Lumi will require Rekordbox to be closed before reading its source.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Button("Import / Refresh") {}
                    .buttonStyle(.borderedProminent)
                    .disabled(true)
                    .help("Available after the Rekordbox 7 adapter is implemented")
                    .accessibilityIdentifier("lumi.library.sources.rekordbox.import")
            }
        }
    }

    @ViewBuilder
    private var activeSource: some View {
        if let source = library.source {
            LumiPanel {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    sourceIcon("shippingbox.fill", state: library.condition.componentState)
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        HStack {
                            Text(source.name).font(LumiTypography.cardTitle)
                            StatusBadge("ACTIVE", state: library.condition.componentState)
                        }
                        Text("\(library.collectionTotal) tracks · revision \(source.revision)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text("The local demo source remains available for dry-running Library, Local Play and planning while Rekordbox import is being built.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                }
            }
        }
    }

    @ViewBuilder
    private var sourceMappings: some View {
        if let settings, !settings.mappingProfiles.isEmpty {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("Initial Phrase Mapping")
                        .font(LumiTypography.cardTitle)
                    Text("Map source phrases once during import. After import, Lumi-owned phrases evolve independently in the Track Editor.")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                providerTabs(settings)
                mappingTable(settings)
                Label(settings.mappingPolicy, systemImage: "lock.shield")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                if let feedback {
                    Label(feedback, systemImage: "checkmark.circle")
                        .font(LumiTypography.caption)
                        .foregroundStyle(
                            feedback.lowercased().contains("could not")
                                ? LumiColor.warning
                                : LumiColor.success
                        )
                }
            }
        }
    }

    private func providerTabs(_ settings: PhraseRoleSettingsState) -> some View {
        HStack(spacing: LumiSpacing.small) {
            ForEach(settings.mappingProfiles) { profile in
                Button(profile.providerName) {
                    selectedProviderKind = profile.providerKind
                }
                .buttonStyle(.bordered)
                .tint(selectedProviderKind == profile.providerKind ? LumiColor.accent : LumiColor.textSecondary)
                .accessibilityIdentifier("lumi.library.sources.mapping.\(profile.providerKind)")
            }
        }
    }

    @ViewBuilder
    private func mappingTable(_ settings: PhraseRoleSettingsState) -> some View {
        let profile = settings.mappingProfiles.first { $0.providerKind == selectedProviderKind }
            ?? settings.mappingProfiles.first
        if let profile {
            LumiPanel {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        Text("Raw source phrase")
                        Spacer()
                        Text("Lumi phrase type")
                            .frame(width: 250, alignment: .leading)
                    }
                    .font(LumiTypography.caption.weight(.semibold))
                    .foregroundStyle(LumiColor.textSecondary)
                    .padding(.bottom, LumiSpacing.medium)
                    Divider()
                    ForEach(profile.mappings) { mapping in
                        mappingRow(mapping, profile: profile, roles: settings.roles)
                    }
                }
            }
        }
    }

    private func mappingRow(
        _ mapping: SourcePhraseMapping,
        profile: SourcePhraseMappingProfile,
        roles: [PhraseRoleDefinition]
    ) -> some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(mapping.rawLabel == "*" ? "Other source phrases" : mapping.rawLabel)
                        .font(LumiTypography.body.weight(.semibold))
                    Text(mapping.rawLabel)
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                if rendersInteractiveControls {
                    Picker("", selection: mappingBinding(mapping, profile: profile)) {
                        ForEach(roles.filter { !$0.archived || $0.id == mapping.roleID }) { role in
                            Text(role.archived ? "\(role.name) · Archived" : role.name)
                                .tag(role.id)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 250)
                } else {
                    Text(roles.first { $0.id == mapping.roleID }?.name ?? mapping.roleID)
                        .frame(width: 250, alignment: .leading)
                }
            }
            .padding(.vertical, LumiSpacing.small)
            Divider()
        }
    }

    private func mappingBinding(
        _ mapping: SourcePhraseMapping,
        profile: SourcePhraseMappingProfile
    ) -> Binding<String> {
        Binding(
            get: { mapping.roleID },
            set: { roleID in
                guard roleID != mapping.roleID else { return }
                onMutation(
                    .setSourceMapping(
                        providerKind: profile.providerKind,
                        rawLabel: mapping.rawLabel,
                        roleID: roleID
                    )
                )
            }
        )
    }

    private func sourceIcon(_ systemName: String, state: LumiComponentState) -> some View {
        Image(systemName: systemName)
            .font(.system(size: 22, weight: .semibold))
            .foregroundStyle(state.color)
            .frame(width: 46, height: 46)
            .background(state.color.opacity(0.14))
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func synchronizeProvider() {
        guard let settings else { return }
        if !settings.mappingProfiles.contains(where: { $0.providerKind == selectedProviderKind }) {
            selectedProviderKind = settings.mappingProfiles.first?.providerKind
        }
    }
}
