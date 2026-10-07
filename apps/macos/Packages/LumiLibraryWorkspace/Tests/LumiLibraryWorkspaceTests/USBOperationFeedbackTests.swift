import XCTest
@testable import LumiLibraryWorkspace

final class USBOperationFeedbackTests: XCTestCase {
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
