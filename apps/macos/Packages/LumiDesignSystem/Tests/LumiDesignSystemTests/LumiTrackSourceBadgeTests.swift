import Testing
@testable import LumiDesignSystem

@Test func trackSourceDistinguishesLocalAndLinkedMedia() {
    #expect(LumiTrackSourceBadge.label(state: "trusted", sourceName: "CHRM",
        sourcePlayer: 1, loadedPlayer: 2, slot: "USB_SLOT")
        == "Source · CHRM · via Player 1 / LINK")
    #expect(LumiTrackSourceBadge.label(state: "trusted", sourceName: "GRAY",
        sourcePlayer: 2, loadedPlayer: 2, slot: "USB_SLOT") == "Source · GRAY · local")
}

@Test func unverifiedTrackSourceNeverBorrowsMountedUSBName() {
    for state in ["unknown", "unavailable", "conflict", "resolving"] {
        let label = LumiTrackSourceBadge.label(state: state, sourceName: "STALE",
            sourcePlayer: 1, loadedPlayer: 2, slot: "USB_SLOT")
        #expect(!label.contains("STALE"))
        #expect(label.contains("via Player 1 / LINK"))
    }
    #expect(LumiTrackSourceBadge.label(state: "unavailable", sourceName: nil,
        sourcePlayer: nil, loadedPlayer: 2, slot: nil) == "Source · Not identified")
}
