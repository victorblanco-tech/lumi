import XCTest
@testable import LumiLibraryWorkspace

final class USBOperationFeedbackTests: XCTestCase {
    func testBulkReviewPreservesExactRevisionsAndOrder() {
        var queue = USBReviewQueue()
        let requests = [UInt32(12), 13].map {
            USBConflictResolutionRequest(root: "/Volumes/GRAY", sourceID: "gray",
                deviceTrackID: $0, expectedIncomingRevision: "usb-\($0)",
                expectedActiveRevision: "lumi-\($0)", choice: .keepLumi)
        }
        queue.start(requests)
        XCTAssertEqual(queue.count, 2)
        let first = queue.next()
        XCTAssertEqual(first?.deviceTrackID, 12)
        XCTAssertEqual(first?.expectedIncomingRevision, "usb-12")
        XCTAssertEqual(first?.expectedActiveRevision, "lumi-12")
        XCTAssertEqual(first?.choice, .keepLumi)
        XCTAssertEqual(queue.next()?.deviceTrackID, 13)
        XCTAssertNil(queue.next())
    }

    func testBulkReviewFailureStopsRemainingRequests() {
        var queue = USBReviewQueue()
        queue.start([USBConflictResolutionRequest(root: "/Volumes/GRAY", sourceID: "gray",
            deviceTrackID: 12, expectedIncomingRevision: "usb", expectedActiveRevision: "lumi", choice: .useUSB)])
        queue.stop()
        XCTAssertTrue(queue.isEmpty)
        XCTAssertNil(queue.next())
    }

    func testIdentityFailureIsVisibleOnlyOnItsSource() {
        let state = USBSourceOperationState(
            phase: .failed, title: "USB sync failed",
            detail: "USB identity conflicts with another physical source",
            sourceID: "gray"
        )
        XCTAssertEqual(state.failedMessage(for: "gray"),
                       "Sync stopped: USB identity needs confirmation. No tracks were imported.")
        XCTAssertNil(state.failedMessage(for: "chrm"))
    }

    func testOtherFailuresKeepTheirActionableExplanation() {
        let state = USBSourceOperationState(
            phase: .failed, title: "USB sync failed",
            detail: "Refresh this USB's playlists before synchronizing.", sourceID: "gray"
        )
        XCTAssertEqual(state.failedMessage(for: "gray"), state.detail)
        XCTAssertNil(USBSourceOperationState.idle.failedMessage(for: "gray"))
    }
}
