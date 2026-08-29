import LumiDesignSystem
import SwiftUI

public enum LibraryHubSection: String, CaseIterable, Identifiable, Sendable {
    case tracks
    case sources

    public var id: String { rawValue }
}

public struct LibraryHubView: View {
    private let state: LibraryWorkspaceState
    @Binding private var keyNotation: KeyNotationPreference
    private let phraseRoleFeedback: String?
    private let timelineFeedback: String?
    private let trackWorkflowFeedback: String?
    private let localPlaybackFeedback: String?
    private let localPlaybackFeedbackIsError: Bool
    private let sourceImportFeedback: String?
    private let sourceImportFeedbackIsError: Bool
    private let usbSourceOperation: USBSourceOperationState
    private let onQuery: @MainActor (LibraryQueryRequest) -> Void
    private let onOpenEditor: @MainActor (UInt64) -> Void
    private let onTimelineEdit: @MainActor (TrackTimelineEditRequest) -> Void
    private let onTimelineHistory: @MainActor (TrackTimelineHistoryRequest) -> Void
    private let onSourceReconcile: @MainActor (TrackSourceReconcileRequest) -> Void
    private let onReuseTimeline: @MainActor (CreativeTimelineReuseRequest) -> Void
    private let onTrackWorkflowMutation: @MainActor (TrackWorkflowMutationRequest) -> Void
    private let onLoadOnLocalDeck: @MainActor (LibraryDeckLoadRequest) -> Void
    private let onPhraseRoleMutation: @Sendable (PhraseRoleMutationRequest) -> Void
    private let onRekordboxSyncPreview: @Sendable (RekordboxXMLSyncPreviewRequest) -> Void
    private let onRekordboxSyncApply: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void
    private let onRekordboxAnalysisImport: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void
    private let onRekordboxDeviceInspect: @Sendable (String, String?) -> Void
    private let onRekordboxDeviceSync: @Sendable (String, String?, [UInt32]) -> Void
    private let onRekordboxDeviceConflictResolution: @Sendable (USBConflictResolutionRequest) -> Void

    @Binding private var section: LibraryHubSection

    public init(
        state: LibraryWorkspaceState,
        keyNotation: Binding<KeyNotationPreference>,
        section: Binding<LibraryHubSection> = .constant(.tracks),
        phraseRoleFeedback: String? = nil,
        timelineFeedback: String? = nil,
        trackWorkflowFeedback: String? = nil,
        localPlaybackFeedback: String? = nil,
        localPlaybackFeedbackIsError: Bool = false,
        sourceImportFeedback: String? = nil,
        sourceImportFeedbackIsError: Bool = false,
        usbSourceOperation: USBSourceOperationState = .idle,
        onQuery: @escaping @MainActor (LibraryQueryRequest) -> Void = { _ in },
        onOpenEditor: @escaping @MainActor (UInt64) -> Void = { _ in },
        onTimelineEdit: @escaping @MainActor (TrackTimelineEditRequest) -> Void = { _ in },
        onTimelineHistory: @escaping @MainActor (TrackTimelineHistoryRequest) -> Void = { _ in },
        onSourceReconcile: @escaping @MainActor (TrackSourceReconcileRequest) -> Void = { _ in },
        onReuseTimeline: @escaping @MainActor (CreativeTimelineReuseRequest) -> Void = { _ in },
        onTrackWorkflowMutation: @escaping @MainActor (TrackWorkflowMutationRequest) -> Void = { _ in },
        onLoadOnLocalDeck: @escaping @MainActor (LibraryDeckLoadRequest) -> Void = { _ in },
        onPhraseRoleMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in },
        onRekordboxSyncPreview: @escaping @Sendable (RekordboxXMLSyncPreviewRequest) -> Void = { _ in },
        onRekordboxSyncApply: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in },
        onRekordboxAnalysisImport: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in },
        onRekordboxDeviceInspect: @escaping @Sendable (String, String?) -> Void = { _, _ in },
        onRekordboxDeviceSync: @escaping @Sendable (String, String?, [UInt32]) -> Void = { _, _, _ in },
        onRekordboxDeviceConflictResolution: @escaping @Sendable (USBConflictResolutionRequest) -> Void = { _ in }
    ) {
        self.state = state
        _keyNotation = keyNotation
        _section = section
        self.phraseRoleFeedback = phraseRoleFeedback
        self.timelineFeedback = timelineFeedback
        self.trackWorkflowFeedback = trackWorkflowFeedback
        self.localPlaybackFeedback = localPlaybackFeedback
        self.localPlaybackFeedbackIsError = localPlaybackFeedbackIsError
        self.sourceImportFeedback = sourceImportFeedback
        self.sourceImportFeedbackIsError = sourceImportFeedbackIsError
        self.usbSourceOperation = usbSourceOperation
        self.onQuery = onQuery
        self.onOpenEditor = onOpenEditor
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        self.onSourceReconcile = onSourceReconcile
        self.onReuseTimeline = onReuseTimeline
        self.onTrackWorkflowMutation = onTrackWorkflowMutation
        self.onLoadOnLocalDeck = onLoadOnLocalDeck
        self.onPhraseRoleMutation = onPhraseRoleMutation
        self.onRekordboxSyncPreview = onRekordboxSyncPreview
        self.onRekordboxSyncApply = onRekordboxSyncApply
        self.onRekordboxAnalysisImport = onRekordboxAnalysisImport
        self.onRekordboxDeviceInspect = onRekordboxDeviceInspect
        self.onRekordboxDeviceSync = onRekordboxDeviceSync
        self.onRekordboxDeviceConflictResolution = onRekordboxDeviceConflictResolution
    }

    public var body: some View {
        HStack(spacing: 0) {
            sectionNavigation
            Divider()
            Group {
                switch section {
                case .tracks:
                    LibraryWorkspaceView(
                        state: state,
                        keyNotation: $keyNotation,
                        onQuery: onQuery,
                        onOpenEditor: onOpenEditor,
                        onTimelineEdit: onTimelineEdit,
                        onTimelineHistory: onTimelineHistory,
                        onSourceReconcile: onSourceReconcile,
                        onReuseTimeline: onReuseTimeline,
                        onTrackWorkflowMutation: onTrackWorkflowMutation,
                        onLoadOnLocalDeck: onLoadOnLocalDeck,
                        timelineFeedback: timelineFeedback,
                        trackWorkflowFeedback: trackWorkflowFeedback,
                        localPlaybackFeedback: localPlaybackFeedback,
                        localPlaybackFeedbackIsError: localPlaybackFeedbackIsError
                    )
                case .sources:
                    LibrarySourcesWorkspaceView(
                        library: state,
                        settings: state.phraseRoleSettings,
                        feedback: phraseRoleFeedback,
                        syncFeedback: sourceImportFeedback,
                        syncFeedbackIsError: sourceImportFeedbackIsError,
                        usbOperation: usbSourceOperation,
                        onMutation: onPhraseRoleMutation,
                        onSyncPreview: onRekordboxSyncPreview,
                        onSyncApply: onRekordboxSyncApply,
                        onAnalysisImport: onRekordboxAnalysisImport,
                        onDeviceInspect: onRekordboxDeviceInspect,
                        onDeviceSync: onRekordboxDeviceSync,
                        onDeviceConflictResolution: onRekordboxDeviceConflictResolution
                    )
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.library.hub")
    }

    private var sectionNavigation: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            Text("LIBRARY")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
                .padding(.horizontal, LumiSpacing.small)
                .padding(.bottom, LumiSpacing.small)
            sectionButton(.tracks, "Tracks", "music.note.list")
            sectionButton(.sources, "Import & Sources", "externaldrive.badge.plus")
            Spacer()
        }
        .padding(LumiSpacing.large)
        .frame(width: 184)
        .background(LumiColor.surface)
    }

    private func sectionButton(
        _ value: LibraryHubSection,
        _ title: String,
        _ systemImage: String
    ) -> some View {
        Button { section = value } label: {
            Label(title, systemImage: systemImage)
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: LumiControlMetric.standardHeight)
                .padding(.horizontal, LumiSpacing.small)
                .foregroundStyle(section == value ? LumiColor.accent : LumiColor.textPrimary)
                .background(section == value ? LumiColor.accent.opacity(0.14) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.section.\(value.rawValue)")
    }
}
