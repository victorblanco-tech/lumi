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
        .frame(minHeight: 250, maxHeight: .infinity)
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
        .task(id: search) {
            guard search != state.query.search else { return }
            do {
                try await Task.sleep(for: .milliseconds(180))
            } catch {
                return
            }
            guard !Task.isCancelled, search != state.query.search else { return }
            submitQuery(offset: 0)
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
                    .padding(.trailing, search.isEmpty ? 0 : 22)
                    .overlay(alignment: .trailing) {
                        if !search.isEmpty {
                            Button {
                                search = ""
                                submitQuery(search: "", offset: 0)
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("lumi.localPlayback.search.clear")
                        }
                    }
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
                LocalPlaybackTrackTable(
                    tracks: state.page.tracks,
                    keyNotation: keyNotation,
                    selection: $selectedTrackID
                )
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
                limit: state.query.limit,
                sortBy: state.query.sortBy,
                sortDirection: state.query.sortDirection
            )
        )
    }

    private func submitQuery(search querySearch: String? = nil, offset: UInt32) {
        onQuery(
            LibraryQueryRequest(
                search: querySearch ?? search,
                playlistID: selectedPlaylistID,
                offset: offset,
                limit: state.query.limit,
                sortBy: state.query.sortBy,
                sortDirection: state.query.sortDirection
            )
        )
    }
}

extension LocalPlaybackLibraryBrowserView: @MainActor Equatable {
    public static func == (
        lhs: LocalPlaybackLibraryBrowserView,
        rhs: LocalPlaybackLibraryBrowserView
    ) -> Bool {
        lhs.state == rhs.state
            && lhs.feedback == rhs.feedback
            && lhs.feedbackIsError == rhs.feedbackIsError
            && lhs.keyNotation == rhs.keyNotation
    }
}
