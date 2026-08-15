import LumiProtocol
import Testing
@testable import LumiEngineClient

@Suite("Engine command encoding")
struct EngineCommandTests {
    @Test("Ableton Link enablement has an explicit boolean command")
    func abletonLinkEnablementPayload() {
        let payload = EngineCommand.setAbletonLinkEnabled(true).payload()
        #expect(payload["kind"] == .string("setAbletonLinkEnabled"))
        #expect(payload["enabled"] == .boolean(true))
    }

    @Test("Ableton Link helper testing has an explicit command")
    func abletonLinkRecoveryPayload() {
        let payload = EngineCommand.testAbletonLinkHelper.payload()

        #expect(payload["kind"] == .string("testAbletonLinkHelper"))
    }

    @Test("Lighting timing offset preserves signed milliseconds")
    func lightingTimingOffsetPayload() {
        let payload = EngineCommand.setOutputTimingOffset(millis: -35).payload()

        #expect(payload["kind"] == .string("setOutputTimingOffset"))
        #expect(payload["millis"] == .number(-35))
    }

    @Test("Rekordbox preview carries the exact selected playlist paths")
    func rekordboxPreviewPayload() {
        let payload = EngineCommand.previewRekordboxXMLSync(
            folder: "/Music/Rekordbox XML",
            followedPaths: ["Sets/Beach Set", "Genre 5 Stars"],
            includeFutureChildPlaylists: true
        ).payload()

        #expect(payload["kind"] == .string("previewRekordboxXmlSync"))
        #expect(payload["folder"] == .string("/Music/Rekordbox XML"))
        #expect(
            payload["followedPaths"] == .array([
                .string("Sets/Beach Set"),
                .string("Genre 5 Stars")
            ])
        )
        #expect(payload["includeFutureChildPlaylists"] == .boolean(true))
    }

    @Test("Rekordbox analysis import is pinned to the reviewed XML fingerprint")
    func rekordboxAnalysisImportPayload() {
        let payload = EngineCommand.importRekordboxAnalysis(
            folder: "/Music/Rekordbox XML",
            followedPaths: ["Sets/Beach Set"],
            includeFutureChildPlaylists: true,
            expectedContentSHA256: "abc123"
        ).payload()

        #expect(payload["kind"] == .string("importRekordboxAnalysis"))
        #expect(payload["expectedContentSha256"] == .string("abc123"))
        #expect(payload["followedPaths"] == .array([.string("Sets/Beach Set")]))
    }

    @Test("Rekordbox device sync carries the selected mounted root exactly")
    func rekordboxDeviceSyncPayload() {
        let payload = EngineCommand.syncRekordboxDevice(
            root: "/Volumes/DJ USB",
            sourceID: "usb-volume:1234",
            playlistIDs: [17, 23]
        ).payload()

        #expect(payload["kind"] == .string("syncRekordboxDevice"))
        #expect(payload["root"] == .string("/Volumes/DJ USB"))
        #expect(payload["sourceId"] == .string("usb-volume:1234"))
        #expect(payload["playlistIds"] == .array([.number(17), .number(23)]))
    }

    @Test("Library reset preview carries the exact preservation selection")
    func libraryResetPreviewPayload() {
        let payload = EngineCommand.previewLibraryReset(
            preserveTrackIDs: [202, 317]
        ).payload()

        #expect(payload["kind"] == .string("previewLibraryReset"))
        #expect(payload["preserveTrackIds"] == .array([.number(202), .number(317)]))
    }

    @Test("Library reset apply is pinned to its reviewed token and mandatory backup")
    func libraryResetApplyPayload() {
        let payload = EngineCommand.applyLibraryReset(
            expectedResetToken: "reset-393-14-202",
            backupDatabasePath: "/Backups/Lumi-pre-reset.lumibackup/library.sqlite"
        ).payload()

        #expect(payload["kind"] == .string("applyLibraryReset"))
        #expect(payload["expectedResetToken"] == .string("reset-393-14-202"))
        #expect(
            payload["backupDatabasePath"]
                == .string("/Backups/Lumi-pre-reset.lumibackup/library.sqlite")
        )
    }

    @Test("Backup and restore stay inside the authenticated engine protocol")
    func engineOwnedBackupPayloads() {
        let backup = EngineCommand.createLibraryBackup(
            destination: "/Backups/Lumi.lumibackup/library.sqlite"
        ).payload()
        #expect(backup["kind"] == .string("createLibraryBackup"))
        #expect(
            backup["destination"] == .string("/Backups/Lumi.lumibackup/library.sqlite")
        )

        let restore = EngineCommand.restoreLibraryBackup(
            source: "/Backups/Lumi.lumibackup/library.sqlite",
            rollback: "/Backups/.rollback/library.sqlite"
        ).payload()
        #expect(restore["kind"] == .string("restoreLibraryBackup"))
        #expect(restore["source"] == .string("/Backups/Lumi.lumibackup/library.sqlite"))
        #expect(restore["rollback"] == .string("/Backups/.rollback/library.sqlite"))
    }
}
