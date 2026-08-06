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
    private let localPlaybackFeedback: String?
    private let localPlaybackFeedbackIsError: Bool
    private let sourceImportFeedback: String?
    private let sourceImportFeedbackIsError: Bool
    private let onQuery: @MainActor (LibraryQueryRequest) -> Void
    private let onOpenEditor: @MainActor (UInt64) -> Void
    private let onTimelineEdit: @MainActor (TrackTimelineEditRequest) -> Void
    private let onTimelineHistory: @MainActor (TrackTimelineHistoryRequest) -> Void
    private let onSourceReconcile: @MainActor (TrackSourceReconcileRequest) -> Void
    private let onLoadOnLocalDeck: @MainActor (LibraryDeckLoadRequest) -> Void
    private let onPhraseRoleMutation: @Sendable (PhraseRoleMutationRequest) -> Void
    private let onRekordboxSyncPreview: @Sendable (RekordboxXMLSyncPreviewRequest) -> Void
    private let onRekordboxSyncApply: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void
    private let onRekordboxAnalysisImport: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void

    @Binding private var section: LibraryHubSection

    public init(
        state: LibraryWorkspaceState,
        keyNotation: Binding<KeyNotationPreference>,
        section: Binding<LibraryHubSection> = .constant(.tracks),
        phraseRoleFeedback: String? = nil,
        timelineFeedback: String? = nil,
        localPlaybackFeedback: String? = nil,
        localPlaybackFeedbackIsError: Bool = false,
        sourceImportFeedback: String? = nil,
        sourceImportFeedbackIsError: Bool = false,
        onQuery: @escaping @MainActor (LibraryQueryRequest) -> Void = { _ in },
        onOpenEditor: @escaping @MainActor (UInt64) -> Void = { _ in },
        onTimelineEdit: @escaping @MainActor (TrackTimelineEditRequest) -> Void = { _ in },
        onTimelineHistory: @escaping @MainActor (TrackTimelineHistoryRequest) -> Void = { _ in },
        onSourceReconcile: @escaping @MainActor (TrackSourceReconcileRequest) -> Void = { _ in },
        onLoadOnLocalDeck: @escaping @MainActor (LibraryDeckLoadRequest) -> Void = { _ in },
        onPhraseRoleMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in },
        onRekordboxSyncPreview: @escaping @Sendable (RekordboxXMLSyncPreviewRequest) -> Void = { _ in },
        onRekordboxSyncApply: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in },
        onRekordboxAnalysisImport: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in }
    ) {
        self.state = state
        _keyNotation = keyNotation
        _section = section
        self.phraseRoleFeedback = phraseRoleFeedback
        self.timelineFeedback = timelineFeedback
        self.localPlaybackFeedback = localPlaybackFeedback
        self.localPlaybackFeedbackIsError = localPlaybackFeedbackIsError
        self.sourceImportFeedback = sourceImportFeedback
        self.sourceImportFeedbackIsError = sourceImportFeedbackIsError
        self.onQuery = onQuery
        self.onOpenEditor = onOpenEditor
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        self.onSourceReconcile = onSourceReconcile
        self.onLoadOnLocalDeck = onLoadOnLocalDeck
        self.onPhraseRoleMutation = onPhraseRoleMutation
        self.onRekordboxSyncPreview = onRekordboxSyncPreview
        self.onRekordboxSyncApply = onRekordboxSyncApply
        self.onRekordboxAnalysisImport = onRekordboxAnalysisImport
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
                        onLoadOnLocalDeck: onLoadOnLocalDeck,
                        timelineFeedback: timelineFeedback,
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
                        onMutation: onPhraseRoleMutation,
                        onSyncPreview: onRekordboxSyncPreview,
                        onSyncApply: onRekordboxSyncApply,
                        onAnalysisImport: onRekordboxAnalysisImport
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
            sectionButton(.sources, "Sources & Import", "externaldrive.badge.plus")
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
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.section.\(value.rawValue)")
    }
}
