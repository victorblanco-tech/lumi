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

public struct LibraryWorkspaceView: View {
    private let state: LibraryWorkspaceState
    private let onQuery: @MainActor (LibraryQueryRequest) -> Void
    private let onOpenEditor: @MainActor (UInt64) -> Void
    private let onCloseEditor: @MainActor () -> Void
    private let onTimelineEdit: @MainActor (TrackTimelineEditRequest) -> Void
    private let onTimelineHistory: @MainActor (TrackTimelineHistoryRequest) -> Void
    private let onSourceReconcile: @MainActor (TrackSourceReconcileRequest) -> Void
    private let timelineFeedback: String?
    private let rendersInteractiveControls: Bool
    @Binding private var keyNotation: KeyNotationPreference
    @State private var search: String
    @State private var selectedTrackID: UInt64?
    @State private var readinessFilter: LibraryReadinessFilter = .all
    @State private var editorAnalysis: TrackEditorAnalysis?

    public init(
        state: LibraryWorkspaceState,
        keyNotation: Binding<KeyNotationPreference>,
        rendersInteractiveControls: Bool = true,
        onQuery: @escaping @MainActor (LibraryQueryRequest) -> Void = { _ in },
        onOpenEditor: @escaping @MainActor (UInt64) -> Void = { _ in },
        onCloseEditor: @escaping @MainActor () -> Void = {},
        onTimelineEdit: @escaping @MainActor (TrackTimelineEditRequest) -> Void = { _ in },
        onTimelineHistory: @escaping @MainActor (TrackTimelineHistoryRequest) -> Void = { _ in },
        onSourceReconcile: @escaping @MainActor (TrackSourceReconcileRequest) -> Void = { _ in },
        timelineFeedback: String? = nil
    ) {
        self.state = state
        self.onQuery = onQuery
        self.onOpenEditor = onOpenEditor
        self.onCloseEditor = onCloseEditor
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        self.onSourceReconcile = onSourceReconcile
        self.timelineFeedback = timelineFeedback
        self.rendersInteractiveControls = rendersInteractiveControls
        _keyNotation = keyNotation
        _search = State(initialValue: state.query.search)
        _selectedTrackID = State(initialValue: state.page.tracks.first?.id)
        _editorAnalysis = State(initialValue: state.editor)
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            conditionBanner
            HStack(spacing: 0) {
                collectionNavigation
                    .frame(minWidth: 180, idealWidth: 210, maxWidth: 250)
                Divider()
                trackBrowser
                    .frame(minWidth: 480, maxWidth: .infinity)
                Divider()
                inspector
                    .frame(minWidth: 260, idealWidth: 310, maxWidth: 360)
            }
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.library.workspace")
        .onChange(of: state.query.search) { _, value in search = value }
        .onChange(of: state.page.tracks) { _, tracks in
            if !tracks.contains(where: { $0.id == selectedTrackID }) {
                selectedTrackID = tracks.first?.id
            }
        }
        .onChange(of: state.editor) { _, editor in
            editorAnalysis = editor
        }
        .sheet(item: $editorAnalysis, onDismiss: onCloseEditor) { analysis in
            TrackLightingEditorView(
                analysis: analysis,
                autoloopCatalog: state.autoloopCatalog,
                keyNotation: keyNotation,
                feedback: timelineFeedback,
                onTimelineEdit: onTimelineEdit,
                onTimelineHistory: onTimelineHistory,
                onSourceReconcile: onSourceReconcile
            )
        }
    }

    private var header: some View {
        HStack(spacing: LumiSpacing.large) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(localized("library.title"))
                    .font(LumiTypography.screenTitle)
                    .foregroundStyle(LumiColor.textPrimary)
                Text(localized("library.subtitle"))
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            if rendersInteractiveControls {
                TextField(localized("library.search"), text: $search)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)
                    .onSubmit { submitQuery(offset: 0) }
                    .accessibilityIdentifier("lumi.library.search")
                Picker(localized("library.filter"), selection: $readinessFilter) {
                    ForEach(LibraryReadinessFilter.allCases) { filter in
                        Text(readinessName(filter)).tag(filter)
                    }
                }
                .frame(width: 180)
                .accessibilityIdentifier("lumi.library.readinessFilter")
            } else {
                Label(localized("library.search"), systemImage: "magnifyingglass")
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(width: 300, height: LumiControlMetric.standardHeight, alignment: .leading)
                    .background(LumiColor.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                Label(readinessName(readinessFilter), systemImage: "line.3.horizontal.decrease.circle")
                    .font(LumiTypography.metadata)
                    .padding(.horizontal, LumiSpacing.medium)
                    .frame(width: 180, height: LumiControlMetric.standardHeight, alignment: .leading)
                    .background(LumiColor.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            }
        }
        .padding(LumiSpacing.xLarge)
        .background(LumiColor.surface)
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
                    selected: state.query.playlistID == nil
                )
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("lumi.library.collection")

            Text(localized("library.playlists"))
                .font(LumiTypography.sectionTitle)
                .padding(.top, LumiSpacing.small)
            ForEach(state.playlists) { playlist in
                Button {
                    selectPlaylist(playlist.id)
                } label: {
                    navigationLabel(
                        playlist.name,
                        count: playlist.trackCount,
                        systemImage: "music.note",
                        selected: state.query.playlistID == playlist.id
                    )
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("lumi.library.playlist.\(playlist.id)")
            }
            Spacer()
            if let source = state.source {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Label(source.name, systemImage: "externaldrive.fill")
                        .font(LumiTypography.metadata)
                    Text(source.revision)
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Label(
                        state.providerKind.capitalized,
                        systemImage: state.condition.componentState.systemImage
                    )
                    .font(LumiTypography.caption)
                    .foregroundStyle(state.condition.componentState.color)
                }
                .accessibilityIdentifier("lumi.library.source")
            }
        }
        .padding(LumiSpacing.large)
        .background(LumiColor.surface)
    }

    private func navigationLabel(
        _ title: String,
        count: UInt64,
        systemImage: String,
        selected: Bool
    ) -> some View {
        HStack(spacing: LumiSpacing.small) {
            Image(systemName: systemImage)
            Text(title).lineLimit(1)
            Spacer()
            Text("\(count)")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .foregroundStyle(selected ? LumiColor.accent : LumiColor.textPrimary)
        .padding(.horizontal, LumiSpacing.small)
        .frame(height: LumiControlMetric.standardHeight)
        .background(selected ? LumiColor.accent.opacity(0.14) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private var trackBrowser: some View {
        VStack(spacing: 0) {
            trackHeader
            Divider()
            if state.condition == .importing {
                statePlaceholder(.importing)
            } else if state.condition == .error {
                statePlaceholder(.error)
            } else if visibleTracks.isEmpty {
                statePlaceholder(.empty)
            } else {
                if rendersInteractiveControls {
                    ScrollView {
                        trackListContent
                    }
                    .accessibilityIdentifier("lumi.library.trackList")
                    .focusable()
                    .onKeyPress(.upArrow) {
                        moveSelection(by: -1)
                        return .handled
                    }
                    .onKeyPress(.downArrow) {
                        moveSelection(by: 1)
                        return .handled
                    }
                } else {
                    trackListContent
                    Spacer(minLength: 0)
                }
            }
            Divider()
            pagination
        }
        .background(LumiColor.canvas)
    }

    private var trackHeader: some View {
        HStack {
            Text(String(format: localized("library.trackCount"), state.page.total))
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textSecondary)
            Spacer()
            if !state.query.search.isEmpty {
                Button(localized("library.clearSearch")) {
                    search = ""
                    submitQuery(search: "", offset: 0)
                }
                .buttonStyle(.borderless)
            }
        }
        .padding(.horizontal, LumiSpacing.large)
        .frame(height: LumiControlMetric.prominentHeight)
    }

    private func trackRow(_ track: LibraryTrack) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            colorSwatch(track.colorRGB)
            VStack(alignment: .leading, spacing: 2) {
                Text(track.title)
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textPrimary)
                    .lineLimit(1)
                Text(track.artist)
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                    .lineLimit(1)
            }
            Spacer()
            Text(formatBPM(track.bpmMilli))
                .font(LumiTypography.technical)
                .frame(width: 48, alignment: .trailing)
            Text(KeyNotationFormatter(notation: keyNotation).string(from: track.musicalKey))
                .font(LumiTypography.technical)
                .frame(width: 38, alignment: .trailing)
            Image(systemName: readinessIcon(track.readiness))
                .foregroundStyle(readinessColor(track.readiness))
                .accessibilityLabel(readinessName(track.readiness))
        }
        .padding(.vertical, LumiSpacing.xSmall)
    }

    private var trackListContent: some View {
        VStack(spacing: LumiSpacing.xSmall) {
            ForEach(visibleTracks) { track in
                trackListItem(track)
            }
        }
        .padding(LumiSpacing.small)
    }

    @ViewBuilder
    private func trackListItem(_ track: LibraryTrack) -> some View {
        let row = trackRow(track)
            .padding(.horizontal, LumiSpacing.small)
            .background(
                selectedTrackID == track.id
                    ? LumiColor.accent.opacity(0.14)
                    : Color.clear
            )
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))

        if rendersInteractiveControls {
            Button {
                selectedTrackID = track.id
            } label: {
                row
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("lumi.library.track.\(track.id)")
        } else {
            row
        }
    }

    private var inspector: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.large) {
            Text(localized("library.inspector"))
                .font(LumiTypography.sectionTitle)
            if let track = selectedTrack {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text(track.title)
                        .font(LumiTypography.cardTitle)
                    Text(track.artist)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                HStack(spacing: LumiSpacing.small) {
                    colorSwatch(track.colorRGB)
                    readinessBadge(track.readiness)
                }
                Divider()
                inspectorField(localized("library.bpm"), formatBPM(track.bpmMilli))
                inspectorField(
                    localized("library.key"),
                    KeyNotationFormatter(notation: keyNotation).string(from: track.musicalKey)
                )
                inspectorField(localized("library.duration"), formatDuration(track.durationMillis))
                inspectorField(localized("library.source"), state.source?.name ?? "—")
                inspectorField(localized("library.sourceTrackID"), track.sourceTrackID)
                inspectorField(localized("library.analysisRevision"), track.analysisRevision)
                inspectorField(
                    localized("library.timelineRevision"),
                    track.timelineRevision.map(String.init) ?? localized("library.notEdited")
                )
                Divider()
                capabilitySummary(track)
                Spacer()
                Button {
                    onOpenEditor(track.id)
                } label: {
                    Label(localized("library.openEditor"), systemImage: "waveform.path.ecg.rectangle")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("lumi.library.openEditor")
            } else {
                Text(localized("library.selectTrack"))
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
                Spacer()
            }
        }
        .padding(LumiSpacing.large)
        .background(LumiColor.surface)
        .accessibilityIdentifier("lumi.library.inspector")
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

    private func capabilitySummary(_ track: LibraryTrack) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            Text(localized("library.readiness"))
                .font(LumiTypography.sectionTitle)
            if track.missingCapabilities.isEmpty, track.warnings.isEmpty {
                Label(localized("library.analysisComplete"), systemImage: "checkmark.circle.fill")
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.success)
            } else {
                ForEach(track.missingCapabilities + track.warnings, id: \.self) { warning in
                    Label(warning, systemImage: "exclamationmark.triangle.fill")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.warning)
                }
            }
            if let capabilities = state.capabilities, !capabilities.missingNames.isEmpty {
                Text(capabilities.missingNames.joined(separator: ", "))
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.warning)
            }
        }
    }

    private func inspectorField(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label.uppercased())
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
            Text(value)
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textPrimary)
                .textSelection(.enabled)
        }
    }

    private func readinessBadge(_ readiness: LibraryReadiness) -> some View {
        Label(readinessName(readiness), systemImage: readinessIcon(readiness))
            .font(LumiTypography.caption)
            .foregroundStyle(readinessColor(readiness))
            .padding(.horizontal, LumiSpacing.small)
            .frame(height: LumiControlMetric.compactHeight)
            .background(readinessColor(readiness).opacity(0.12))
            .clipShape(Capsule())
    }

    private func colorSwatch(_ rgb: UInt32?) -> some View {
        let color = rgb.map {
            Color(
                red: Double(($0 >> 16) & 0xff) / 255,
                green: Double(($0 >> 8) & 0xff) / 255,
                blue: Double($0 & 0xff) / 255
            )
        } ?? LumiColor.surfaceElevated
        return RoundedRectangle(cornerRadius: LumiRadius.compact)
            .fill(color)
            .frame(width: 12, height: 32)
            .overlay { RoundedRectangle(cornerRadius: LumiRadius.compact).stroke(LumiColor.border) }
    }

    private var selectedTrack: LibraryTrack? {
        state.page.tracks.first { $0.id == selectedTrackID }
    }

    private var visibleTracks: [LibraryTrack] {
        LibraryWorkspacePresenter.visibleTracks(in: state, filter: readinessFilter)
    }

    private func selectPlaylist(_ id: UInt64?) {
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

    private func moveSelection(by distance: Int) {
        guard !visibleTracks.isEmpty else { return }
        let currentIndex = visibleTracks.firstIndex { $0.id == selectedTrackID } ?? 0
        let newIndex = min(max(0, currentIndex + distance), visibleTracks.count - 1)
        selectedTrackID = visibleTracks[newIndex].id
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
