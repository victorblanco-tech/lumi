import Foundation
import Testing
@testable import LumiLibraryWorkspace

@Suite("Rekordbox XML source discovery")
struct RekordboxXMLDiscoveryTests {
    @Test("A supported export exposes the playlist tree without the Rekordbox ROOT wrapper")
    func scansPlaylistTreeReadOnly() throws {
        let url = try #require(Bundle.module.url(
            forResource: "rekordbox-playlists",
            withExtension: "xml"
        ))
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        let export = RekordboxXMLExport(
            path: url.path,
            fileName: url.lastPathComponent,
            modifiedAt: attributes[.modificationDate] as? Date ?? .distantPast,
            sizeBytes: (attributes[.size] as? NSNumber)?.uint64Value ?? 0
        )

        let result = try RekordboxXMLDiscoveryService().scan(export)

        #expect(result.productName == "rekordbox")
        #expect(result.productVersion == "7.2.14")
        #expect(result.collectionEntries == 3)
        #expect(result.folderCount == 1)
        #expect(result.playlistCount == 3)
        #expect(result.roots.map(\.name) == ["Sets", "Preparation"])
        #expect(result.roots[0].path == "Sets")
        #expect(result.roots[0].children[0].path == "Sets/Beach Set")
        #expect(result.roots[0].children[0].trackCount == 2)
    }

    @Test("Folder discovery ignores non-XML files and orders newest exports first")
    func discoversNewestExportFirst() throws {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("lumi-rb-discovery-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }

        let older = folder.appendingPathComponent("older.xml")
        let newest = folder.appendingPathComponent("newest.XML")
        let ignored = folder.appendingPathComponent("notes.txt")
        try Data("<xml/>".utf8).write(to: older)
        try Data("<xml/>".utf8).write(to: newest)
        try Data("ignore".utf8).write(to: ignored)
        try FileManager.default.setAttributes(
            [.modificationDate: Date(timeIntervalSince1970: 1)],
            ofItemAtPath: older.path
        )
        try FileManager.default.setAttributes(
            [.modificationDate: Date(timeIntervalSince1970: 2)],
            ofItemAtPath: newest.path
        )

        let exports = try RekordboxXMLDiscoveryService().exports(in: folder)

        #expect(exports.map(\.fileName) == ["newest.XML", "older.xml"])
    }

    @Test("A playlist with a mismatched declared entry count fails closed")
    func rejectsMismatchedPlaylistCount() throws {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("lumi-rb-invalid-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let url = folder.appendingPathComponent("invalid.xml")
        let xml = """
        <DJ_PLAYLISTS Version="1.0.0">
          <PRODUCT Name="rekordbox" Version="7"/>
          <COLLECTION Entries="1"/>
          <PLAYLISTS><NODE Type="1" Name="Set" Entries="2"><TRACK Key="1"/></NODE></PLAYLISTS>
        </DJ_PLAYLISTS>
        """
        try Data(xml.utf8).write(to: url)
        let export = RekordboxXMLExport(
            path: url.path,
            fileName: url.lastPathComponent,
            modifiedAt: .now,
            sizeBytes: UInt64(Data(xml.utf8).count)
        )

        #expect(throws: RekordboxXMLDiscoveryError.invalidNode) {
            try RekordboxXMLDiscoveryService().scan(export)
        }
    }
}
