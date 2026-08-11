import Foundation
import LumiDesignSystem
import SwiftUI

public struct LibraryQueryRequest: Equatable, Sendable {
    public let search: String
    public let playlistID: UInt64?
    public let offset: UInt32
    public let limit: UInt16

    public init(search: String, playlistID: UInt64?, offset: UInt32, limit: UInt16 = 50) {
        self.search = search
        self.playlistID = playlistID
        self.offset = offset
        self.limit = limit
    }
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
    private let onLoadOnLocalDeck: @MainActor (LibraryDeckLoadRequest) -> Void
    private let timelineFeedback: String?
    private let localPlaybackFeedback: String?
    private let localPlaybackFeedbackIsError: Bool
    private let rendersInteractiveControls: Bool
    @Binding private var keyNotation: KeyNotationPreference
    @State private var search: String
    @State private var selectedTrackID: UInt64?
    @State private var selectedPlaylistID: UInt64?
    @State private var readinessFilter: LibraryReadinessFilter = .all
    @State private var editorAnalysis: TrackEditorAnalysis?
    @State private var requestedEditorTrackID: UInt64?
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
        onLoadOnLocalDeck: @escaping @MainActor (LibraryDeckLoadRequest) -> Void = { _ in },
        timelineFeedback: String? = nil,
        localPlaybackFeedback: String? = nil,
        localPlaybackFeedbackIsError: Bool = false
    ) {
        self.state = state
        self.onQuery = onQuery
        self.onOpenEditor = onOpenEditor
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        self.onSourceReconcile = onSourceReconcile
        self.onLoadOnLocalDeck = onLoadOnLocalDeck
        self.timelineFeedback = timelineFeedback
        self.localPlaybackFeedback = localPlaybackFeedback
        self.localPlaybackFeedbackIsError = localPlaybackFeedbackIsError
        self.rendersInteractiveControls = rendersInteractiveControls
        _keyNotation = keyNotation
        _search = State(initialValue: state.query.search)
        _selectedTrackID = State(initialValue: state.page.tracks.first?.id)
        _selectedPlaylistID = State(initialValue: state.query.playlistID)
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
                            keyNotation: keyNotation,
                            feedback: timelineFeedback,
                            isEmbedded: true,
                            onTimelineEdit: onTimelineEdit,
                            onTimelineHistory: onTimelineHistory,
                            onSourceReconcile: onSourceReconcile
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
        .onChange(of: state.query.playlistID) { _, value in selectedPlaylistID = value }
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
                    : "Loading the selected track…"
            )
            .font(LumiTypography.metadata)
            .foregroundStyle(LumiColor.textSecondary)
            if state.condition != .error {
                ProgressView().controlSize(.small)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.trackEditor.placeholder")
    }

    @ViewBuilder
    private var conditionBanner: some View {
        if state.condition != .ready || state.diagnostic != nil {
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
            Text(localized("library.sources"))
                .font(LumiTypography.sectionTitle)
            Button {
                selectPlaylist(nil)
            } label: {
                navigationLabel(
                    localized("library.collection"),
                    count: state.collectionTotal,
                    systemImage: "music.note.list",
                    selected: selectedPlaylistID == nil
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
                        Button {
                            selectPlaylist(playlist.id)
                        } label: {
                            navigationLabel(
                                playlist.name,
                                count: playlist.trackCount,
                                systemImage: "music.note",
                                selected: selectedPlaylistID == playlist.id,
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
        .padding(LumiSpacing.large)
        .background(LumiColor.surface)
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
        if let device = state.rekordboxDevices.first(where: { device in
            device.playlists.contains { $0.libraryPlaylistID == playlist.id }
        }) {
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
                    .frame(minWidth: 180, idealWidth: 260, maxWidth: 320)
                    .onSubmit { submitQuery(offset: 0) }
                    .accessibilityIdentifier("lumi.library.search")
                Picker(localized("library.filter"), selection: $readinessFilter) {
                    ForEach(LibraryReadinessFilter.allCases) { filter in
                        Text(readinessName(filter)).tag(filter)
                    }
                }
                .labelsHidden()
                .frame(width: 150)
                .accessibilityIdentifier("lumi.library.readinessFilter")
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
            columnCustomization: $trackTableCustomization
        ) {
            TableColumn(localized("library.trackTitle")) { track in
                editorLoadingCell(track) {
                    HStack(spacing: LumiSpacing.small) {
                        colorSwatch(track.colorRGB, height: 18)
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

            TableColumn(localized("library.artist")) { track in
                editorLoadingCell(track) {
                    Text(track.artist).lineLimit(1)
                }
            }
            .width(min: 120, ideal: 190, max: 420)
            .customizationID("artist")

            TableColumn(localized("library.bpm")) { track in
                editorLoadingCell(track) {
                    Text(formatBPM(track.bpmMilli)).font(LumiTypography.technical)
                }
            }
            .width(min: 54, ideal: 64, max: 90)
            .customizationID("bpm")

            TableColumn(localized("library.key")) { track in
                editorLoadingCell(track) {
                    Text(KeyNotationFormatter(notation: keyNotation).string(from: track.musicalKey))
                        .font(LumiTypography.technical)
                }
            }
            .width(min: 44, ideal: 54, max: 80)
            .customizationID("key")

            TableColumn(localized("library.duration")) { track in
                editorLoadingCell(track) {
                    Text(formatDuration(track.durationMillis)).font(LumiTypography.technical)
                }
            }
            .width(min: 56, ideal: 68, max: 100)
            .customizationID("duration")

            TableColumn(localized("library.usbSources")) { track in
                editorLoadingCell(track) {
                    let sources = track.usbSources.map(\.displayName).joined(separator: ", ")
                    Text(sources.isEmpty ? "—" : sources)
                        .lineLimit(1)
                }
            }
            .width(min: 120, ideal: 180, max: 360)
            .customizationID("usbSources")

            TableColumn(localized("library.timelineRevision")) { track in
                editorLoadingCell(track) {
                    Text(track.timelineRevision.map { "R\($0)" } ?? "—")
                        .font(LumiTypography.technical)
                }
            }
            .width(min: 70, ideal: 92, max: 140)
            .customizationID("timeline")

            TableColumn(localized("library.readiness")) { track in
                editorLoadingCell(track) {
                    Label(readinessName(track.readiness), systemImage: readinessIcon(track.readiness))
                        .font(LumiTypography.caption)
                        .foregroundStyle(readinessColor(track.readiness))
                }
            }
            .width(min: 88, ideal: 112, max: 180)
            .customizationID("readiness")

            TableColumn(localized("library.sourceTrackID")) { track in
                editorLoadingCell(track) {
                    Text(track.sourceTrackID).font(LumiTypography.technical).lineLimit(1)
                }
            }
            .width(min: 110, ideal: 170, max: 360)
            .defaultVisibility(.hidden)
            .customizationID("sourceTrackID")

            TableColumn(localized("library.analysisRevision")) { track in
                editorLoadingCell(track) {
                    Text(track.analysisRevision).font(LumiTypography.technical).lineLimit(1)
                }
            }
            .width(min: 110, ideal: 170, max: 360)
            .defaultVisibility(.hidden)
            .customizationID("analysisRevision")
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
            Text(conditionTitle(condition))
                .font(LumiTypography.sectionTitle)
            Text(state.diagnostic ?? conditionDescription(condition))
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

    private func colorSwatch(_ rgb: UInt32?, height: CGFloat = 32) -> some View {
        let color = rgb.map {
            Color(
                red: Double(($0 >> 16) & 0xff) / 255,
                green: Double(($0 >> 8) & 0xff) / 255,
                blue: Double($0 & 0xff) / 255
            )
        } ?? LumiColor.surfaceElevated
        return RoundedRectangle(cornerRadius: LumiRadius.compact)
            .fill(color)
            .frame(width: 12, height: height)
            .overlay { RoundedRectangle(cornerRadius: LumiRadius.compact).stroke(LumiColor.border) }
    }

    private var selectedTrack: LibraryTrack? {
        state.page.tracks.first { $0.id == selectedTrackID }
    }

    private var visibleTracks: [LibraryTrack] {
        LibraryWorkspacePresenter.visibleTracks(in: state, filter: readinessFilter)
    }

    private func selectPlaylist(_ id: UInt64?) {
        selectedPlaylistID = id
        onQuery(LibraryQueryRequest(search: search, playlistID: id, offset: 0))
    }

    private func submitQuery(search querySearch: String? = nil, offset: UInt32) {
        onQuery(
            LibraryQueryRequest(
                search: querySearch ?? search,
                playlistID: state.query.playlistID,
                offset: offset,
                limit: state.query.limit
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

private func conditionTitle(_ value: LibraryCondition) -> String {
    localized("condition.\(value.rawValue).title")
}

private func conditionDescription(_ value: LibraryCondition) -> String {
    localized("condition.\(value.rawValue).detail")
}

private func localized(_ key: String) -> String {
    LibraryWorkspaceLocalization.value(key)
}
