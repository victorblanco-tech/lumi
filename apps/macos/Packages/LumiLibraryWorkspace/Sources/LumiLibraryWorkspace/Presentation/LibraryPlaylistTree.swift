import Foundation
import LumiDesignSystem
import SwiftUI

struct LibraryPlaylistTreeRow: Identifiable, Equatable {
    enum ID: Hashable { case folder([String]), playlist(UInt64) }
    enum Kind: Equatable {
        case folder([String], playlistCount: Int)
        case playlist(LibraryPlaylist, title: String)
    }
    let id: ID
    let depth: Int
    let kind: Kind
}

func libraryPlaylistTreeRows(
    playlists: [LibraryPlaylist], expanded: Set<[String]>
) -> [LibraryPlaylistTreeRow] {
    var folders: [[String]: Set<[String]>] = [:]
    var leaves: [[String]: [(LibraryPlaylist, String)]] = [:]
    for playlist in playlists {
        // Unknown legacy paths stay literal until a native USB scan enriches
        // them. A slash in a playlist/folder name is NOT a hierarchy separator.
        let names = playlist.folderNames ?? []
        let prefix = names.isEmpty ? "" : names.joined(separator: "/") + "/"
        let valid = names.count <= 32 && !names.contains(where: \.isEmpty)
            && playlist.name.hasPrefix(prefix) && playlist.name.count > prefix.count
        let parents = valid ? names : []
        var parent: [String] = []
        for name in parents {
            let child = parent + [name]
            folders[parent, default: []].insert(child)
            parent = child
        }
        let title = valid ? String(playlist.name.dropFirst(prefix.count)) : playlist.name
        leaves[parent, default: []].append((playlist, title))
    }
    var rows: [LibraryPlaylistTreeRow] = []
    func append(_ parent: [String]) {
        for folder in (folders[parent] ?? []).sorted(by: {
            ($0.last ?? "").localizedStandardCompare($1.last ?? "") == .orderedAscending
        }) {
            let count = leaves.reduce(0) { total, pair in
                total + (pair.key.starts(with: folder) ? pair.value.count : 0)
            }
            rows.append(.init(id: .folder(folder), depth: parent.count,
                              kind: .folder(folder, playlistCount: count)))
            if expanded.contains(folder) { append(folder) }
        }
        for (playlist, title) in (leaves[parent] ?? []).sorted(by: {
            let order = $0.1.localizedStandardCompare($1.1)
            return order == .orderedSame ? $0.0.id < $1.0.id : order == .orderedAscending
        }) {
            rows.append(.init(id: .playlist(playlist.id), depth: parent.count,
                              kind: .playlist(playlist, title: title)))
        }
    }
    append([])
    return rows
}

struct LibraryPlaylistTreeView: View {
    let playlists: [LibraryPlaylist]
    let selectedPlaylistID: UInt64?
    let accessibilityPrefix: String
    var sourceLabels: [UInt64: String] = [:]
    let onSelect: (UInt64) -> Void
    @State private var expanded: Set<[String]> = []

    var body: some View {
        LazyVStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            ForEach(libraryPlaylistTreeRows(playlists: playlists, expanded: expanded)) { row in
                switch row.kind {
                case let .folder(path, count):
                    Button {
                        if expanded.contains(path) { expanded.remove(path) }
                        else { expanded.insert(path) }
                    } label: {
                        HStack(spacing: LumiSpacing.small) {
                            Image(systemName: expanded.contains(path) ? "chevron.down" : "chevron.right")
                                .font(.caption).frame(width: 10)
                            Image(systemName: "folder").foregroundStyle(LumiColor.accent)
                            Text(path.last ?? "").lineLimit(1)
                            Spacer(minLength: 2)
                            Text("\(count)").foregroundStyle(LumiColor.textSecondary)
                        }
                        .font(LumiTypography.metadata)
                        .padding(.leading, CGFloat(min(row.depth, 8)) * 12 + 8)
                        .padding(.trailing, 8)
                        .frame(minHeight: LumiControlMetric.standardHeight)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(path.joined(separator: "/"))
                    .accessibilityLabel("\(path.last ?? ""), \(count) playlists")
                    .accessibilityIdentifier("\(accessibilityPrefix).folder.\(path.joined(separator: "/"))")
                case let .playlist(playlist, title):
                    Button { onSelect(playlist.id) } label: {
                        HStack(spacing: LumiSpacing.small) {
                            Image(systemName: "music.note.list")
                            VStack(alignment: .leading, spacing: 1) {
                                Text(title).lineLimit(1)
                                if let source = sourceLabels[playlist.id] {
                                    Text(source).font(LumiTypography.technical)
                                        .foregroundStyle(LumiColor.textSecondary).lineLimit(1)
                                }
                            }
                            Spacer(minLength: 2)
                            Text("\(playlist.trackCount)").font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        .font(LumiTypography.metadata)
                        .foregroundStyle(selectedPlaylistID == playlist.id ? LumiColor.accent : LumiColor.textPrimary)
                        .padding(.leading, CGFloat(min(row.depth, 8)) * 12 + 8)
                        .padding(.trailing, 8)
                        .frame(minHeight: sourceLabels[playlist.id] == nil ? LumiControlMetric.standardHeight : 44)
                        .contentShape(Rectangle())
                        .background(selectedPlaylistID == playlist.id ? LumiColor.accent.opacity(0.14) : .clear)
                        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                    }
                    .buttonStyle(.plain)
                    .help(playlist.name)
                    .accessibilityIdentifier("\(accessibilityPrefix).playlist.\(playlist.id)")
                }
            }
        }
        .onAppear { revealSelection() }
        .onChange(of: selectedPlaylistID) { _, _ in revealSelection() }
        .onChange(of: playlists) { _, _ in revealSelection() }
    }

    private func revealSelection() {
        guard let names = playlists.first(where: { $0.id == selectedPlaylistID })?.folderNames,
              !names.isEmpty, names.count <= 32 else { return }
        for depth in 1...names.count {
            expanded.insert(Array(names.prefix(depth)))
        }
    }
}
