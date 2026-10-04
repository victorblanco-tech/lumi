import Testing
@testable import LumiDesignSystem

@Test func usbUsesNativeMediaPaletteNotTrackPalette() {
    #expect(LumiPlayerUSBBadge.colorRGB(for: 1) == 0xff3399)
    #expect(LumiPlayerUSBBadge.colorRGB(for: 7) == 0x3399ff)
    #expect(LumiPlayerUSBBadge.colorRGB(for: 0) == nil)
    #expect(LumiPlayerUSBBadge.colorRGB(for: 99) == nil)
}

@Test func uncertainUSBDoesNotDisplayStaleSourceName() {
    #expect(LumiPlayerUSBBadge.label(state: "trusted", sourceName: "GRAY") == "USB · GRAY")
    for state in ["unknown", "conflict", "unavailable", "resolving"] {
        #expect(!LumiPlayerUSBBadge.label(state: state, sourceName: "STALE").contains("STALE"))
    }
}
