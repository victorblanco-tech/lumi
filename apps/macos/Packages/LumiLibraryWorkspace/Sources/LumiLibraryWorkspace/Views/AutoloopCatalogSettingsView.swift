import Foundation
import LumiDesignSystem
import SwiftUI

public struct AutoloopCatalogSettingsView: View {
    private enum ProfileSection: String, CaseIterable, Identifiable {
        case banks
        case controller
        case midi

        var id: String { rawValue }
    }

    private let catalog: AutoloopCatalogState?
    private let profile = SoundSwitchOutputProfileState.builtIn
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (AutoloopCatalogMutationRequest) -> Void

    @State private var section: ProfileSection = .banks
    @State private var selectedBankID: UInt64?
    @State private var selectedSlotNumber: UInt16 = 1
    @State private var activePage: UInt16 = 1
    @State private var bankNameDraft = ""
    @State private var variantNameDraft = ""
    @State private var entryNameDraft = ""
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
        let firstBank = catalog?.themes.first
        _selectedBankID = State(initialValue: firstBank?.id)
        _bankNameDraft = State(initialValue: firstBank?.name ?? "")
        let firstSlot = catalog.flatMap { value in
            firstBank.flatMap {
                SoundSwitchOutputProfileProjection.slots(for: $0.id, catalog: value).first
            }
        }
        _variantNameDraft = State(initialValue: firstSlot?.variantName ?? "")
        _entryNameDraft = State(initialValue: firstSlot?.entryName ?? "")
    }

    public var body: some View {
        Group {
            if let catalog {
                VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                    profileHeader(catalog)
                    sectionTabs
                    switch section {
                    case .banks:
                        banksAndAutoloops(catalog)
                    case .controller:
                        virtualController(catalog)
                    case .midi:
                        midiPreparation(catalog)
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
                .padding(LumiSpacing.large)
                .onChange(of: catalog.revision) { _, _ in synchronize(catalog) }
                .alert(copy("settings.addVariant"), isPresented: $showsAddVariant) {
                    TextField(copy("settings.variantName"), text: $newVariantName)
                    Button(copy("settings.cancel"), role: .cancel) { newVariantName = "" }
                    Button(copy("settings.add")) {
                        guard let roleID = selectedSlot(catalog)?.roleID else { return }
                        let name = newVariantName.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !name.isEmpty else { return }
                        onMutation(.addVariant(roleID: roleID, displayName: name))
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
        .accessibilityIdentifier("lumi.settings.outputProfiles")
    }

    private func profileHeader(_ catalog: AutoloopCatalogState) -> some View {
        LumiPanel {
            HStack(spacing: LumiSpacing.medium) {
                Image(systemName: "slider.horizontal.3")
                    .font(.system(size: 20, weight: .semibold))
                    .foregroundStyle(LumiColor.accent)
                    .frame(width: 40, height: 40)
                    .background(LumiColor.accent.opacity(0.14))
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                VStack(alignment: .leading, spacing: 2) {
                    Text(profile.name)
                        .font(LumiTypography.cardTitle)
                    Text("Main lighting · CoreMIDI · \(profile.controllerName)")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Text("BUILT-IN")
                    .font(LumiTypography.technical)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(LumiColor.surfaceElevated)
                    .clipShape(Capsule())
                Spacer()
                Label("Demo configuration", systemImage: "shippingbox.fill")
                    .font(LumiTypography.caption.weight(.semibold))
                    .foregroundStyle(LumiColor.accent)
                Label(
                    catalog.preflight.status == "ready" ? "Catalog ready" : "Catalog incomplete",
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
    }

    private var sectionTabs: some View {
        HStack(spacing: 4) {
            profileTab(.banks, "Banks & Autoloops")
            profileTab(.controller, "Virtual Controller")
            profileTab(.midi, "MIDI & POC")
            Spacer()
        }
        .overlay(alignment: .bottom) { Divider() }
    }

    private func profileTab(_ value: ProfileSection, _ title: String) -> some View {
        Button(title) { section = value }
            .buttonStyle(.plain)
            .font(LumiTypography.body.weight(.semibold))
            .foregroundStyle(section == value ? LumiColor.accent : LumiColor.textSecondary)
            .padding(.horizontal, LumiSpacing.medium)
            .frame(height: 38)
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(section == value ? LumiColor.accent : Color.clear)
                    .frame(height: 2)
            }
            .accessibilityIdentifier("lumi.settings.outputProfiles.\(value.rawValue)")
    }

    private func banksAndAutoloops(_ catalog: AutoloopCatalogState) -> some View {
        VStack(spacing: LumiSpacing.medium) {
            bankTabs(catalog)
            HStack(alignment: .top, spacing: LumiSpacing.medium) {
                bankSurface(catalog)
                    .frame(maxWidth: .infinity)
                mappingInspector(catalog)
                    .frame(width: 288)
            }
        }
    }

    private func bankTabs(_ catalog: AutoloopCatalogState) -> some View {
        HStack(spacing: LumiSpacing.small) {
            ForEach(catalog.themes) { bank in
                let projectedBank = SoundSwitchOutputProfileProjection.banks(catalog: catalog)
                    .first { $0.id == bank.id }
                let mapped = SoundSwitchOutputProfileProjection.mappedCount(
                    for: bank.id,
                    catalog: catalog
                )
                Button {
                    selectBank(bank, catalog: catalog)
                } label: {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(
                            "BANK \(bank.sortOrder) · "
                                + (projectedBank?.organization.displayName.uppercased() ?? "CUSTOM")
                        )
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text(bank.name)
                            .font(LumiTypography.body.weight(.semibold))
                            .lineLimit(1)
                        Text("\(mapped) / \(profile.slotsPerBank) mapped")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(height: 68)
                }
                .buttonStyle(.plain)
                .foregroundStyle(selectedBankID == bank.id ? LumiColor.accent : LumiColor.textPrimary)
                .background(
                    selectedBankID == bank.id
                        ? LumiColor.accent.opacity(0.14)
                        : LumiColor.surfaceElevated
                )
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                .overlay {
                    RoundedRectangle(cornerRadius: LumiRadius.control)
                        .stroke(selectedBankID == bank.id ? LumiColor.accent : LumiColor.border)
                }
                .accessibilityIdentifier("lumi.settings.outputProfiles.bank.\(bank.id)")
            }
        }
    }

    private func bankSurface(_ catalog: AutoloopCatalogState) -> some View {
        let bank = selectedBank(catalog)
        let slots = slots(catalog)
        let columns = Array(repeating: GridItem(.flexible(), spacing: 7), count: 4)
        return LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Bank \(bank?.sortOrder ?? 1) · \(bank?.name ?? "")")
                            .font(LumiTypography.cardTitle)
                        Text("SoundSwitch Autoloop positions · logical row order")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Text("\(slots.filter { $0.status == .mapped }.count) / \(profile.slotsPerBank) mapped")
                        .font(LumiTypography.technical)
                }
                LazyVGrid(columns: columns, spacing: 7) {
                    ForEach(slots) { slot in
                        slotButton(slot, catalog: catalog)
                    }
                }
                HStack(spacing: LumiSpacing.large) {
                    legend(.mapped, "Mapped")
                    legend(.incomplete, "Incomplete")
                    legend(.available, "Available")
                    Spacer()
                    Text("Provider binding is added after the MIDI POC")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
    }

    private func slotButton(
        _ slot: SoundSwitchAutoloopSlotState,
        catalog: AutoloopCatalogState
    ) -> some View {
        Button {
            selectSlot(slot, catalog: catalog)
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(String(format: "%02d · %@", slot.number, slotLabel(slot)))
                    .font(LumiTypography.technical.weight(.bold))
                    .lineLimit(1)
                Text(slot.variantName ?? slotStatusLabel(slot.status))
                    .font(LumiTypography.caption)
                    .foregroundStyle(slotStatusColor(slot.status))
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 8)
            .frame(height: 43)
        }
        .buttonStyle(.plain)
        .foregroundStyle(LumiColor.textPrimary)
        .background(
            selectedSlotNumber == slot.number
                ? LumiColor.accent.opacity(0.15)
                : LumiColor.surfaceElevated
        )
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(
                    selectedSlotNumber == slot.number
                        ? LumiColor.accent
                        : slotStatusColor(slot.status).opacity(0.48)
                )
        }
        .accessibilityIdentifier("lumi.settings.outputProfiles.slot.\(slot.number)")
    }

    private func mappingInspector(_ catalog: AutoloopCatalogState) -> some View {
        let bank = selectedBank(catalog)
        let slot = selectedSlot(catalog)
        return LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                Text("Mapping Inspector")
                    .font(LumiTypography.cardTitle)
                inspectorHeading("BANK")
                if rendersInteractiveControls {
                    TextField("Bank name", text: $bankNameDraft)
                        .textFieldStyle(.roundedBorder)
                    Button("Save Bank Name") {
                        guard let bank else { return }
                        onMutation(.renameTheme(themeID: bank.id, displayName: bankNameDraft))
                    }
                    .disabled(
                        bankNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                            || bankNameDraft == bank?.name
                    )
                } else {
                    inspectorValue("Bank name", bank?.name ?? "")
                }
                HStack {
                    inspectorValue(
                        "Organization",
                        projectedBank(catalog)?.organization.displayName ?? "Custom"
                    )
                    inspectorValue("Target", "Bank \(bank?.sortOrder ?? 1)")
                }
                inspectorValue("Group", projectedBank(catalog)?.groupName ?? "Default")
                Text("Bank organization becomes configurable in the later profile builder.")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                Divider()
                inspectorHeading("SELECTED AUTOLOOP")
                HStack {
                    inspectorValue("Bank", "\(bank?.sortOrder ?? 1)")
                    inspectorValue("Slot", "\(slot?.number ?? 1)")
                }
                inspectorValue("Phrase Role", slot?.roleName ?? "Available")
                inspectorValue("Variant", slot?.variantName ?? "No logical row")
                if let roleID = slot?.roleID, let variantID = slot?.variantID {
                    if rendersInteractiveControls {
                        TextField("Variant name", text: $variantNameDraft)
                            .textFieldStyle(.roundedBorder)
                        HStack {
                            Button("Save Variant") {
                                onMutation(
                                    .renameVariant(
                                        roleID: roleID,
                                        variantID: variantID,
                                        displayName: variantNameDraft
                                    )
                                )
                            }
                            .disabled(
                                variantNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                    || variantNameDraft == slot?.variantName
                            )
                            Button {
                                showsAddVariant = true
                            } label: {
                                Label("Add Variant", systemImage: "plus")
                            }
                        }
                        TextField("Logical Autoloop name", text: $entryNameDraft)
                            .textFieldStyle(.roundedBorder)
                        HStack {
                            Button(slot?.status == .mapped ? "Save Mapping" : "Create Mapping") {
                                let name = entryNameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
                                guard let bank, !name.isEmpty else { return }
                                onMutation(
                                    .setCell(
                                        themeID: bank.id,
                                        roleID: roleID,
                                        variantID: variantID,
                                        displayName: name
                                    )
                                )
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(LumiColor.accent)
                            .disabled(
                                entryNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                    || entryNameDraft == slot?.entryName
                            )
                            if slot?.status == .mapped {
                                Button("Clear", role: .destructive) {
                                    guard let bank else { return }
                                    onMutation(
                                        .setCell(
                                            themeID: bank.id,
                                            roleID: roleID,
                                            variantID: variantID,
                                            displayName: nil
                                        )
                                    )
                                }
                            }
                        }
                    } else {
                        inspectorValue("Variant name", variantNameDraft)
                        inspectorValue("Logical Autoloop", entryNameDraft)
                    }
                } else {
                    Label(
                        "Available after another logical Phrase Role variant is added.",
                        systemImage: "plus.square.dashed"
                    )
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer(minLength: 0)
            }
        }
    }

    private func virtualController(_ catalog: AutoloopCatalogState) -> some View {
        let bank = selectedBank(catalog)
        let pageSlots = slots(catalog).filter { slot in
            let first = (activePage - 1) * profile.slotsPerPage + 1
            return slot.number >= first && slot.number < first + profile.slotsPerPage
        }
        let columns = Array(repeating: GridItem(.flexible(), spacing: 10), count: 4)
        return HStack(alignment: .top, spacing: LumiSpacing.medium) {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(profile.controllerName.uppercased())
                                .font(LumiTypography.cardTitle)
                            Text("Peer MIDI controller for SoundSwitch · not a Control One dependency")
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        Text("VIRTUAL SURFACE")
                            .font(LumiTypography.technical)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(LumiColor.surfaceElevated)
                            .clipShape(Capsule())
                    }
                    HStack(spacing: 8) {
                        ForEach(catalog.themes) { value in
                            Button("BANK \(value.sortOrder)\n\(value.name)") {
                                selectBank(value, catalog: catalog)
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(selectedBankID == value.id ? LumiColor.accent : LumiColor.surfaceElevated)
                            .frame(maxWidth: .infinity)
                        }
                    }
                    HStack {
                        Text("AUTOLOOP PAGES")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        ForEach(1...profile.pageCount, id: \.self) { page in
                            Button("\(page)") {
                                activePage = page
                                selectedSlotNumber = (page - 1) * profile.slotsPerPage + 1
                                refreshDrafts(catalog)
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(activePage == page ? LumiColor.accent : LumiColor.surfaceElevated)
                        }
                        Spacer()
                        Text("Bank \(bank?.sortOrder ?? 1) · Page \(activePage) / \(profile.pageCount)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    LazyVGrid(columns: columns, spacing: 10) {
                        ForEach(pageSlots) { slot in
                            Button {
                                selectSlot(slot, catalog: catalog)
                            } label: {
                                VStack(spacing: 5) {
                                    Text("AUTOLOOP \(slot.number)")
                                        .font(LumiTypography.technical.weight(.bold))
                                    Text("\(slotLabel(slot)) · \(slot.variantName ?? slotStatusLabel(slot.status))")
                                        .font(LumiTypography.caption)
                                        .lineLimit(1)
                                }
                                .frame(maxWidth: .infinity)
                                .frame(height: 68)
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(LumiColor.textPrimary)
                            .background(
                                selectedSlotNumber == slot.number
                                    ? LumiColor.accent.opacity(0.2)
                                    : LumiColor.surfaceElevated
                            )
                            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                            .overlay {
                                RoundedRectangle(cornerRadius: LumiRadius.control)
                                    .stroke(
                                        selectedSlotNumber == slot.number
                                            ? LumiColor.accent
                                            : slotStatusColor(slot.status).opacity(0.48)
                                    )
                            }
                        }
                    }
                    HStack {
                        controllerUtility("Previous Autoloop")
                        controllerUtility("Repeat Autoloop")
                        controllerUtility("Next Autoloop")
                        Spacer()
                        controllerUtility("Override Scripted Tracks")
                    }
                }
            }
            .frame(maxWidth: .infinity)
            virtualButtonInspector(catalog)
                .frame(width: 288)
        }
    }

    private func virtualButtonInspector(_ catalog: AutoloopCatalogState) -> some View {
        let bank = selectedBank(catalog)
        let slot = selectedSlot(catalog)
        return LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                Text("Virtual Button")
                    .font(LumiTypography.cardTitle)
                inspectorHeading("CONTROL")
                HStack {
                    inspectorValue("Bank", "\(bank?.sortOrder ?? 1)")
                    inspectorValue("Autoloop", "\(slot?.number ?? 1)")
                }
                inspectorHeading("SOUNDSWITCH BINDING")
                inspectorValue("Lumi element", slotBinding(slot))
                inspectorValue("MIDI address", "POC pending")
                Divider()
                Label(
                    "Lumi and Control One will be tested as parallel SoundSwitch controllers.",
                    systemImage: "arrow.triangle.branch"
                )
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
                Button("Test Button") {}
                    .buttonStyle(.borderedProminent)
                    .tint(LumiColor.accent)
                    .disabled(true)
                Text("Live sending is deliberately unavailable until the MIDI POC.")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                Spacer(minLength: 0)
            }
        }
    }

    private func midiPreparation(_ catalog: AutoloopCatalogState) -> some View {
        HStack(alignment: .top, spacing: LumiSpacing.medium) {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    Text("MIDI Transport")
                        .font(LumiTypography.cardTitle)
                    inspectorValue("Output device", "Lumi Virtual MIDI → SoundSwitch")
                    inspectorValue("Transport", "CoreMIDI virtual source")
                    inspectorValue("Timing", "Ableton Link → SoundSwitch")
                    HStack {
                        inspectorValue("Bank switch delay", "Measure in POC")
                        inspectorValue("Output requirement", "Required for Start")
                    }
                    Divider()
                    Text("CATALOG PREFLIGHT")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Label(
                        preflightSummary(catalog),
                        systemImage: catalog.preflight.status == "ready"
                            ? "checkmark.shield.fill"
                            : "exclamationmark.triangle.fill"
                    )
                    .foregroundStyle(
                        catalog.preflight.status == "ready" ? LumiColor.success : LumiColor.warning
                    )
                    Spacer(minLength: 0)
                }
            }
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    Text("POC Acceptance")
                        .font(LumiTypography.cardTitle)
                    pocRequirement("SoundSwitch discovers Lumi's virtual MIDI device")
                    pocRequirement("One bank and multiple Autoloops respond deterministically")
                    pocRequirement("Physical Control One remains usable in parallel")
                    pocRequirement("DMX output through Control One visibly drives fixtures")
                    pocRequirement("Disconnect and reconnect remain fail-silent")
                    Divider()
                    Text("SELECT BANK 1\nWAIT <measured> ms\nTRIGGER AUTOLOOP 1")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.accent)
                        .padding(LumiSpacing.medium)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(LumiColor.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                    HStack {
                        Button("Dry Run") {}
                        Button("Send Test") {}
                            .buttonStyle(.borderedProminent)
                            .tint(LumiColor.accent)
                    }
                    .disabled(true)
                    Text("Enabled by the next MIDI POC story.")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Spacer(minLength: 0)
                }
            }
        }
    }

    private func pocRequirement(_ text: String) -> some View {
        Label(text, systemImage: "circle.dashed")
            .font(LumiTypography.body)
            .foregroundStyle(LumiColor.textSecondary)
    }

    private func preflightSummary(_ catalog: AutoloopCatalogState) -> String {
        let mapped = catalog.themes.reduce(0) { partial, bank in
            partial + SoundSwitchOutputProfileProjection.mappedCount(for: bank.id, catalog: catalog)
        }
        return "\(mapped) logical Autoloops · \(catalog.preflight.missingCellCount) incomplete · MIDI addresses unbound"
    }

    private func controllerUtility(_ title: String) -> some View {
        Text(title.uppercased())
            .font(LumiTypography.technical)
            .foregroundStyle(LumiColor.textSecondary)
            .padding(.horizontal, 8)
            .frame(height: 28)
            .background(LumiColor.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func inspectorHeading(_ title: String) -> some View {
        Text(title)
            .font(LumiTypography.technical.weight(.bold))
            .foregroundStyle(LumiColor.textSecondary)
    }

    private func inspectorValue(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
            Text(value)
                .font(LumiTypography.body.weight(.semibold))
                .lineLimit(1)
        }
        .padding(.horizontal, 9)
        .frame(maxWidth: .infinity, minHeight: 43, alignment: .leading)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func legend(_ status: SoundSwitchAutoloopSlotStatus, _ title: String) -> some View {
        Label {
            Text(title).font(LumiTypography.technical)
        } icon: {
            Circle().fill(slotStatusColor(status)).frame(width: 7, height: 7)
        }
        .foregroundStyle(LumiColor.textSecondary)
    }

    private func slotStatusColor(_ status: SoundSwitchAutoloopSlotStatus) -> Color {
        switch status {
        case .mapped: LumiColor.success
        case .incomplete: LumiColor.warning
        case .available: LumiColor.textSecondary
        }
    }

    private func slotStatusLabel(_ status: SoundSwitchAutoloopSlotStatus) -> String {
        switch status {
        case .mapped: "Mapped"
        case .incomplete: "Incomplete"
        case .available: "Available"
        }
    }

    private func slotLabel(_ slot: SoundSwitchAutoloopSlotState) -> String {
        guard let roleName = slot.roleName else { return "AVAILABLE" }
        let words = roleName
            .replacingOccurrences(of: "/", with: " ")
            .split(separator: " ")
        if words.count == 1 { return String(words[0]).uppercased() }
        return words.map { String($0.prefix(1)) }.joined().uppercased()
    }

    private func slotBinding(_ slot: SoundSwitchAutoloopSlotState?) -> String {
        guard let slot, let role = slot.roleName, let variant = slot.variantName else {
            return "Unmapped"
        }
        return "\(role) · \(variant)"
    }

    private func selectedBank(_ catalog: AutoloopCatalogState) -> AutoloopThemeState? {
        catalog.themes.first { $0.id == selectedBankID } ?? catalog.themes.first
    }

    private func projectedBank(_ catalog: AutoloopCatalogState) -> SoundSwitchOutputBankState? {
        SoundSwitchOutputProfileProjection.banks(catalog: catalog)
            .first { $0.id == selectedBankID }
            ?? SoundSwitchOutputProfileProjection.banks(catalog: catalog).first
    }

    private func slots(_ catalog: AutoloopCatalogState) -> [SoundSwitchAutoloopSlotState] {
        guard let bank = selectedBank(catalog) else { return [] }
        return SoundSwitchOutputProfileProjection.slots(for: bank.id, catalog: catalog)
    }

    private func selectedSlot(_ catalog: AutoloopCatalogState) -> SoundSwitchAutoloopSlotState? {
        slots(catalog).first { $0.number == selectedSlotNumber }
    }

    private func selectBank(_ bank: AutoloopThemeState, catalog: AutoloopCatalogState) {
        selectedBankID = bank.id
        selectedSlotNumber = 1
        activePage = 1
        bankNameDraft = bank.name
        refreshDrafts(catalog)
    }

    private func selectSlot(_ slot: SoundSwitchAutoloopSlotState, catalog: AutoloopCatalogState) {
        selectedSlotNumber = slot.number
        activePage = (slot.number - 1) / profile.slotsPerPage + 1
        refreshDrafts(catalog)
    }

    private func refreshDrafts(_ catalog: AutoloopCatalogState) {
        let slot = selectedSlot(catalog)
        variantNameDraft = slot?.variantName ?? ""
        entryNameDraft = slot?.entryName ?? ""
    }

    private func synchronize(_ catalog: AutoloopCatalogState) {
        if !catalog.themes.contains(where: { $0.id == selectedBankID }) {
            selectedBankID = catalog.themes.first?.id
        }
        let bank = selectedBank(catalog)
        bankNameDraft = bank?.name ?? ""
        if !slots(catalog).contains(where: { $0.number == selectedSlotNumber }) {
            selectedSlotNumber = 1
            activePage = 1
        }
        refreshDrafts(catalog)
    }

    private func copy(_ key: String) -> String {
        LibraryWorkspaceLocalization.value(key)
    }
}
