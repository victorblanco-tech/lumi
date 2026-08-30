import Foundation
import LumiDesignSystem
import SwiftUI

public struct LibraryQueryRequest: Equatable, Sendable {
    public let search: String
    public let playlistID: UInt64?
    public let offset: UInt32
    public let limit: UInt16
    public let sortBy: LibraryTrackSortField
    public let sortDirection: LibraryTrackSortDirection
    public let workflowFilter: TrackWorkflowFilter?
    public let workflowStepID: String?

    public init(
        search: String,
        playlistID: UInt64?,
        offset: UInt32,
        limit: UInt16 = 50,
        sortBy: LibraryTrackSortField = .playlist,
        sortDirection: LibraryTrackSortDirection = .ascending,
        workflowFilter: TrackWorkflowFilter? = nil,
        workflowStepID: String? = nil
    ) {
        self.search = search
        self.playlistID = playlistID
        self.offset = offset
        self.limit = limit
        self.sortBy = sortBy
        self.sortDirection = sortDirection
        self.workflowFilter = workflowFilter
        self.workflowStepID = workflowStepID
    }
}

public enum TrackWorkflowMutationRequest: Equatable, Sendable {
    case setPreparationStatus(
        trackID: UInt64,
        expectedRevision: UInt64,
        status: TrackPreparationStatus
    )
    case resolveAttention(trackID: UInt64, expectedRevision: UInt64)
    case assignStep(trackID: UInt64, expectedRevision: UInt64, stepID: String)
    case setPhraseProtection(trackID: UInt64, expectedRevision: UInt64, locked: Bool)
    case replaceCatalog(expectedRevision: UInt64, steps: [WorkflowStepDefinition])
    case keepVersionSeparate(sourceTrackID: UInt64, targetTrackID: UInt64, expectedTargetRevision: UInt64)
}

private enum LibraryBrowserMode: String, CaseIterable, Identifiable {
    case playlists = "Playlists"
    case workflow = "Workflow"

    var id: String { rawValue }
}

public struct LibraryDeckLoadRequest: Equatable, Sendable {
    public let trackID: UInt64
    public let deckID: UInt64
    public let expectedTimelineRevision: UInt64

    public init(trackID: UInt64, deckID: UInt64, expectedTimelineRevision: UInt64) {
        self.trackID = trackID
        self.deckID = deckID
        self.expectedTimelineRevision = expectedTimelineRevision
    }
}

public struct LibraryWorkspaceView: View {
    private let state: LibraryWorkspaceState
    private let onQuery: @MainActor (LibraryQueryRequest) -> Void
    private let onOpenEditor: @MainActor (UInt64) -> Void
    private let onTimelineEdit: @MainActor (TrackTimelineEditRequest) -> Void
    private let onTimelineHistory: @MainActor (TrackTimelineHistoryRequest) -> Void
    private let onSourceReconcile: @MainActor (TrackSourceReconcileRequest) -> Void
    private let onReuseTimeline: @MainActor (CreativeTimelineReuseRequest) -> Void
    private let onTrackWorkflowMutation: @MainActor (TrackWorkflowMutationRequest) -> Void
    private let onLoadOnLocalDeck: @MainActor (LibraryDeckLoadRequest) -> Void
    private let timelineFeedback: String?
    private let trackWorkflowFeedback: String?
    private let localPlaybackFeedback: String?
    private let localPlaybackFeedbackIsError: Bool
    private let rendersInteractiveControls: Bool
    @Binding private var keyNotation: KeyNotationPreference
    @State private var search: String
    @State private var selectedTrackID: UInt64?
    @State private var selectedPlaylistID: UInt64?
    @State private var readinessFilter: LibraryReadinessFilter = .all
    @State private var browserMode: LibraryBrowserMode
    @State private var selectedWorkflowFilter: TrackWorkflowFilter?
    @State private var selectedWorkflowStepID: String?
    @State private var sortBy: LibraryTrackSortField
    @State private var sortDirection: LibraryTrackSortDirection
    @State private var tableSortOrder: [KeyPathComparator<LibraryTrack>] = []
    @State private var editorAnalysis: TrackEditorAnalysis?
    @State private var requestedEditorTrackID: UInt64?
    @FocusState private var isSearchFocused: Bool
    @AppStorage(LumiPreferenceKey.libraryTableColumns)
    private var trackTableCustomization = TableColumnCustomization<LibraryTrack>()

    public init(
        state: LibraryWorkspaceState,
        keyNotation: Binding<KeyNotationPreference>,
        rendersInteractiveControls: Bool = true,
        onQuery: @escaping @MainActor (LibraryQueryRequest) -> Void = { _ in },
        onOpenEditor: @escaping @MainActor (UInt64) -> Void = { _ in },
        onTimelineEdit: @escaping @MainActor (TrackTimelineEditRequest) -> Void = { _ in },
        onTimelineHistory: @escaping @MainActor (TrackTimelineHistoryRequest) -> Void = { _ in },
        onSourceReconcile: @escaping @MainActor (TrackSourceReconcileRequest) -> Void = { _ in },
        onReuseTimeline: @escaping @MainActor (CreativeTimelineReuseRequest) -> Void = { _ in },
        onTrackWorkflowMutation: @escaping @MainActor (TrackWorkflowMutationRequest) -> Void = { _ in },
        onLoadOnLocalDeck: @escaping @MainActor (LibraryDeckLoadRequest) -> Void = { _ in },
        timelineFeedback: String? = nil,
        trackWorkflowFeedback: String? = nil,
        localPlaybackFeedback: String? = nil,
        localPlaybackFeedbackIsError: Bool = false
    ) {
        self.state = state
        self.onQuery = onQuery
        self.onOpenEditor = onOpenEditor
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        self.onSourceReconcile = onSourceReconcile
        self.onReuseTimeline = onReuseTimeline
        self.onTrackWorkflowMutation = onTrackWorkflowMutation
        self.onLoadOnLocalDeck = onLoadOnLocalDeck
        self.timelineFeedback = timelineFeedback
        self.trackWorkflowFeedback = trackWorkflowFeedback
        self.localPlaybackFeedback = localPlaybackFeedback
        self.localPlaybackFeedbackIsError = localPlaybackFeedbackIsError
        self.rendersInteractiveControls = rendersInteractiveControls
        _keyNotation = keyNotation
        _search = State(initialValue: state.query.search)
        _selectedTrackID = State(initialValue: state.page.tracks.first?.id)
        _selectedPlaylistID = State(initialValue: state.query.playlistID)
        _browserMode = State(
            initialValue: state.query.workflowFilter == nil && state.query.workflowStepID == nil
                ? .playlists : .workflow
        )
        _selectedWorkflowFilter = State(initialValue: state.query.workflowFilter)
        _selectedWorkflowStepID = State(initialValue: state.query.workflowStepID)
        _sortBy = State(initialValue: state.query.sortBy)
        _sortDirection = State(initialValue: state.query.sortDirection)
        _editorAnalysis = State(initialValue: state.editor)
    }

    public var body: some View {
        VStack(spacing: 0) {
            VSplitView {
                Group {
                    if let analysis = editorAnalysis {
                        TrackLightingEditorView(
                            analysis: analysis,
                            autoloopCatalog: state.autoloopCatalog,
                            phraseColorPalette: state.phraseRoleSettings?.colorPalette ?? .defaults,
                            workflowCatalog: state.workflowCatalog,
                            keyNotation: keyNotation,
                            feedback: timelineFeedback,
                            isEmbedded: true,
                            onTimelineEdit: onTimelineEdit,
                            onTimelineHistory: onTimelineHistory,
                            onSourceReconcile: onSourceReconcile,
                            onReuseTimeline: onReuseTimeline,
                            onTrackWorkflowMutation: onTrackWorkflowMutation,
                            workflowFeedback: trackWorkflowFeedback
                        )
                        .id(analysis.track.id)
                    } else {
                        editorPlaceholder
                    }
                }
                .frame(minHeight: 620, idealHeight: 680)
                .clipped()

                libraryBrowser
                    .frame(minHeight: 130, idealHeight: 280)
            }
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.library.workspace")
        .task { requestInitialEditorIfNeeded() }
        .onChange(of: state.query.search) { _, value in search = value }
        .onChange(of: search) { _, value in
            guard rendersInteractiveControls,
                  value != state.query.search else { return }
            // The engine coalesces rapid requests by generation. Sending each
            // edit immediately avoids depending on a view task restart while
            // the native search field owns keyboard focus.
            submitQuery(search: value, offset: 0)
        }
        .onChange(of: state.query.playlistID) { _, value in selectedPlaylistID = value }
        .onChange(of: state.query.workflowFilter) { _, value in
            selectedWorkflowFilter = value
            if value != nil { browserMode = .workflow }
        }
        .onChange(of: state.query.workflowStepID) { _, value in
            selectedWorkflowStepID = value
            if value != nil { browserMode = .workflow }
        }
        .onChange(of: state.query.sortBy) { _, value in sortBy = value }
        .onChange(of: state.query.sortDirection) { _, value in sortDirection = value }
        .onChange(of: tableSortOrder) { _, value in applyTableSort(value) }
        .onChange(of: state.page.tracks) { _, tracks in
            if !tracks.contains(where: { $0.id == selectedTrackID }) {
                selectedTrackID = tracks.first?.id
            }
            requestInitialEditorIfNeeded()
        }
        .onChange(of: state.editor) { _, editor in
            editorAnalysis = editor
            if let editor {
                requestedEditorTrackID = nil
                selectedTrackID = editor.track.id
            }
        }
    }

    private var libraryBrowser: some View {
        HStack(spacing: 0) {
            collectionNavigation
                .frame(minWidth: 180, idealWidth: 210, maxWidth: 250)
            Divider()
            trackBrowser
                .frame(minWidth: 480, maxWidth: .infinity)
        }
    }

    private var editorPlaceholder: some View {
        VStack(spacing: LumiSpacing.medium) {
            Image(systemName: state.condition == .error ? "exclamationmark.triangle.fill" : "waveform")
                .font(.system(size: 34, weight: .medium))
                .foregroundStyle(state.condition == .error ? LumiColor.warning : LumiColor.accent)
            Text("Track Lighting Editor")
                .font(LumiTypography.screenTitle)
            Text(
                state.condition == .error
                    ? (state.diagnostic ?? "The local Track Editor is unavailable.")
                    : state.page.tracks.isEmpty
                        ? "Select a workflow step or playlist containing tracks to begin."
                        : "Loading the selected track…"
            )
            .font(LumiTypography.metadata)
            .foregroundStyle(LumiColor.textSecondary)
            if state.condition != .error, !state.page.tracks.isEmpty {
                ProgressView().controlSize(.small)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.trackEditor.placeholder")
    }

    @ViewBuilder
    private var conditionBanner: some View {
        if (state.condition != .ready && state.condition != .empty) || state.diagnostic != nil {
            HStack(spacing: LumiSpacing.small) {
                Image(systemName: state.condition.componentState.systemImage)
                    .foregroundStyle(state.condition.componentState.color)
                Text(state.diagnostic ?? conditionDescription(state.condition))
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textPrimary)
                Spacer()
            }
            .padding(.horizontal, LumiSpacing.xLarge)
            .padding(.vertical, LumiSpacing.small)
            .background(state.condition.componentState.color.opacity(0.12))
            .accessibilityIdentifier("lumi.library.condition.\(state.condition.rawValue)")
        }
    }

    private var collectionNavigation: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            Picker("Browser", selection: $browserMode) {
                ForEach(LibraryBrowserMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .focusable()
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityIdentifier("lumi.library.browserMode")

            if browserMode == .playlists {
                playlistNavigation
            } else {
                workflowNavigation
            }

        }
        .padding(LumiSpacing.large)
        .background(LumiColor.surface)
        .onChange(of: browserMode) { _, mode in
            guard rendersInteractiveControls else { return }
            if mode == .playlists {
                selectPlaylist(nil)
            } else if let stepID = selectedWorkflowStepID {
                selectWorkflowStep(stepID)
            } else {
                selectWorkflow(selectedWorkflowFilter ?? .changedAfterUSBSync)
            }
        }
    }

    private var playlistNavigation: some View {
        Group {
            Text(localized("library.sources"))
                .font(LumiTypography.sectionTitle)
            Button { selectPlaylist(nil) } label: {
                navigationLabel(
                    localized("library.collection"),
                    count: state.collectionTotal,
                    systemImage: "music.note.list",
                    selected: selectedPlaylistID == nil && state.query.workflowFilter == nil
                )
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("lumi.library.collection")

            Text(localized("library.playlists"))
                .font(LumiTypography.sectionTitle)
                .padding(.top, LumiSpacing.small)
            ScrollView(.vertical) {
                LazyVStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    ForEach(state.playlists) { playlist in
                        Button { selectPlaylist(playlist.id) } label: {
                            navigationLabel(
                                playlist.name,
                                count: playlist.trackCount,
                                systemImage: "music.note",
                                selected: selectedPlaylistID == playlist.id
                                    && state.query.workflowFilter == nil,
                                subtitle: playlistSourceLabel(playlist)
                            )
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("lumi.library.playlist.\(playlist.id)")
                    }
                }
            }
            .scrollIndicators(.automatic)
            .frame(maxHeight: .infinity)
            .accessibilityIdentifier("lumi.library.playlists")
        }
    }

    private var workflowNavigation: some View {
        ScrollView(.vertical) {
            LazyVStack(alignment: .leading, spacing: LumiSpacing.small) {
                Text("SYSTEM")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                workflowButton(
                    .changedAfterUSBSync,
                    title: "Changed after USB sync",
                    count: state.workflow.changedAfterUSBSync,
                    systemImage: "externaldrive.badge.exclamationmark",
                    color: LumiColor.warning
                )
                workflowButton(
                    .versionCandidates,
                    title: "New track versions",
                    count: state.workflow.versionCandidates,
                    systemImage: "arrow.triangle.2.circlepath.circle",
                    color: LumiColor.accent
                )
                Text("PREPARATION")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .padding(.top, LumiSpacing.medium)
                ForEach(state.workflowCatalog.steps.filter { !$0.archived }) { step in
                    workflowStepButton(step)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollIndicators(.automatic)
        .accessibilityIdentifier("lumi.library.workflow")
    }

    private func workflowStepButton(_ step: WorkflowStepDefinition) -> some View {
        Button { selectWorkflowStep(step.id) } label: {
            HStack(spacing: LumiSpacing.small) {
                Image(systemName: step.icon)
                    .foregroundStyle(workflowColor(step.colorRGB))
                Text(step.displayName).lineLimit(2)
                Spacer()
                Text("\(state.workflow.stepCounts[step.id, default: 0])")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .foregroundStyle(
                state.query.workflowStepID == step.id ? LumiColor.accent : LumiColor.textPrimary
            )
            .padding(.horizontal, LumiSpacing.small)
            .frame(minHeight: LumiControlMetric.standardHeight)
            .contentShape(Rectangle())
            .background(
                state.query.workflowStepID == step.id ? LumiColor.accent.opacity(0.14) : Color.clear
            )
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.workflow.step.\(step.id)")
    }

    private func workflowButton(
        _ filter: TrackWorkflowFilter,
        title: String,
        count: UInt64,
        systemImage: String,
        color: Color
    ) -> some View {
        Button { selectWorkflow(filter) } label: {
            HStack(spacing: LumiSpacing.small) {
                Image(systemName: systemImage).foregroundStyle(color)
                Text(title).lineLimit(2)
                Spacer()
                Text("\(count)")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .foregroundStyle(
                state.query.workflowFilter == filter ? LumiColor.accent : LumiColor.textPrimary
            )
            .padding(.horizontal, LumiSpacing.small)
            .frame(minHeight: LumiControlMetric.standardHeight)
            .contentShape(Rectangle())
            .background(
                state.query.workflowFilter == filter ? LumiColor.accent.opacity(0.14) : Color.clear
            )
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.workflow.\(filter.rawValue)")
    }

    private func navigationLabel(
        _ title: String,
        count: UInt64,
        systemImage: String,
        selected: Bool,
        subtitle: String? = nil
    ) -> some View {
        HStack(spacing: LumiSpacing.small) {
            Image(systemName: systemImage)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).lineLimit(1)
                if let subtitle {
                    Text(subtitle.uppercased())
                        .font(LumiTypography.technical)
                        .foregroundStyle(
                            subtitle.hasSuffix("· USB")
                                ? LumiColor.success
                                : LumiColor.textSecondary
                        )
                        .lineLimit(1)
                }
            }
            Spacer()
            Text("\(count)")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .foregroundStyle(selected ? LumiColor.accent : LumiColor.textPrimary)
        .padding(.horizontal, LumiSpacing.small)
        .frame(height: subtitle == nil ? LumiControlMetric.standardHeight : 44)
        .contentShape(Rectangle())
        .background(selected ? LumiColor.accent.opacity(0.14) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func playlistSourceLabel(_ playlist: LibraryPlaylist) -> String {
        let devices = state.rekordboxDevices.filter { device in
            device.playlists.contains {
                $0.name.trimmingCharacters(in: .whitespacesAndNewlines)
                    .localizedCaseInsensitiveCompare(
                        playlist.name.trimmingCharacters(in: .whitespacesAndNewlines)
                    ) == .orderedSame
            }
        }
        if devices.count > 1 {
            return "\(devices.count) USB sources"
        }
        if let device = devices.first {
            return "\(device.displayName) · USB"
        }
        if playlist.sourcePlaylistID.hasPrefix("onelibrary:") {
            return "USB"
        }
        return "Legacy library"
    }

    private var trackBrowser: some View {
        VStack(spacing: 0) {
            trackHeader
            conditionBanner
            Divider()
            if state.condition == .importing {
                statePlaceholder(.importing)
            } else if state.condition == .error {
                statePlaceholder(.error)
            } else if visibleTracks.isEmpty {
                statePlaceholder(.empty)
            } else {
                trackTable
            }
            Divider()
            pagination
        }
        .background(LumiColor.canvas)
    }

    private var trackHeader: some View {
        HStack(spacing: LumiSpacing.medium) {
            Text(String(format: localized("library.trackCount"), state.page.total))
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textSecondary)
            Spacer()
            if rendersInteractiveControls {
                TextField(localized("library.search"), text: $search)
                    .textFieldStyle(.roundedBorder)
                    .focused($isSearchFocused)
                    .padding(.trailing, search.isEmpty ? 0 : 22)
                    .overlay(alignment: .trailing) {
                        if !search.isEmpty {
                            Button {
                                isSearchFocused = false
                                search = ""
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            .buttonStyle(.plain)
                            .help("Clear search")
                            .accessibilityIdentifier("lumi.library.search.clear")
                        }
                    }
                    .frame(minWidth: 180, idealWidth: 260, maxWidth: 320)
                    .onSubmit { submitQuery(offset: 0) }
                    .accessibilityIdentifier("lumi.library.search")
                if browserMode == .playlists {
                    Picker(localized("library.filter"), selection: $readinessFilter) {
                        ForEach(LibraryReadinessFilter.allCases) { filter in
                            Text(readinessName(filter)).tag(filter)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 150)
                    .accessibilityIdentifier("lumi.library.readinessFilter")
                }
            }
            if let selectedTrack {
                if let localPlaybackFeedback {
                    Image(
                        systemName: localPlaybackFeedbackIsError
                            ? "exclamationmark.triangle.fill"
                            : "checkmark.circle.fill"
                    )
                    .foregroundStyle(localPlaybackFeedbackIsError ? LumiColor.warning : LumiColor.success)
                    .help(localPlaybackFeedback)
                    .accessibilityIdentifier("lumi.library.localPlaybackFeedback")
                }
                localDeckToolbarButton(selectedTrack, deckID: 1)
                localDeckToolbarButton(selectedTrack, deckID: 2)
            }
        }
        .padding(.horizontal, LumiSpacing.large)
        .frame(height: LumiControlMetric.prominentHeight)
        .background(LumiColor.surface)
    }

    private var trackTable: some View {
        Table(
            visibleTracks,
            selection: $selectedTrackID,
            sortOrder: $tableSortOrder,
            columnCustomization: $trackTableCustomization
        ) {
            TableColumn(localized("library.trackTitle"), value: \.title) { track in
                editorLoadingCell(track) {
                    HStack(spacing: LumiSpacing.small) {
                        LumiTrackColorSwatch(colorRGB: track.colorRGB, diameter: 12)
                            .help(LumiTrackColorPalette.accessibilityLabel(for: track.colorRGB))
                        Text(track.title)
                            .font(LumiTypography.body.weight(.semibold))
                            .foregroundStyle(LumiColor.textPrimary)
                            .lineLimit(1)
                        if requestedEditorTrackID == track.id {
                            ProgressView()
                                .controlSize(.mini)
                                .accessibilityLabel("Loading track editor")
                        }
                    }
                    .accessibilityIdentifier("lumi.library.track.\(track.id)")
                }
            }
            .width(min: 160, ideal: 260, max: 520)
            .customizationID("title")

            TableColumn(localized("library.artist"), value: \.artist) { track in
                editorLoadingCell(track) {
                    Text(track.artist).lineLimit(1)
                }
            }
            .width(min: 120, ideal: 190, max: 420)
            .customizationID("artist")

            TableColumn(localized("library.bpm"), value: \.bpmMilli) { track in
                editorLoadingCell(track) {
                    Text(formatBPM(track.bpmMilli)).font(LumiTypography.technical)
                }
            }
            .width(min: 54, ideal: 64, max: 90)
            .customizationID("bpm")

            TableColumn(localized("library.key"), value: \.sortKey) { track in
                editorLoadingCell(track) {
                    Text(KeyNotationFormatter(notation: keyNotation).string(from: track.musicalKey))
                        .font(LumiTypography.technical)
                }
            }
            .width(min: 44, ideal: 54, max: 80)
            .customizationID("key")

            TableColumn(localized("library.duration"), value: \.durationMillis) { track in
                editorLoadingCell(track) {
                    Text(formatDuration(track.durationMillis)).font(LumiTypography.technical)
                }
            }
            .width(min: 56, ideal: 68, max: 100)
            .customizationID("duration")

            TableColumn(localized("library.usbSources"), value: \.sortUSBSources) { track in
                editorLoadingCell(track) {
                    let sources = track.usbSources.map(\.displayName).joined(separator: ", ")
                    Text(sources.isEmpty ? "—" : sources)
                        .lineLimit(1)
                }
            }
            .width(min: 120, ideal: 180, max: 360)
            .customizationID("usbSources")

            TableColumn(localized("library.timelineRevision"), value: \.sortTimelineRevision) { track in
                editorLoadingCell(track) {
                    Text(track.timelineRevisionLabel)
                        .font(LumiTypography.technical)
                }
            }
            .width(min: 70, ideal: 92, max: 140)
            .customizationID("timeline")

            TableColumn(localized("library.readiness"), value: \.sortReadiness) { track in
                editorLoadingCell(track) {
                    Label(readinessName(track.readiness), systemImage: readinessIcon(track.readiness))
                        .font(LumiTypography.caption)
                        .foregroundStyle(readinessColor(track.readiness))
                }
            }
            .width(min: 88, ideal: 112, max: 180)
            .customizationID("readiness")

            TableColumn("Workflow", value: \.sortPreparationStatus) { track in
                workflowCell(track)
            }
            .width(min: 110, ideal: 150, max: 240)
            .customizationID("workflowStatus")

        }
        .tableStyle(.inset(alternatesRowBackgrounds: true))
        .scrollContentBackground(.hidden)
        .background(LumiColor.canvas)
        .overlay {
            TableDoubleClickMonitor {
                guard rendersInteractiveControls, let selectedTrackID else { return }
                requestEditor(trackID: selectedTrackID)
            }
        }
        .help("Double-click a track to load it in the Track Lighting Editor.")
        .accessibilityIdentifier("lumi.library.trackTable")
    }

    private func editorLoadingCell<Content: View>(
        _: LibraryTrack,
        @ViewBuilder content: () -> Content
    ) -> some View {
        content()
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
    }

    private func workflowCell(_ track: LibraryTrack) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Label(
                workflowStep(for: track).displayName,
                systemImage: workflowStep(for: track).icon
            )
            if let attention = track.workflow.attention {
                Text(attentionSummary(attention))
                    .foregroundStyle(LumiColor.warning)
                    .help(attention.reasons.map(attentionReasonName).joined(separator: ", "))
            }
        }
        .font(LumiTypography.caption)
        .foregroundStyle(preparationStatusColor(track.workflow))
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func workflowStep(for track: LibraryTrack) -> WorkflowStepDefinition {
        state.workflowCatalog.steps.first { $0.id == track.workflow.stepID }
            ?? WorkflowStepDefinition(
                id: track.workflow.stepID,
                displayName: preparationStatusName(track.workflow.preparationStatus),
                icon: preparationStatusIcon(track.workflow.preparationStatus),
                colorRGB: 0x8A949F,
                sortOrder: 1,
                archived: false,
                rules: []
            )
    }

    private func localDeckToolbarButton(_ track: LibraryTrack, deckID: UInt64) -> some View {
        Button {
            guard let timelineRevision = track.timelineRevision else { return }
            onLoadOnLocalDeck(
                LibraryDeckLoadRequest(
                    trackID: track.id,
                    deckID: deckID,
                    expectedTimelineRevision: timelineRevision
                )
            )
        } label: {
            Label(
                "Deck \(deckID)",
                systemImage: "arrow.down.to.line.compact"
            )
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .disabled(track.timelineRevision == nil)
        .help(String(format: localized("library.loadOnDeck"), deckID))
        .accessibilityIdentifier("lumi.library.loadDeck\(deckID)")
    }

    private var pagination: some View {
        HStack {
            Button(localized("library.previous")) {
                let offset = state.query.offset >= UInt32(state.query.limit)
                    ? state.query.offset - UInt32(state.query.limit)
                    : 0
                submitQuery(offset: offset)
            }
            .disabled(state.query.offset == 0)
            Spacer()
            Text(
                String(
                    format: localized("library.page"),
                    LibraryWorkspacePresenter.pageNumber(in: state),
                    LibraryWorkspacePresenter.pageCount(in: state)
                )
            )
            .font(LumiTypography.technical)
            Spacer()
            Button(localized("library.next")) {
                submitQuery(offset: state.query.offset + UInt32(state.query.limit))
            }
            .disabled(UInt64(state.query.offset) + UInt64(state.query.limit) >= state.page.total)
        }
        .padding(.horizontal, LumiSpacing.large)
        .frame(height: LumiControlMetric.prominentHeight)
    }

    private func statePlaceholder(_ condition: LibraryCondition) -> some View {
        VStack(spacing: LumiSpacing.medium) {
            Image(systemName: condition.componentState.systemImage)
                .font(.system(size: 30))
                .foregroundStyle(condition.componentState.color)
            Text(condition == .empty ? emptyStateTitle : conditionTitle(condition))
                .font(LumiTypography.sectionTitle)
            Text(condition == .empty ? emptyStateDetail : (state.diagnostic ?? conditionDescription(condition)))
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textSecondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
            if condition == .importing {
                ProgressView().controlSize(.small)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("lumi.library.placeholder.\(condition.rawValue)")
    }

    private var selectedTrack: LibraryTrack? {
        state.page.tracks.first { $0.id == selectedTrackID }
    }

    private var emptyStateTitle: String {
        if state.query.workflowFilter != nil || state.query.workflowStepID != nil {
            return "Nothing to review"
        }
        return conditionTitle(.empty)
    }

    private var emptyStateDetail: String {
        if let stepID = state.query.workflowStepID,
           let step = state.workflowCatalog.steps.first(where: { $0.id == stepID }) {
            return "No tracks currently match \(step.displayName). This is a normal empty workflow step, not a Library error."
        }
        if state.query.workflowFilter != nil {
            return "No tracks currently need attention in this queue. This is a normal empty result."
        }
        return state.diagnostic ?? conditionDescription(.empty)
    }

    private var visibleTracks: [LibraryTrack] {
        LibraryWorkspacePresenter.visibleTracks(in: state, filter: readinessFilter)
    }

    private func selectPlaylist(_ id: UInt64?) {
        selectedPlaylistID = id
        selectedWorkflowFilter = nil
        selectedWorkflowStepID = nil
        let playlistSort = id == nil && sortBy == .playlist ? LibraryTrackSortField.title : sortBy
        sortBy = playlistSort
        onQuery(
            LibraryQueryRequest(
                search: search,
                playlistID: id,
                offset: 0,
                sortBy: playlistSort,
                sortDirection: sortDirection,
                workflowFilter: nil
            )
        )
    }

    private func selectWorkflow(_ filter: TrackWorkflowFilter) {
        selectedWorkflowFilter = filter
        selectedWorkflowStepID = nil
        selectedPlaylistID = nil
        onQuery(
            LibraryQueryRequest(
                search: search,
                playlistID: nil,
                offset: 0,
                sortBy: sortBy == .playlist ? .title : sortBy,
                sortDirection: sortDirection,
                workflowFilter: filter
            )
        )
    }

    private func selectWorkflowStep(_ stepID: String) {
        selectedWorkflowFilter = nil
        selectedWorkflowStepID = stepID
        selectedPlaylistID = nil
        onQuery(
            LibraryQueryRequest(
                search: search,
                playlistID: nil,
                offset: 0,
                sortBy: sortBy == .playlist ? .title : sortBy,
                sortDirection: sortDirection,
                workflowFilter: nil,
                workflowStepID: stepID
            )
        )
    }

    private func submitQuery(search querySearch: String? = nil, offset: UInt32) {
        onQuery(
            LibraryQueryRequest(
                search: querySearch ?? search,
                playlistID: state.query.playlistID,
                offset: offset,
                limit: state.query.limit,
                sortBy: sortBy,
                sortDirection: sortDirection,
                workflowFilter: state.query.workflowFilter,
                workflowStepID: state.query.workflowStepID
            )
        )
    }

    private func applyTableSort(_ order: [KeyPathComparator<LibraryTrack>]) {
        guard let comparator = order.first else { return }
        let keyPath = comparator.keyPath as AnyKeyPath
        let field: LibraryTrackSortField
        switch keyPath {
        case \LibraryTrack.title: field = .title
        case \LibraryTrack.artist: field = .artist
        case \LibraryTrack.bpmMilli: field = .bpm
        case \LibraryTrack.sortKey: field = .key
        case \LibraryTrack.durationMillis: field = .duration
        case \LibraryTrack.sortUSBSources: field = .usbSources
        case \LibraryTrack.sortTimelineRevision: field = .timelineRevision
        case \LibraryTrack.sortReadiness: field = .readiness
        case \LibraryTrack.sortPreparationStatus: field = .preparationStatus
        case \LibraryTrack.sortAttention: field = .attention
        case \LibraryTrack.sourceTrackID: field = .sourceTrackID
        case \LibraryTrack.analysisRevision: field = .analysisRevision
        default: return
        }
        let direction: LibraryTrackSortDirection = comparator.order == .forward
            ? .ascending
            : .descending
        guard field != sortBy || direction != sortDirection else { return }
        sortBy = field
        sortDirection = direction
        onQuery(
            LibraryQueryRequest(
                search: search,
                playlistID: state.query.playlistID,
                offset: 0,
                limit: state.query.limit,
                sortBy: field,
                sortDirection: direction,
                workflowFilter: state.query.workflowFilter,
                workflowStepID: state.query.workflowStepID
            )
        )
    }

    private func requestInitialEditorIfNeeded() {
        guard rendersInteractiveControls,
              editorAnalysis == nil,
              requestedEditorTrackID == nil,
              state.condition == .ready,
              let trackID = selectedTrackID ?? visibleTracks.first?.id else { return }
        requestEditor(trackID: trackID)
    }

    private func requestEditor(trackID: UInt64) {
        guard editorAnalysis?.track.id != trackID,
              requestedEditorTrackID != trackID else { return }
        requestedEditorTrackID = trackID
        onOpenEditor(trackID)
    }
}

private func workflowColor(_ rgb: UInt32) -> Color {
    Color(
        red: Double((rgb >> 16) & 0xFF) / 255,
        green: Double((rgb >> 8) & 0xFF) / 255,
        blue: Double(rgb & 0xFF) / 255
    )
}

private func formatBPM(_ value: UInt64) -> String {
    String(format: "%.1f", Double(value) / 1_000)
}

private func formatDuration(_ millis: UInt64) -> String {
    let seconds = millis / 1_000
    return String(format: "%llu:%02llu", seconds / 60, seconds % 60)
}

private func readinessIcon(_ value: LibraryReadiness) -> String {
    switch value {
    case .ready: "checkmark.circle.fill"
    case .missingAnalysis: "waveform.badge.exclamationmark"
    case .staleSource: "clock.badge.exclamationmark"
    case .conflict: "arrow.trianglehead.branch"
    }
}

private func readinessColor(_ value: LibraryReadiness) -> Color {
    switch value {
    case .ready: LumiColor.success
    case .missingAnalysis, .staleSource: LumiColor.warning
    case .conflict: LumiColor.destructive
    }
}

private func readinessName(_ value: LibraryReadiness) -> String {
    localized("readiness.\(value.rawValue)")
}

private func readinessName(_ value: LibraryReadinessFilter) -> String {
    localized("filter.\(value.rawValue)")
}

private func preparationStatusName(_ value: TrackPreparationStatus) -> String {
    switch value {
    case .notStarted: "Not Started"
    case .inProgress: "In Progress"
    case .readyForShow: "Ready for Show"
    }
}

private func preparationStatusIcon(_ value: TrackPreparationStatus) -> String {
    switch value {
    case .notStarted: "circle"
    case .inProgress: "circle.lefthalf.filled"
    case .readyForShow: "checkmark.circle.fill"
    }
}

private func preparationStatusColor(_ workflow: TrackWorkflowState) -> Color {
    if workflow.attention != nil { return LumiColor.warning }
    switch workflow.preparationStatus {
    case .notStarted: return LumiColor.textSecondary
    case .inProgress: return LumiColor.warning
    case .readyForShow: return LumiColor.success
    }
}

private func attentionReasonName(_ value: TrackAttentionReason) -> String {
    switch value {
    case .metadataChanged: "Metadata"
    case .waveformChanged: "Waveform"
    case .beatGridChanged: "Beatgrid"
    case .hotCuesChanged: "Hot cues"
    case .sourcePhrasesChanged: "Source phrases"
    }
}

private func attentionSummary(_ attention: TrackWorkflowAttention) -> String {
    guard let first = attention.reasons.first else { return "USB change" }
    let remaining = attention.reasons.count - 1
    return remaining == 0
        ? attentionReasonName(first)
        : "\(attentionReasonName(first)) +\(remaining)"
}

private func conditionTitle(_ value: LibraryCondition) -> String {
    localized("condition.\(value.rawValue).title")
}

private func conditionDescription(_ value: LibraryCondition) -> String {
    localized("condition.\(value.rawValue).detail")
}

private func localized(_ key: String) -> String {
    LibraryWorkspaceLocalization.value(key)
}
