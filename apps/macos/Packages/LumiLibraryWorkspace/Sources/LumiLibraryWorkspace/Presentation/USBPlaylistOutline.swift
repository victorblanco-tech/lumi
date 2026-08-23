import Foundation

struct USBPlaylistOutlineRow: Identifiable, Equatable, Sendable {
    enum Kind: Equatable, Sendable {
        case folder(path: String, name: String, playlistCount: Int, trackCount: UInt64)
        case playlist(RekordboxDevicePlaylistState)
    }

    let id: String
    let depth: Int
    let kind: Kind
}

func usbPlaylistOutlineRows(
    playlists: [RekordboxDevicePlaylistState],
    expandedFolderPaths: Set<String>,
    search: String
) -> [USBPlaylistOutlineRow] {
    let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
    let visiblePlaylists = query.isEmpty
        ? playlists
        : playlists.filter {
            $0.path.localizedCaseInsensitiveContains(query)
                || $0.name.localizedCaseInsensitiveContains(query)
        }
    var foldersByParent: [String: Set<String>] = [:]
    var playlistsByParent: [String: [RekordboxDevicePlaylistState]] = [:]

    for playlist in visiblePlaylists {
        var parent = ""
        for component in playlist.folderNames {
            let path = parent.isEmpty ? component : "\(parent)/\(component)"
            foldersByParent[parent, default: []].insert(path)
            parent = path
        }
        playlistsByParent[parent, default: []].append(playlist)
    }

    func descendantCounts(for folderPath: String) -> (playlists: Int, tracks: UInt64) {
        let prefix = folderPath + "/"
        let descendants = visiblePlaylists.filter {
            $0.path.hasPrefix(prefix)
        }
        return (
            descendants.count,
            descendants.reduce(0) { $0 + $1.trackCount }
        )
    }

    var rows: [USBPlaylistOutlineRow] = []
    func appendChildren(parent: String, depth: Int) {
        let folders = (foldersByParent[parent] ?? []).sorted {
            $0.localizedCaseInsensitiveCompare($1) == .orderedAscending
        }
        for path in folders {
            let counts = descendantCounts(for: path)
            rows.append(
                USBPlaylistOutlineRow(
                    id: "folder:\(path)",
                    depth: depth,
                    kind: .folder(
                        path: path,
                        name: path.split(separator: "/").last.map(String.init) ?? path,
                        playlistCount: counts.playlists,
                        trackCount: counts.tracks
                    )
                )
            )
            if !query.isEmpty || expandedFolderPaths.contains(path) {
                appendChildren(parent: path, depth: depth + 1)
            }
        }
        let leafPlaylists = (playlistsByParent[parent] ?? []).sorted {
            if $0.name == $1.name { return $0.id < $1.id }
            return $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending
        }
        rows.append(contentsOf: leafPlaylists.map {
            USBPlaylistOutlineRow(
                id: "playlist:\($0.id)",
                depth: depth,
                kind: .playlist($0)
            )
        })
    }
    appendChildren(parent: "", depth: 0)
    return rows
}
