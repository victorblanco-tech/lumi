import LumiProtocol
import Testing
@testable import LumiEngineClient

@Suite("Engine command encoding")
struct EngineCommandTests {
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
        let payload = EngineCommand.syncRekordboxDevice(root: "/Volumes/DJ USB").payload()

        #expect(payload["kind"] == .string("syncRekordboxDevice"))
        #expect(payload["root"] == .string("/Volumes/DJ USB"))
    }
}
