import Foundation
import LumiDesignSystem
import SwiftUI

public struct AutoloopCatalogSettingsView: View {
    private enum ProfileSection: String {
        case banks
        case controller
        case midi
    }

    private let catalog: AutoloopCatalogState?
    private let profile = SoundSwitchOutputProfileState.builtIn
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (AutoloopCatalogMutationRequest) -> Void

    @State private var section: ProfileSection = .banks
    @State private var selectedBankID: UInt64?
    @State private var selectedButtonNumber: UInt16 = 1
    @State private var bankNameDraft = ""
    @State private var autoloopNameDraft = ""
    @State private var phraseRoleDraft = ""

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
        let firstSlot = catalog.flatMap { value in
            firstBank.flatMap {
                SoundSwitchOutputProfileProjection.slots(for: $0.id, catalog: value).first
            }
        }
        _selectedBankID = State(initialValue: firstBank?.id)
        _bankNameDraft = State(initialValue: firstBank?.name ?? "")
        _autoloopNameDraft = State(initialValue: firstSlot?.entryName ?? "")
        _phraseRoleDraft = State(
            initialValue: firstSlot?.roleID
                ?? catalog?.roles.first(where: { !$0.archived })?.id
                ?? ""
        )
    }

    public var body: some View {
        Group {
            if let catalog {
                VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                    profileHeader(catalog)
                    sectionTabs
                    switch section {
                    case .banks: banksAndAutoloops(catalog)
                    case .controller: virtualController(catalog)
                    case .midi: midiPreparation(catalog)
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
                    Text(profile.name).font(LumiTypography.cardTitle)
                    Text("4 banks · 8 AutoLoops per bank · 32 mappings total")
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
                Text("\(totalMapped(catalog)) / 32 mapped")
                    .font(LumiTypography.technical.weight(.semibold))
            }
        }
    }

    private var sectionTabs: some View {
        HStack(spacing: 4) {
            profileTab(.banks, "Banks & AutoLoops")
            profileTab(.controller, "Test Controller")
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
                bankSurface(catalog).frame(maxWidth: .infinity)
                mappingInspector(catalog).frame(width: 310)
            }
        }
    }

    private func bankTabs(_ catalog: AutoloopCatalogState) -> some View {
        HStack(spacing: LumiSpacing.small) {
            ForEach(catalog.themes) { bank in
                let mapped = SoundSwitchOutputProfileProjection.mappedCount(
                    for: bank.id,
                    catalog: catalog
                )
                Button { selectBank(bank, catalog: catalog) } label: {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("BANK \(bank.sortOrder)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text(bank.name)
                            .font(LumiTypography.body.weight(.semibold))
                            .lineLimit(1)
                        Text("\(mapped) / 8 AutoLoops")
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
        return LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Bank \(bank?.sortOrder ?? 1) · \(bank?.name ?? "")")
                            .font(LumiTypography.cardTitle)
                        Text("The order below matches the eight SoundSwitch AutoLoop buttons.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Text("BUTTON → AUTOLOOP NAME → PHRASE TYPE")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                VStack(spacing: 7) {
                    ForEach(slots(catalog)) { slot in
                        Button { selectSlot(slot, catalog: catalog) } label: {
                            HStack(spacing: LumiSpacing.medium) {
                                Text("\(slot.number)")
                                    .font(LumiTypography.technical.weight(.bold))
                                    .frame(width: 28, height: 28)
                                    .background(LumiColor.accent.opacity(0.14))
                                    .clipShape(RoundedRectangle(cornerRadius: 5))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(slot.entryName ?? "Empty AutoLoop")
                                        .font(LumiTypography.body.weight(.semibold))
                                        .lineLimit(1)
                                    Text("AutoLoop Name")
                                        .font(LumiTypography.technical)
                                        .foregroundStyle(LumiColor.textSecondary)
                                }
                                Spacer()
                                Text(slot.roleName ?? "Choose Phrase Type")
                                    .font(LumiTypography.caption.weight(.semibold))
                                    .foregroundStyle(slot.status == .mapped ? LumiColor.accent : LumiColor.warning)
                                    .padding(.horizontal, 9)
                                    .frame(height: 27)
                                    .background(LumiColor.surface)
                                    .clipShape(Capsule())
                            }
                            .padding(.horizontal, 10)
                            .frame(height: 51)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(LumiColor.textPrimary)
                        .background(
                            selectedButtonNumber == slot.number
                                ? LumiColor.accent.opacity(0.14)
                                : LumiColor.surfaceElevated
                        )
                        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                        .overlay {
                            RoundedRectangle(cornerRadius: LumiRadius.control)
                                .stroke(
                                    selectedButtonNumber == slot.number
                                        ? LumiColor.accent
                                        : LumiColor.border
                                )
                        }
                        .accessibilityIdentifier("lumi.settings.outputProfiles.button.\(slot.number)")
                    }
                }
            }
        }
    }

    private func mappingInspector(_ catalog: AutoloopCatalogState) -> some View {
        let bank = selectedBank(catalog)
        let slot = selectedSlot(catalog)
        return LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                Text("Button Mapping").font(LumiTypography.cardTitle)
                Text("BANK")
                    .font(LumiTypography.technical.weight(.bold))
                    .foregroundStyle(LumiColor.textSecondary)
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
                    inspectorValue("Bank Name", bank?.name ?? "")
                }
                Divider()
                Text("BUTTON \(slot?.number ?? 1)")
                    .font(LumiTypography.technical.weight(.bold))
                    .foregroundStyle(LumiColor.textSecondary)
                if rendersInteractiveControls {
                    VStack(alignment: .leading, spacing: 5) {
                        Text("AutoLoop Name")
                            .font(LumiTypography.caption.weight(.semibold))
                        TextField("Exact name from SoundSwitch", text: $autoloopNameDraft)
                            .textFieldStyle(.roundedBorder)
                    }
                    VStack(alignment: .leading, spacing: 5) {
                        Text("Phrase Type")
                            .font(LumiTypography.caption.weight(.semibold))
                        Picker("Phrase Type", selection: $phraseRoleDraft) {
                            ForEach(catalog.roles.filter { !$0.archived }) { role in
                                Text(role.name).tag(role.id)
                            }
                        }
                        .labelsHidden()
                        .frame(maxWidth: .infinity)
                    }
                    Button(slot?.status == .mapped ? "Save Mapping" : "Create Mapping") {
                        guard let bank else { return }
                        let name = autoloopNameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !name.isEmpty, !phraseRoleDraft.isEmpty else { return }
                        onMutation(
                            .setButton(
                                themeID: bank.id,
                                buttonNumber: selectedButtonNumber,
                                roleID: phraseRoleDraft,
                                displayName: name
                            )
                        )
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(LumiColor.accent)
                    .disabled(!mappingChanged(slot))
                    if slot?.status == .mapped {
                        Button("Clear Mapping", role: .destructive) {
                            guard let bank else { return }
                            onMutation(
                                .setButton(
                                    themeID: bank.id,
                                    buttonNumber: selectedButtonNumber,
                                    roleID: phraseRoleDraft,
                                    displayName: nil
                                )
                            )
                        }
                    }
                } else {
                    inspectorValue("AutoLoop Name", autoloopNameDraft.isEmpty ? "Empty" : autoloopNameDraft)
                    inspectorValue("Phrase Type", roleName(phraseRoleDraft, catalog: catalog))
                }
                Divider()
                Text("The AutoLoop Name is copied exactly from SoundSwitch. Phrase Type is Lumi's functional mapping.")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                Spacer(minLength: 0)
            }
        }
    }

    private func virtualController(_ catalog: AutoloopCatalogState) -> some View {
        HStack(alignment: .top, spacing: 10) {
            ForEach(catalog.themes) { bank in
                let bankSlots = SoundSwitchOutputProfileProjection.slots(for: bank.id, catalog: catalog)
                LumiPanel {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("BANK \(bank.sortOrder)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text(bank.name)
                            .font(LumiTypography.body.weight(.semibold))
                            .lineLimit(1)
                        ForEach(bankSlots) { slot in
                            Button {
                                selectBankAndSlot(bank, slot: slot, catalog: catalog)
                            } label: {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(slot.entryName ?? "EMPTY AUTOLOOP")
                                        .font(LumiTypography.caption.weight(.semibold))
                                        .lineLimit(1)
                                    Text("\(slot.number) · \(slot.roleName ?? "Unmapped")")
                                        .font(LumiTypography.technical)
                                        .foregroundStyle(LumiColor.textSecondary)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 9)
                                .frame(height: 45)
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(LumiColor.textPrimary)
                            .background(LumiColor.surfaceElevated)
                            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                            .overlay {
                                RoundedRectangle(cornerRadius: LumiRadius.control)
                                    .stroke(LumiColor.border)
                            }
                        }
                    }
                }
            }
        }
        .overlay(alignment: .bottomTrailing) {
            Text("TEST SURFACE · MIDI OUTPUT ENABLED IN THE POC")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
                .padding(8)
        }
    }

    private func midiPreparation(_ catalog: AutoloopCatalogState) -> some View {
        HStack(alignment: .top, spacing: LumiSpacing.medium) {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    Text("MIDI Transport").font(LumiTypography.cardTitle)
                    inspectorValue("Output Device", "Lumi Virtual MIDI → SoundSwitch")
                    inspectorValue("Configured Surface", "4 banks · 8 AutoLoops")
                    inspectorValue("Timing", "Ableton Link → SoundSwitch")
                    inspectorValue("Bank Switch Delay", "Measure in POC")
                    Spacer(minLength: 0)
                }
            }
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    Text("POC Acceptance").font(LumiTypography.cardTitle)
                    pocRequirement("SoundSwitch discovers Lumi's virtual MIDI device")
                    pocRequirement("Configured Bank and AutoLoop buttons respond deterministically")
                    pocRequirement("Physical Control One remains usable in parallel")
                    pocRequirement("DMX output through Control One visibly drives fixtures")
                    pocRequirement("Disconnect and reconnect remain fail-silent")
                    Divider()
                    Text("SELECT BANK 1\nTRIGGER BUTTON 1")
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
                    Text("Enabled by the MIDI POC story.")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Spacer(minLength: 0)
                }
            }
        }
    }

    private func inspectorValue(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(LumiTypography.technical).foregroundStyle(LumiColor.textSecondary)
            Text(value).font(LumiTypography.body.weight(.semibold)).lineLimit(1)
        }
        .padding(.horizontal, 9)
        .frame(maxWidth: .infinity, minHeight: 43, alignment: .leading)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func pocRequirement(_ text: String) -> some View {
        Label(text, systemImage: "circle.dashed")
            .font(LumiTypography.body)
            .foregroundStyle(LumiColor.textSecondary)
    }

    private func slots(_ catalog: AutoloopCatalogState) -> [SoundSwitchAutoloopSlotState] {
        guard let bank = selectedBank(catalog) else { return [] }
        return SoundSwitchOutputProfileProjection.slots(for: bank.id, catalog: catalog)
    }

    private func selectedBank(_ catalog: AutoloopCatalogState) -> AutoloopThemeState? {
        catalog.themes.first { $0.id == selectedBankID } ?? catalog.themes.first
    }

    private func selectedSlot(_ catalog: AutoloopCatalogState) -> SoundSwitchAutoloopSlotState? {
        slots(catalog).first { $0.number == selectedButtonNumber }
    }

    private func totalMapped(_ catalog: AutoloopCatalogState) -> Int {
        catalog.themes.reduce(0) {
            $0 + SoundSwitchOutputProfileProjection.mappedCount(for: $1.id, catalog: catalog)
        }
    }

    private func mappingChanged(_ slot: SoundSwitchAutoloopSlotState?) -> Bool {
        let name = autoloopNameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        return !name.isEmpty
            && !phraseRoleDraft.isEmpty
            && (name != slot?.entryName || phraseRoleDraft != slot?.roleID)
    }

    private func roleName(_ id: String, catalog: AutoloopCatalogState) -> String {
        catalog.roles.first { $0.id == id }?.name ?? "Choose Phrase Type"
    }

    private func selectBank(_ bank: AutoloopThemeState, catalog: AutoloopCatalogState) {
        selectedBankID = bank.id
        selectedButtonNumber = 1
        bankNameDraft = bank.name
        refreshDrafts(catalog)
    }

    private func selectSlot(_ slot: SoundSwitchAutoloopSlotState, catalog: AutoloopCatalogState) {
        selectedButtonNumber = slot.number
        refreshDrafts(catalog)
    }

    private func selectBankAndSlot(
        _ bank: AutoloopThemeState,
        slot: SoundSwitchAutoloopSlotState,
        catalog: AutoloopCatalogState
    ) {
        selectedBankID = bank.id
        selectedButtonNumber = slot.number
        bankNameDraft = bank.name
        refreshDrafts(catalog)
    }

    private func refreshDrafts(_ catalog: AutoloopCatalogState) {
        let slot = selectedSlot(catalog)
        autoloopNameDraft = slot?.entryName ?? ""
        phraseRoleDraft = slot?.roleID
            ?? catalog.roles.first(where: { !$0.archived })?.id
            ?? ""
    }

    private func synchronize(_ catalog: AutoloopCatalogState) {
        if !catalog.themes.contains(where: { $0.id == selectedBankID }) {
            selectedBankID = catalog.themes.first?.id
        }
        bankNameDraft = selectedBank(catalog)?.name ?? ""
        if !(1...profile.slotsPerBank).contains(selectedButtonNumber) {
            selectedButtonNumber = 1
        }
        refreshDrafts(catalog)
    }

    private func copy(_ key: String) -> String {
        LibraryWorkspaceLocalization.value(key)
    }
}
