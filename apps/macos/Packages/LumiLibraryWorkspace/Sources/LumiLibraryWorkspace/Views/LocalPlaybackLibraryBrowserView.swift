import LumiDesignSystem
import SwiftUI

/// Compact Library browser embedded in Live when Local Playback owns the decks.
/// It deliberately reuses the authoritative Library page and the same load
/// command as the full Library workspace instead of introducing a second model.
public struct LocalPlaybackLibraryBrowserView: View {
    private let state: LibraryWorkspaceState
    private let feedback: String?
    private let feedbackIsError: Bool
    private let onQuery: @MainActor (LibraryQueryRequest) -> Void
    private let onLoadOnLocalDeck: @MainActor (LibraryDeckLoadRequest) -> Void
    @Binding private var keyNotation: KeyNotationPreference
    @State private var search: String
    @State private var selectedPlaylistID: UInt64?
    @State private var selectedTrackID: UInt64?

    public init(
        state: LibraryWorkspaceState,
        keyNotation: Binding<KeyNotationPreference>,
        feedback: String? = nil,
        feedbackIsError: Bool = false,
        onQuery: @escaping @MainActor (LibraryQueryRequest) -> Void = { _ in },
        onLoadOnLocalDeck: @escaping @MainActor (LibraryDeckLoadRequest) -> Void = { _ in }
    ) {
        self.state = state
        self.feedback = feedback
        self.feedbackIsError = feedbackIsError
        self.onQuery = onQuery
        self.onLoadOnLocalDeck = onLoadOnLocalDeck
        _keyNotation = keyNotation
        _search = State(initialValue: state.query.search)
        _selectedPlaylistID = State(initialValue: state.query.playlistID)
        _selectedTrackID = State(initialValue: state.page.tracks.first?.id)
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            HStack(spacing: 0) {
                playlistBrowser
                    .frame(width: 220)
                Divider()
                trackBrowser
            }
        }
        .frame(minHeight: 250, idealHeight: 290, maxHeight: 340)
        .background(LumiColor.canvas)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.panel)
                .stroke(LumiColor.border, lineWidth: 1)
        }
        .accessibilityIdentifier("lumi.localPlayback.library")
        .onChange(of: state.query.search) { _, value in search = value }
        .onChange(of: state.query.playlistID) { _, value in selectedPlaylistID = value }
        .onChange(of: state.page.tracks) { _, tracks in
            if !tracks.contains(where: { $0.id == selectedTrackID }) {
                selectedTrackID = tracks.first?.id
            }
        }
    }

    private var header: some View {
        HStack(spacing: LumiSpacing.medium) {
            Label("Local Playback Library", systemImage: "music.note.list")
                .font(LumiTypography.sectionTitle)
            Text("Select a track, then load Deck A or Deck B")
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textSecondary)
            Spacer()
            if let feedback {
                Label(
                    feedback,
                    systemImage: feedbackIsError ? "exclamationmark.triangle.fill" : "checkmark.circle.fill"
                )
                .font(LumiTypography.metadata)
                .foregroundStyle(feedbackIsError ? LumiColor.warning : LumiColor.success)
                .lineLimit(1)
                .help(feedback)
                .accessibilityIdentifier("lumi.localPlayback.feedback")
            }
            loadButton(deckID: 1, name: "Deck A")
            loadButton(deckID: 2, name: "Deck B")
        }
        .padding(.horizontal, LumiSpacing.large)
        .frame(height: 48)
        .background(LumiColor.surface)
    }

    private var playlistBrowser: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.small) {
            Button {
                selectPlaylist(nil)
            } label: {
                playlistLabel(
                    "Collection",
                    count: state.collectionTotal,
                    selected: selectedPlaylistID == nil,
                    systemImage: "music.note.list"
                )
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("lumi.localPlayback.collection")

            Text("PLAYLISTS")
                .font(LumiTypography.caption.weight(.bold))
                .foregroundStyle(LumiColor.textSecondary)
                .padding(.top, LumiSpacing.xSmall)

            ScrollView(.vertical) {
                LazyVStack(spacing: LumiSpacing.xSmall) {
                    ForEach(state.playlists) { playlist in
                        Button {
                            selectPlaylist(playlist.id)
                        } label: {
                            playlistLabel(
                                playlist.name,
                                count: playlist.trackCount,
                                selected: selectedPlaylistID == playlist.id,
                                systemImage: "music.note"
                            )
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("lumi.localPlayback.playlist.\(playlist.id)")
                    }
                }
            }
            .scrollIndicators(.automatic)
            .accessibilityIdentifier("lumi.localPlayback.playlists")
        }
        .padding(LumiSpacing.medium)
        .background(LumiColor.surface)
    }

    private func playlistLabel(
        _ title: String,
        count: UInt64,
        selected: Bool,
        systemImage: String
    ) -> some View {
        HStack(spacing: LumiSpacing.small) {
            Image(systemName: systemImage)
                .frame(width: 16)
            Text(title).lineLimit(1)
            Spacer()
            Text("\(count)")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .font(LumiTypography.metadata.weight(selected ? .semibold : .regular))
        .foregroundStyle(selected ? LumiColor.accent : LumiColor.textPrimary)
        .padding(.horizontal, LumiSpacing.small)
        .frame(maxWidth: .infinity, minHeight: LumiControlMetric.standardHeight)
        .contentShape(Rectangle())
        .background(selected ? LumiColor.accent.opacity(0.14) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private var trackBrowser: some View {
        VStack(spacing: 0) {
            HStack(spacing: LumiSpacing.medium) {
                Text("\(state.page.total) tracks")
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
                Spacer()
                TextField("Search title, artist, or source ID", text: $search)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 280)
                    .onSubmit { submitQuery(offset: 0) }
                    .accessibilityIdentifier("lumi.localPlayback.search")
            }
            .padding(.horizontal, LumiSpacing.medium)
            .frame(height: 42)

            if state.condition == .error {
                ContentUnavailableView(
                    "Library unavailable",
                    systemImage: "exclamationmark.triangle.fill",
                    description: Text(state.diagnostic ?? "The local Library could not be loaded.")
                )
            } else if state.page.tracks.isEmpty {
                ContentUnavailableView(
                    "No tracks",
                    systemImage: "music.note",
                    description: Text("Choose another playlist or search term.")
                )
            } else {
                Table(state.page.tracks, selection: $selectedTrackID) {
                    TableColumn("Track Title") { track in
                        HStack(spacing: LumiSpacing.small) {
                            colorSwatch(track.colorRGB)
                            Text(track.title)
                                .font(LumiTypography.body.weight(.semibold))
                                .lineLimit(1)
                        }
                    }
                    .width(min: 180, ideal: 300)

                    TableColumn("Artist") { track in
                        Text(track.artist).lineLimit(1)
                    }
                    .width(min: 120, ideal: 200)

                    TableColumn("BPM") { track in
                        Text(String(format: "%.1f", Double(track.bpmMilli) / 1_000))
                            .font(LumiTypography.technical)
                    }
                    .width(min: 55, ideal: 64, max: 80)

                    TableColumn("Key") { track in
                        Text(KeyNotationFormatter(notation: keyNotation).string(from: track.musicalKey))
                            .font(LumiTypography.technical)
                    }
                    .width(min: 44, ideal: 54, max: 70)

                    TableColumn("Lumi") { track in
                        Text(track.timelineRevision.map { "R\($0)" } ?? "—")
                            .font(LumiTypography.technical)
                            .foregroundStyle(track.timelineRevision == nil ? LumiColor.warning : LumiColor.success)
                    }
                    .width(min: 48, ideal: 58, max: 72)
                }
                .tableStyle(.inset(alternatesRowBackgrounds: true))
                .scrollContentBackground(.hidden)
                .background(LumiColor.canvas)
                .accessibilityIdentifier("lumi.localPlayback.trackTable")
            }

            pagination
        }
    }

    private var pagination: some View {
        HStack {
            Button("Previous") {
                submitQuery(offset: state.page.offset - UInt32(state.query.limit))
            }
            .disabled(state.page.offset == 0)
            Spacer()
            Text("Page \(pageNumber) of \(pageCount)")
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
            Spacer()
            Button("Next") {
                submitQuery(offset: state.page.offset + UInt32(state.query.limit))
            }
            .disabled(UInt64(state.page.offset) + UInt64(state.query.limit) >= state.page.total)
        }
        .padding(.horizontal, LumiSpacing.medium)
        .frame(height: 38)
        .background(LumiColor.surface)
    }

    private func loadButton(deckID: UInt64, name: String) -> some View {
        Button {
            guard let track = selectedTrack,
                  let revision = track.timelineRevision else { return }
            onLoadOnLocalDeck(
                LibraryDeckLoadRequest(
                    trackID: track.id,
                    deckID: deckID,
                    expectedTimelineRevision: revision
                )
            )
        } label: {
            Label("Load \(name)", systemImage: "arrow.down.to.line.compact")
        }
        .buttonStyle(.bordered)
        .disabled(selectedTrack?.timelineRevision == nil)
        .help(selectedTrack?.timelineRevision == nil ? "This track has no ready Lumi timeline." : "Load the selected track on \(name)")
        .accessibilityIdentifier("lumi.localPlayback.loadDeck\(deckID)")
    }

    private func colorSwatch(_ rgb: UInt32?) -> some View {
        RoundedRectangle(cornerRadius: 2)
            .fill(rgb.map {
                Color(
                    red: Double(($0 >> 16) & 0xFF) / 255,
                    green: Double(($0 >> 8) & 0xFF) / 255,
                    blue: Double($0 & 0xFF) / 255
                )
            } ?? LumiColor.surfaceElevated)
            .frame(width: 8, height: 18)
    }

    private var selectedTrack: LibraryTrack? {
        state.page.tracks.first { $0.id == selectedTrackID }
    }

    private var pageNumber: UInt64 {
        UInt64(state.page.offset) / UInt64(max(1, state.query.limit)) + 1
    }

    private var pageCount: UInt64 {
        max(1, (state.page.total + UInt64(max(1, state.query.limit)) - 1) / UInt64(max(1, state.query.limit)))
    }

    private func selectPlaylist(_ id: UInt64?) {
        selectedPlaylistID = id
        onQuery(
            LibraryQueryRequest(
                search: search,
                playlistID: id,
                offset: 0,
                limit: state.query.limit
            )
        )
    }

    private func submitQuery(offset: UInt32) {
        onQuery(
            LibraryQueryRequest(
                search: search,
                playlistID: selectedPlaylistID,
                offset: offset,
                limit: state.query.limit
            )
        )
    }
}
