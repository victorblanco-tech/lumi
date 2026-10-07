import Testing
@testable import LumiLibraryWorkspace

@Suite("Synced library playlist tree")
struct LibraryPlaylistTreeTests {
    @Test("Folders are collapsed until expanded; leaf IDs retain query identity")
    func nesting() {
        let playlists = [
            playlist(81, ["Sets", "Trancendence", "Trancendence 2"], "Part 1"),
            playlist(82, ["Sets", "Trancendence", "Trancendence 2"], "Part 2"),
            playlist(80, ["Genre 5 Stars", "140+"], "MainStage 140+")
        ]
        let collapsed = libraryPlaylistTreeRows(playlists: playlists, expanded: [])
        #expect(collapsed.map(\.id) == [.folder(["Genre 5 Stars"]), .folder(["Sets"])])
        let open = libraryPlaylistTreeRows(playlists: playlists, expanded: [
            ["Sets"], ["Sets", "Trancendence"], ["Sets", "Trancendence", "Trancendence 2"]
        ])
        #expect(open.map(\.id) == [.folder(["Genre 5 Stars"]), .folder(["Sets"]),
                                  .folder(["Sets", "Trancendence"]),
                                  .folder(["Sets", "Trancendence", "Trancendence 2"]),
                                  .playlist(81), .playlist(82)])
        #expect(open.last?.depth == 3)
        #expect(libraryPlaylistTreeRows(playlists: playlists, expanded: []).count == 2)
    }

    @Test("Slash characters in native folder and playlist names stay literal")
    func literalSlashes() {
        let names = ["Genre 5 Stars", "Tech/Trance"]
        let rows = libraryPlaylistTreeRows(
            playlists: [playlist(1, names, "Psy/Tech Trance 135+")],
            expanded: [["Genre 5 Stars"], names]
        )
        #expect(rows.map(\.id) == [.folder(["Genre 5 Stars"]), .folder(names), .playlist(1)])
        #expect(rows.last?.kind == .playlist(playlist(1, names, "Psy/Tech Trance 135+"), title: "Psy/Tech Trance 135+"))
    }

    @Test("Unknown legacy paths and invalid metadata never create guessed folders")
    func legacy() {
        let unknown = LibraryPlaylist(id: 1, sourcePlaylistID: "onelibrary:1",
                                      name: "Psy/Tech Trance", trackCount: 2)
        let invalid = LibraryPlaylist(id: 2, sourcePlaylistID: "onelibrary:2",
                                      name: "Other/Tracklist", trackCount: 1, folderNames: ["Wrong"])
        let rows = libraryPlaylistTreeRows(playlists: [unknown, invalid], expanded: [])
        #expect(rows.count == 2)
        #expect(rows.allSatisfy { $0.depth == 0 })
    }

    @Test("Equal leaf labels in different parents and root playlists stay distinct")
    func identities() {
        let rows = libraryPlaylistTreeRows(playlists: [playlist(1, ["A"], "Closing"),
            playlist(2, ["B"], "Closing"), playlist(3, [], "Closing")], expanded: [["A"], ["B"]])
        #expect(Set(rows.map(\.id)).count == 5)
        #expect(rows.map(\.id).contains(.playlist(3)))
    }

    private func playlist(_ id: UInt64, _ folders: [String], _ name: String) -> LibraryPlaylist {
        LibraryPlaylist(id: id, sourcePlaylistID: "onelibrary:\(id)",
                        name: (folders + [name]).joined(separator: "/"),
                        trackCount: 2, folderNames: folders)
    }
}
