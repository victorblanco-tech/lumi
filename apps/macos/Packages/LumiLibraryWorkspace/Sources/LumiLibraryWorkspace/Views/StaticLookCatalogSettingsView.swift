import LumiDesignSystem
import SwiftUI

public struct StaticLookCatalogSettingsView: View {
    private let profile = SoundSwitchStaticLookProfileState.builtIn
    private let policy: LightPlanningPolicyState
    private let midiIntegration: MidiIntegrationState?
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onSave: @Sendable (LightPlanningPolicyState) -> Void
    private let onSendLearnPulse: @Sendable (UInt16) -> Void
    private let onToggleStaticLook: @Sendable (UInt16) -> Void

    @State private var draft: LightPlanningPolicyState
    @State private var selectedSlotNumber: UInt16 = 1
    @State private var displayNameDraft = ""

    public init(
        policy: LightPlanningPolicyState,
        midiIntegration: MidiIntegrationState?,
        feedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        onSave: @escaping @Sendable (LightPlanningPolicyState) -> Void = { _ in },
        onSendLearnPulse: @escaping @Sendable (UInt16) -> Void = { _ in },
        onToggleStaticLook: @escaping @Sendable (UInt16) -> Void = { _ in }
    ) {
        self.policy = policy
        self.midiIntegration = midiIntegration
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onSave = onSave
        self.onSendLearnPulse = onSendLearnPulse
        self.onToggleStaticLook = onToggleStaticLook
        _draft = State(initialValue: policy)
        let first = SoundSwitchStaticLookProjection.slots(policy: policy).first
        _displayNameDraft = State(initialValue: first?.displayName ?? "")
    }

    public var body: some View {
        HStack(alignment: .top, spacing: LumiSpacing.medium) {
            mappingSurface.frame(maxWidth: .infinity)
            inspector.frame(width: 310)
        }
        .onChange(of: policy) { _, value in
            draft = value
            refreshName()
        }
        .accessibilityIdentifier("lumi.settings.outputProfiles.staticLooks")
    }

    private var mappingSurface: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text("SoundSwitch Static Looks")
                        .font(LumiTypography.cardTitle)
                    Text("One global 32-slot surface. Static Looks are exclusive: SoundSwitch can activate one at a time.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Label(
                    midiIntegration?.isReady == true ? "SOURCE PUBLISHED" : "PUBLISH SOURCE FIRST",
                    systemImage: midiIntegration?.isReady == true ? "checkmark.circle.fill" : "circle.dashed"
                )
                .font(LumiTypography.technical.weight(.bold))
                .foregroundStyle(midiIntegration?.isReady == true ? LumiColor.success : LumiColor.warning)
            }
            Label(
                "AUTOMATION MODE: SoundSwitch does not report its active Static Look. While Lumi is in Start, change automated Static Looks through Lumi only; Control One AutoLoops remain fully parallel.",
                systemImage: "exclamationmark.triangle.fill"
            )
            .font(LumiTypography.caption)
            .foregroundStyle(LumiColor.warning)
            .help("Lumi tracks only Static Look pulses it successfully sent. A direct Static Look change on Control One cannot be observed and can make a later toggle ambiguous.")
            guidedLearnCard
            LumiPanel { staticLookGrid }
        }
    }

    private var guidedLearnCard: some View {
        let slot = selectedSlot
        return HStack(spacing: LumiSpacing.medium) {
            Image(systemName: "arrow.right.circle.fill")
                .font(.system(size: 22))
                .foregroundStyle(LumiColor.accent)
            VStack(alignment: .leading, spacing: 3) {
                Text("GUIDED MIDI LEARN")
                    .font(LumiTypography.technical.weight(.bold))
                Text("In SoundSwitch, arm Map for this Static Look. Send the pulse and Lumi advances to the next slot.")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                Text("READY: STATIC LOOK \(selectedSlotNumber) · CHANNEL \(profile.midiChannel) · NOTE \(slot?.midiNote ?? 0)")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.accent)
            }
            Spacer()
            Button("Send Learn & Next") {
                onSendLearnPulse(selectedSlotNumber)
                advanceLearnTarget()
            }
            .buttonStyle(.borderedProminent)
            .tint(LumiColor.accent)
            .disabled(!rendersInteractiveControls || midiIntegration?.isReady != true)
            .accessibilityIdentifier("lumi.settings.outputProfiles.staticLooks.guidedLearn.next")
        }
        .padding(LumiSpacing.medium)
        .background(LumiColor.accent.opacity(0.10))
        .overlay { RoundedRectangle(cornerRadius: LumiRadius.control).stroke(LumiColor.accent) }
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private var staticLookGrid: some View {
        let columns = Array(repeating: GridItem(.flexible(minimum: 130), spacing: 7), count: 4)
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("STATIC LOOK ADDRESSES")
                    .font(LumiTypography.technical.weight(.bold))
                Spacer()
                Text("CHANNEL \(profile.midiChannel) · NOTES 64–95")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            LazyVGrid(columns: columns, spacing: 7) {
                ForEach(gridSlots) { slot in
                    slotButton(slot)
                }
            }
        }
    }

    private func slotButton(_ slot: SoundSwitchStaticLookSlotState) -> some View {
        let selected = selectedSlotNumber == slot.number
        return Button {
            select(slot)
        } label: {
            VStack(alignment: .leading, spacing: 5) {
                HStack {
                    Text("STATIC LOOK \(slot.number)")
                        .font(LumiTypography.caption.weight(.semibold))
                    Spacer()
                    Circle()
                        .fill(statusColor(slot.status))
                        .frame(width: 7, height: 7)
                }
                Text(slot.displayName.flatMap { $0.isEmpty ? nil : $0 } ?? "Available")
                    .font(LumiTypography.body.weight(.semibold))
                    .lineLimit(1)
                Text("Ch \(slot.midiChannel) · Note \(slot.midiNote)")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .frame(maxWidth: .infinity, minHeight: 64, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 9)
        .padding(.vertical, 7)
        .background(selected ? LumiColor.accent.opacity(0.15) : LumiColor.surfaceElevated)
        .overlay {
            Rectangle().stroke(selected ? LumiColor.accent : LumiColor.border, lineWidth: selected ? 2 : 1)
        }
        .accessibilityIdentifier("lumi.settings.outputProfiles.staticLooks.slot.\(slot.number)")
    }

    private var inspector: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                Text("Static Look \(selectedSlotNumber)")
                    .font(LumiTypography.cardTitle)
                if let slot = selectedSlot {
                    inspectorValue("MIDI Address", "Channel \(slot.midiChannel) · Note \(slot.midiNote)")
                    TextField("Static Look Name", text: $displayNameDraft)
                        .textFieldStyle(.roundedBorder)
                        .disabled(!rendersInteractiveControls)
                    Button(slot.modifierID == nil ? "Create Mapping" : "Save Mapping") {
                        saveSelectedSlot(slot)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(LumiColor.accent)
                    .disabled(!rendersInteractiveControls || displayNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    Divider()
                    Text("MIDI Verification")
                        .font(LumiTypography.body.weight(.semibold))
                    Text("SoundSwitch does not return its selected Static Look. Toggle it, observe the result, then confirm both directions manually.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                    HStack {
                        Button("Learn") { onSendLearnPulse(slot.number) }
                        Button("Toggle in SoundSwitch") { onToggleStaticLook(slot.number) }
                    }
                    .disabled(!rendersInteractiveControls || midiIntegration?.isReady != true)
                    Toggle("Activation verified", isOn: verificationBinding(\.activationVerified, slot: slot))
                    Toggle("Release verified", isOn: verificationBinding(\.releaseVerified, slot: slot))
                    if slot.modifierID != nil {
                        Button("Save Verification") { saveVerification(slot) }
                            .disabled(!rendersInteractiveControls)
                    }
                    Label(
                        slot.status == .verified ? "READY FOR AUTOMATION" : "AUTOMATION LOCKED",
                        systemImage: slot.status == .verified ? "checkmark.circle.fill" : "lock.circle"
                    )
                    .font(LumiTypography.technical.weight(.bold))
                    .foregroundStyle(slot.status == .verified ? LumiColor.success : LumiColor.warning)
                    if slot.modifierID != nil {
                        Button("Clear Mapping", role: .destructive) { clearSelectedSlot(slot) }
                            .disabled(!rendersInteractiveControls)
                    }
                    if let feedback {
                        Text(feedback)
                            .font(LumiTypography.caption)
                            .foregroundStyle(feedback.lowercased().contains("could not") ? LumiColor.warning : LumiColor.success)
                    }
                }
                Spacer(minLength: 0)
            }
        }
    }

    private var slots: [SoundSwitchStaticLookSlotState] {
        SoundSwitchStaticLookProjection.slots(policy: draft)
    }

    private var gridSlots: [SoundSwitchStaticLookSlotState] {
        SoundSwitchStaticLookProjection.controllerGridSlots(policy: draft)
    }

    private var selectedSlot: SoundSwitchStaticLookSlotState? {
        slots.first { $0.number == selectedSlotNumber }
    }

    private func select(_ slot: SoundSwitchStaticLookSlotState) {
        selectedSlotNumber = slot.number
        displayNameDraft = slot.displayName ?? ""
    }

    private func advanceLearnTarget() {
        guard selectedSlotNumber < profile.slotCount else { return }
        selectedSlotNumber += 1
        refreshName()
    }

    private func refreshName() {
        displayNameDraft = selectedSlot?.displayName ?? ""
    }

    private func saveSelectedSlot(_ slot: SoundSwitchStaticLookSlotState) {
        let name = displayNameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return }
        var updated = draft
        if let modifierID = slot.modifierID,
           let index = updated.modifiers.firstIndex(where: { $0.id == modifierID }) {
            updated.modifiers[index].displayName = name
            updated.modifiers[index].enabled = true
        } else {
            updated.modifiers.append(LightPlanOutputModifier(
                id: "soundswitch-static-look-\(slot.number)",
                providerKind: "soundswitch",
                kind: .atmosphere,
                displayName: name,
                enabled: true,
                midiChannel: slot.midiChannel,
                midiNote: slot.midiNote,
                activationVerified: false,
                releaseVerified: false
            ))
        }
        draft = updated
        onSave(updated)
    }

    private func saveVerification(_ slot: SoundSwitchStaticLookSlotState) {
        guard let modifierID = slot.modifierID else { return }
        onSave(draft)
        if let modifier = draft.modifiers.first(where: { $0.id == modifierID }) {
            displayNameDraft = modifier.displayName
        }
    }

    private func clearSelectedSlot(_ slot: SoundSwitchStaticLookSlotState) {
        guard let modifierID = slot.modifierID else { return }
        var updated = draft
        updated.modifiers.removeAll { $0.id == modifierID }
        updated.modifierRules.removeAll { $0.modifierID == modifierID }
        draft = updated
        displayNameDraft = ""
        onSave(updated)
    }

    private func verificationBinding(
        _ keyPath: WritableKeyPath<LightPlanOutputModifier, Bool>,
        slot: SoundSwitchStaticLookSlotState
    ) -> Binding<Bool> {
        Binding {
            guard let modifierID = slot.modifierID,
                  let modifier = draft.modifiers.first(where: { $0.id == modifierID }) else { return false }
            return modifier[keyPath: keyPath]
        } set: { value in
            guard let modifierID = slot.modifierID,
                  let index = draft.modifiers.firstIndex(where: { $0.id == modifierID }) else { return }
            draft.modifiers[index][keyPath: keyPath] = value
        }
    }

    private func statusColor(_ status: SoundSwitchStaticLookSlotStatus) -> Color {
        switch status {
        case .available: LumiColor.textSecondary
        case .mapped: LumiColor.warning
        case .verified: LumiColor.success
        }
    }

    private func inspectorValue(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(LumiTypography.technical).foregroundStyle(LumiColor.textSecondary)
            Text(value).font(LumiTypography.body.weight(.semibold))
        }
        .padding(.horizontal, 9)
        .frame(maxWidth: .infinity, minHeight: 43, alignment: .leading)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }
}
