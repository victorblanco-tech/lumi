import LumiDesignSystem
import SwiftUI

struct TrackWorkflowSettingsView: View {
    let catalog: TrackWorkflowCatalog
    let feedback: String?
    let rendersInteractiveControls: Bool
    let onSave: @Sendable ([WorkflowStepDefinition]) -> Void

    @State private var drafts: [WorkflowStepDraft]
    @State private var selectedID: String?

    init(
        catalog: TrackWorkflowCatalog,
        feedback: String?,
        rendersInteractiveControls: Bool,
        onSave: @escaping @Sendable ([WorkflowStepDefinition]) -> Void
    ) {
        self.catalog = catalog
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onSave = onSave
        let values = catalog.steps.map(WorkflowStepDraft.init)
        _drafts = State(initialValue: values)
        _selectedID = State(initialValue: values.first?.id)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("Track Preparation Workflow").font(LumiTypography.screenTitle)
                    Text("Define ordered preparation buckets and their automatic eligibility rules. System queues for USB changes and new track versions remain fixed and always visible.")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Button {
                    addStep()
                } label: {
                    Label("Add Step", systemImage: "plus")
                }
                .disabled(!rendersInteractiveControls || drafts.count >= 12)
                Button("Save Workflow") { onSave(compiledSteps) }
                    .buttonStyle(.borderedProminent)
                    .disabled(!rendersInteractiveControls || !isValid || !hasChanges)
                    .accessibilityIdentifier("lumi.settings.workflow.save")
            }

            HStack(spacing: LumiSpacing.large) {
                stepList.frame(width: 310)
                inspector.frame(maxWidth: .infinity)
            }
            if let feedback {
                Label(feedback, systemImage: "checkmark.circle.fill")
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.success)
            }
        }
        .padding(LumiSpacing.xLarge)
        .onChange(of: catalog.revision) { _, _ in
            drafts = catalog.steps.map(WorkflowStepDraft.init)
            selectedID = drafts.first?.id
        }
        .accessibilityIdentifier("lumi.settings.workflow")
    }

    private var stepList: some View {
        LumiPanel {
            VStack(spacing: LumiSpacing.xSmall) {
                ForEach(Array(drafts.enumerated()), id: \.element.id) { index, draft in
                    HStack(spacing: LumiSpacing.xSmall) {
                        Button { selectedID = draft.id } label: {
                            HStack(spacing: LumiSpacing.small) {
                            Image(systemName: draft.icon)
                                .foregroundStyle(workflowSettingsColor(draft.colorRGB))
                            VStack(alignment: .leading, spacing: 2) {
                                Text(draft.displayName).font(LumiTypography.body.weight(.semibold))
                                Text(draft.id).font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            }
                            .padding(.horizontal, LumiSpacing.small)
                            .frame(maxWidth: .infinity, minHeight: 56)
                            .contentShape(Rectangle())
                            .background(selectedID == draft.id ? LumiColor.accent.opacity(0.14) : Color.clear)
                            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                        }
                        .buttonStyle(.plain)
                        VStack(spacing: 2) {
                            Button { move(index, by: -1) } label: { Image(systemName: "chevron.up") }
                                .disabled(index == 0)
                            Button { move(index, by: 1) } label: { Image(systemName: "chevron.down") }
                                .disabled(index + 1 == drafts.count)
                            }.buttonStyle(.plain)
                    }
                    .accessibilityIdentifier("lumi.settings.workflow.step.\(draft.id)")
                }
            }
        }
    }

    @ViewBuilder private var inspector: some View {
        if let index = drafts.firstIndex(where: { $0.id == selectedID }) {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    TextField("Step name", text: $drafts[index].displayName)
                        .textFieldStyle(.roundedBorder)
                    HStack {
                        Picker("Icon", selection: $drafts[index].icon) {
                            Label("Circle", systemImage: "circle").tag("circle")
                            Label("Progress", systemImage: "circle.lefthalf.filled").tag("circle.lefthalf.filled")
                            Label("Review", systemImage: "checklist").tag("checklist")
                            Label("Ready", systemImage: "checkmark.circle.fill").tag("checkmark.circle.fill")
                        }
                        Picker("Color", selection: $drafts[index].colorRGB) {
                            Text("Gray").tag(UInt32(0x8A949F))
                            Text("Blue").tag(UInt32(0x32B8F5))
                            Text("Orange").tag(UInt32(0xFF9F0A))
                            Text("Green").tag(UInt32(0x30D158))
                            Text("Purple").tag(UInt32(0xBF5AF2))
                        }
                    }
                    Divider()
                    Text("SMART ELIGIBILITY").font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Text("A track must already be assigned to this step and meet every enabled condition. This keeps steps mutually exclusive while still allowing automatic quality gates.")
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                    Toggle("Technical analysis is ready", isOn: $drafts[index].technicalReady)
                    Toggle("Audio is available for preview", isOn: $drafts[index].audioAvailable)
                    Toggle("Lumi phrases have been authored", isOn: $drafts[index].authoredTimeline)
                    Toggle("No unresolved USB change", isOn: $drafts[index].noUnresolvedUSBChange)
                    Toggle("A likely newer track version exists", isOn: $drafts[index].versionCandidate)
                    if !isRequired(drafts[index].id) {
                        Divider()
                        Button("Delete Step", role: .destructive) { remove(index) }
                    }
                }
            }
        } else {
            ContentUnavailableView("Select a workflow step", systemImage: "checklist")
        }
    }

    private var compiledSteps: [WorkflowStepDefinition] {
        drafts.enumerated().map { index, draft in draft.definition(sortOrder: UInt16(index + 1)) }
    }
    private var hasChanges: Bool { compiledSteps != catalog.steps }
    private var isValid: Bool {
        drafts.count >= 3 && Set(drafts.map(\.id)).count == drafts.count
            && drafts.allSatisfy { !$0.displayName.trimmingCharacters(in: .whitespaces).isEmpty }
    }
    private func isRequired(_ id: String) -> Bool {
        ["not-started", "in-progress", "ready-for-show"].contains(id)
    }
    private func addStep() {
        let id = "custom-" + UUID().uuidString.lowercased().prefix(8)
        drafts.append(WorkflowStepDraft(id: String(id), displayName: "Review", icon: "checklist", colorRGB: 0x32B8F5))
        selectedID = String(id)
    }
    private func remove(_ index: Int) {
        drafts.remove(at: index)
        selectedID = drafts.first?.id
    }
    private func move(_ index: Int, by delta: Int) {
        let target = index + delta
        guard drafts.indices.contains(target) else { return }
        drafts.swapAt(index, target)
    }
}

private struct WorkflowStepDraft: Identifiable {
    var id: String
    var displayName: String
    var icon: String
    var colorRGB: UInt32
    var archived = false
    var technicalReady = false
    var audioAvailable = false
    var authoredTimeline = false
    var noUnresolvedUSBChange = false
    var versionCandidate = false

    init(id: String, displayName: String, icon: String, colorRGB: UInt32) {
        self.id = id; self.displayName = displayName; self.icon = icon; self.colorRGB = colorRGB
    }
    init(_ step: WorkflowStepDefinition) {
        id = step.id; displayName = step.displayName; icon = step.icon
        colorRGB = step.colorRGB; archived = step.archived
        for rule in step.rules where rule.operator == .isEqual && rule.value == "true" {
            switch rule.field {
            case .technicalReady: technicalReady = true
            case .audioAvailable: audioAvailable = true
            case .authoredTimeline: authoredTimeline = true
            case .unresolvedUsbChange: noUnresolvedUSBChange = false
            case .versionCandidate: versionCandidate = true
            case .preparationStatus: break
            }
        }
        noUnresolvedUSBChange = step.rules.contains {
            $0.field == .unresolvedUsbChange && $0.operator == .isEqual && $0.value == "false"
        }
    }
    func definition(sortOrder: UInt16) -> WorkflowStepDefinition {
        var rules = [WorkflowRule(field: .preparationStatus, operator: .isEqual, value: id)]
        if technicalReady { rules.append(.init(field: .technicalReady, operator: .isEqual, value: "true")) }
        if audioAvailable { rules.append(.init(field: .audioAvailable, operator: .isEqual, value: "true")) }
        if authoredTimeline { rules.append(.init(field: .authoredTimeline, operator: .isEqual, value: "true")) }
        if noUnresolvedUSBChange { rules.append(.init(field: .unresolvedUsbChange, operator: .isEqual, value: "false")) }
        if versionCandidate { rules.append(.init(field: .versionCandidate, operator: .isEqual, value: "true")) }
        return WorkflowStepDefinition(id: id, displayName: displayName, icon: icon, colorRGB: colorRGB,
                                      sortOrder: sortOrder, archived: archived, rules: rules)
    }
}

private func workflowSettingsColor(_ rgb: UInt32) -> Color {
    Color(red: Double((rgb >> 16) & 0xFF) / 255,
          green: Double((rgb >> 8) & 0xFF) / 255,
          blue: Double(rgb & 0xFF) / 255)
}
