import Foundation
import Testing
@testable import LumiLibraryWorkspace

@Suite("Existing USB authorization")
struct USBBookmarkResolverTests {
    private let root = URL(fileURLWithPath: "/Volumes/TEST USB", isDirectory: true)

    @Test("A current grant is reused without renewal")
    func currentGrant() throws {
        let result = USBBookmarkResolver.validateAndRenew(resolved: root, expectedRoot: root, stale: false) {
            Issue.record("A current grant must not be renewed")
            return nil
        }
        #expect(result?.url == root)
        #expect(result?.renewedBookmark == nil)
    }

    @Test("A stale but still authorized grant renews for exactly the same root")
    func staleGrant() {
        let bytes = Data("renewed-grant".utf8)
        var renewals = 0
        let result = USBBookmarkResolver.validateAndRenew(resolved: root, expectedRoot: root, stale: true) {
            renewals += 1
            return bytes
        }
        #expect(renewals == 1)
        #expect(result?.url == root)
        #expect(result?.renewedBookmark == bytes)
    }

    @Test("A different volume or broader root never reuses a grant", arguments: ["/Volumes/OTHER USB", "/Volumes"])
    func mismatchedGrant(path: String) {
        let result = USBBookmarkResolver.validateAndRenew(
            resolved: URL(fileURLWithPath: path), expectedRoot: root, stale: true
        ) {
            Issue.record("A different root must not be renewed")
            return Data()
        }
        #expect(result == nil)
    }

    @Test("Unavailable access cannot silently become an authorized path")
    func unavailableGrant() {
        let result = USBBookmarkResolver.validateAndRenew(resolved: root, expectedRoot: root, stale: true) { nil }
        #expect(result == nil)
    }
}
