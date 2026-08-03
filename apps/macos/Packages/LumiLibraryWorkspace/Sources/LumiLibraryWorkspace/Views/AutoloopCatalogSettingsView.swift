import LumiDesignSystem
import SwiftUI

public struct AutoloopCatalogSettingsView: View {
    private let catalog: AutoloopCatalogState?
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (AutoloopCatalogMutationRequest) -> Void

    @State private var selectedThemeID: UInt64?
    @State private var selectedRoleID: String?
    @State private var selectedVariantID: String?
    @State private var themeNameDraft = ""
    @State private var variantNameDraft = ""
    @State private var cellNameDraft = ""
    @State private var newVariantName = ""
    @State private var showsAddVariant = false

    public init(
        catalog: AutoloopCatalogState?,
        feedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        onMutation: @escaping @Sendable (AutoloopCatalogMutationRequest) -> Void = { _ in }
    ) {
        self.catalog = catalog
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onMutation = onMutation
        _selectedThemeID = State(initialValue: catalog?.themes.first?.id)
        _selectedRoleID = State(initialValue: catalog?.roles.first(where: { !$0.archived })?.id)
        _selectedVariantID = State(
            initialValue: catalog?.roles.first(where: { !$0.archived })?.variants.first?.id
        )
        _themeNameDraft = State(initialValue: catalog?.themes.first?.name ?? "")
        _variantNameDraft = State(
            initialValue: catalog?.roles.first(where: { !$0.archived })?.variants.first?.name ?? ""
        )
        _cellNameDraft = State(
            initialValue: catalog?.roles.first(where: { !$0.archived })?.variants.first?.cells.first?.name ?? ""
        )
    }

    public var body: some View {
        Group {
            if let catalog {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    heading(catalog)
                    themeTabs(catalog)
                    HStack(spacing: LumiSpacing.large) {
                        roleList(catalog)
                            .frame(width: 290)
                        matrixEditor(catalog)
                            .frame(maxWidth: .infinity)
                    }
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
                .padding(LumiSpacing.xLarge)
                .onChange(of: catalog.revision) { _, _ in synchronize(catalog) }
                .alert(copy("settings.addVariant"), isPresented: $showsAddVariant) {
                    TextField(copy("settings.variantName"), text: $newVariantName)
                    Button(copy("settings.cancel"), role: .cancel) { newVariantName = "" }
                    Button(copy("settings.add")) {
                        guard let selectedRoleID else { return }
                        let name = newVariantName.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !name.isEmpty else { return }
                        onMutation(.addVariant(roleID: selectedRoleID, displayName: name))
                        newVariantName = ""
                    }
                }
            } else {
                ContentUnavailableView(
                    copy("settings.autoloopUnavailable"),
                    systemImage: "square.grid.3x3",
                    description: Text(copy("settings.autoloopUnavailableDetail"))
                )
            }
        }
        .accessibilityIdentifier("lumi.settings.autoloopMatrix")
    }

    private func heading(_ catalog: AutoloopCatalogState) -> some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(copy("settings.autoloopMatrix"))
                    .font(LumiTypography.cardTitle)
                Text(copy("settings.autoloopMatrixDetail"))
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            Label(
                catalog.preflight.status == "ready"
                    ? copy("settings.preflightReady")
                    : preflightSummary(catalog.preflight),
                systemImage: catalog.preflight.status == "ready"
                    ? "checkmark.shield.fill"
                    : "exclamationmark.triangle.fill"
            )
            .font(LumiTypography.caption.weight(.semibold))
            .foregroundStyle(
                catalog.preflight.status == "ready" ? LumiColor.success : LumiColor.warning
            )
        }
    }

    private func preflightSummary(_ preflight: AutoloopPreflightState) -> String {
        var issues: [String] = []
        if preflight.missingCellCount > 0 {
            issues.append("\(preflight.missingCellCount) \(copy("settings.missingCells"))")
        }
        if preflight.missingRoleCount > 0 {
            issues.append("\(preflight.missingRoleCount) \(copy("settings.rolesWithoutVariants"))")
        }
        return issues.joined(separator: " · ")
    }

    private func themeTabs(_ catalog: AutoloopCatalogState) -> some View {
        HStack(spacing: LumiSpacing.small) {
            ForEach(catalog.themes) { theme in
                Button {
                    selectTheme(theme, catalog: catalog)
                } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("\(copy("settings.themeTarget")) \(theme.sortOrder)")
                            .font(LumiTypography.technical)
                        Text(theme.name)
                            .font(LumiTypography.body.weight(.semibold))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(height: 58)
                }
                .buttonStyle(.plain)
                .foregroundStyle(
                    selectedThemeID == theme.id ? LumiColor.accent : LumiColor.textPrimary
                )
                .background(
                    selectedThemeID == theme.id
                        ? LumiColor.accent.opacity(0.14)
                        : LumiColor.surfaceElevated
                )
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                .overlay(
                    RoundedRectangle(cornerRadius: LumiRadius.control)
                        .stroke(
                            selectedThemeID == theme.id ? LumiColor.accent : LumiColor.border,
                            lineWidth: 1
                        )
                )
                .accessibilityIdentifier("lumi.settings.autoloop.theme.\(theme.id)")
            }
        }
    }

    private func roleList(_ catalog: AutoloopCatalogState) -> some View {
        LumiPanel {
            if rendersInteractiveControls {
                ScrollView {
                    LazyVStack(spacing: LumiSpacing.xSmall) {
                        roleRows(catalog)
                    }
                }
            } else {
                VStack(spacing: LumiSpacing.xSmall) {
                    roleRows(catalog)
                }
            }
        }
    }

    @ViewBuilder
    private func roleRows(_ catalog: AutoloopCatalogState) -> some View {
        ForEach(catalog.roles.filter { !$0.archived }) { role in
            Button {
                selectRole(role, catalog: catalog)
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(role.name)
                            .font(LumiTypography.body.weight(.semibold))
                        Text(role.id)
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Text("\(role.variants.filter { !$0.archived }.count)")
                        .font(LumiTypography.technical)
                    Image(systemName: "chevron.right")
                }
                .padding(.horizontal, LumiSpacing.medium)
                .frame(height: 52)
                .background(
                    selectedRoleID == role.id
                        ? LumiColor.accent.opacity(0.13)
                        : Color.clear
                )
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("lumi.settings.autoloop.role.\(role.id)")
        }
    }

    @ViewBuilder
    private func matrixEditor(_ catalog: AutoloopCatalogState) -> some View {
        let role = catalog.roles.first { $0.id == selectedRoleID }
            ?? catalog.roles.first { !$0.archived }
        let theme = catalog.themes.first { $0.id == selectedThemeID } ?? catalog.themes.first
        if let role, let theme {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    themeNameEditor(theme)
                    Divider()
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(role.name)
                                .font(LumiTypography.cardTitle)
                            Text("\(copy("settings.autoloopCategory")) · \(role.id)")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        Button {
                            showsAddVariant = true
                        } label: {
                            Label(copy("settings.addVariant"), systemImage: "plus")
                        }
                        .buttonStyle(.borderedProminent)
                    }
                    variantRows(role, theme: theme)
                    Label(copy("settings.targetCapacityDetail"), systemImage: "shippingbox")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
    }

    private func themeNameEditor(_ theme: AutoloopThemeState) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(copy("settings.themeName"))
                    .font(LumiTypography.caption.weight(.semibold))
                Text(copy("settings.themeNameDetail"))
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            if rendersInteractiveControls {
                TextField(copy("settings.themeName"), text: $themeNameDraft)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 230)
                Button(copy("settings.saveName")) {
                    onMutation(.renameTheme(themeID: theme.id, displayName: themeNameDraft))
                }
                .disabled(
                    themeNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || themeNameDraft == theme.name
                )
            } else {
                Text(theme.name)
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(width: 300, height: LumiControlMetric.standardHeight, alignment: .leading)
                    .background(LumiColor.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            }
        }
    }

    private func variantRows(
        _ role: AutoloopRoleMatrixState,
        theme: AutoloopThemeState
    ) -> some View {
        VStack(spacing: LumiSpacing.small) {
            ForEach(role.variants) { variant in
                let cell = variant.cells.first { $0.themeID == theme.id }
                Button {
                    selectedVariantID = variant.id
                    variantNameDraft = variant.name
                    cellNameDraft = cell?.name ?? ""
                } label: {
                    HStack(spacing: LumiSpacing.medium) {
                        Image(systemName: variant.archived ? "archivebox.fill" : "circle.fill")
                            .font(.system(size: variant.archived ? 12 : 7))
                            .foregroundStyle(
                                variant.archived ? LumiColor.textSecondary : LumiColor.accent
                            )
                        VStack(alignment: .leading, spacing: 2) {
                            Text(variant.name)
                                .font(LumiTypography.body.weight(.semibold))
                            Text("\(role.id) / \(variant.id)")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        Text(cell?.isMissing == false ? cell?.name ?? "" : copy("settings.missing"))
                            .font(LumiTypography.caption)
                            .foregroundStyle(
                                cell?.isMissing == false ? LumiColor.textPrimary : LumiColor.warning
                            )
                            .lineLimit(1)
                        Image(systemName: "chevron.right")
                    }
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(height: 56)
                    .background(
                        selectedVariantID == variant.id
                            ? LumiColor.accent.opacity(0.12)
                            : LumiColor.surfaceElevated
                    )
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                }
                .buttonStyle(.plain)
            }
            variantInspector(role, theme: theme)
        }
    }

    @ViewBuilder
    private func variantInspector(
        _ role: AutoloopRoleMatrixState,
        theme: AutoloopThemeState
    ) -> some View {
        let variant = role.variants.first { $0.id == selectedVariantID } ?? role.variants.first
        if let variant {
            let cell = variant.cells.first { $0.themeID == theme.id }
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                Divider()
                if rendersInteractiveControls {
                    HStack {
                        Text(copy("settings.selectedVariant"))
                            .font(LumiTypography.caption.weight(.semibold))
                        Spacer()
                        Button(copy("settings.moveEarlier")) {
                            onMutation(.moveVariantEarlier(roleID: role.id, variantID: variant.id))
                        }
                        .disabled(variant.sortOrder == 1)
                        Button(copy("settings.moveLater")) {
                            onMutation(.moveVariantLater(roleID: role.id, variantID: variant.id))
                        }
                        .disabled(Int(variant.sortOrder) == role.variants.count)
                        Button(variant.archived ? copy("settings.restore") : copy("settings.archive")) {
                            onMutation(
                                variant.archived
                                    ? .restoreVariant(roleID: role.id, variantID: variant.id)
                                    : .archiveVariant(roleID: role.id, variantID: variant.id)
                            )
                        }
                    }
                    .buttonStyle(.bordered)
                    HStack {
                        TextField(copy("settings.variantName"), text: $variantNameDraft)
                            .textFieldStyle(.roundedBorder)
                        Button(copy("settings.saveVariant")) {
                            onMutation(
                                .renameVariant(
                                    roleID: role.id,
                                    variantID: variant.id,
                                    displayName: variantNameDraft
                                )
                            )
                        }
                        .disabled(
                            variantNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                || variantNameDraft == variant.name
                        )
                    }
                    HStack {
                        TextField(copy("settings.logicalEntry"), text: $cellNameDraft)
                            .textFieldStyle(.roundedBorder)
                        Button(cell?.isMissing == false ? copy("settings.saveEntry") : copy("settings.createEntry")) {
                            let name = cellNameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
                            guard !name.isEmpty else { return }
                            onMutation(
                                .setCell(
                                    themeID: theme.id,
                                    roleID: role.id,
                                    variantID: variant.id,
                                    displayName: name
                                )
                            )
                        }
                        .disabled(cellNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        if cell?.isMissing == false {
                            Button(copy("settings.markMissing"), role: .destructive) {
                                onMutation(
                                    .setCell(
                                        themeID: theme.id,
                                        roleID: role.id,
                                        variantID: variant.id,
                                        displayName: nil
                                    )
                                )
                            }
                        }
                    }
                } else {
                    Text(copy("settings.selectedVariant"))
                        .font(LumiTypography.caption.weight(.semibold))
                    staticField(copy("settings.variantName"), value: variant.name)
                    staticField(
                        copy("settings.logicalEntry"),
                        value: cell?.name ?? copy("settings.missing"),
                        warning: cell?.isMissing != false
                    )
                }
            }
        }
    }

    private func staticField(_ label: String, value: String, warning: Bool = false) -> some View {
        HStack {
            Text(label)
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(LumiColor.textSecondary)
                .frame(width: 120, alignment: .leading)
            Text(value)
                .font(LumiTypography.body)
                .foregroundStyle(warning ? LumiColor.warning : LumiColor.textPrimary)
                .lineLimit(1)
            Spacer()
        }
        .padding(.horizontal, LumiSpacing.medium)
        .frame(height: LumiControlMetric.standardHeight)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func selectTheme(_ theme: AutoloopThemeState, catalog: AutoloopCatalogState) {
        selectedThemeID = theme.id
        themeNameDraft = theme.name
        refreshCellDraft(catalog)
    }

    private func selectRole(_ role: AutoloopRoleMatrixState, catalog: AutoloopCatalogState) {
        selectedRoleID = role.id
        selectedVariantID = role.variants.first?.id
        variantNameDraft = role.variants.first?.name ?? ""
        refreshCellDraft(catalog)
    }

    private func refreshCellDraft(_ catalog: AutoloopCatalogState) {
        cellNameDraft = catalog.roles
            .first { $0.id == selectedRoleID }?
            .variants.first { $0.id == selectedVariantID }?
            .cells.first { $0.themeID == selectedThemeID }?
            .name ?? ""
    }

    private func synchronize(_ catalog: AutoloopCatalogState) {
        if !catalog.themes.contains(where: { $0.id == selectedThemeID }) {
            selectedThemeID = catalog.themes.first?.id
        }
        if !catalog.roles.contains(where: { $0.id == selectedRoleID && !$0.archived }) {
            selectedRoleID = catalog.roles.first { !$0.archived }?.id
        }
        let role = catalog.roles.first { $0.id == selectedRoleID }
        if role?.variants.contains(where: { $0.id == selectedVariantID }) != true {
            selectedVariantID = role?.variants.first?.id
        }
        themeNameDraft = catalog.themes.first { $0.id == selectedThemeID }?.name ?? ""
        variantNameDraft = role?.variants.first { $0.id == selectedVariantID }?.name ?? ""
        refreshCellDraft(catalog)
    }

    private func copy(_ key: String) -> String {
        LibraryWorkspaceLocalization.value(key)
    }
}
