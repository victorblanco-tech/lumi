import Testing
@testable import LumiDesignSystem

@Suite("Track color palette")
struct LumiTrackColorPaletteTests {
    @Test("The fixed Rekordbox RGB palette keeps stable English names")
    func fixedPaletteNames() {
        #expect(LumiTrackColorPalette.name(for: 0xff_33_cc) == "Pink")
        #expect(LumiTrackColorPalette.name(for: 0xff_33_33) == "Red")
        #expect(LumiTrackColorPalette.name(for: 0xff_8c_1a) == "Orange")
        #expect(LumiTrackColorPalette.name(for: 0xff_d6_00) == "Yellow")
        #expect(LumiTrackColorPalette.name(for: 0x32_d7_4b) == "Green")
        #expect(LumiTrackColorPalette.name(for: 0x32_d7_d5) == "Aqua")
        #expect(LumiTrackColorPalette.name(for: 0x32_80_ff) == "Blue")
        #expect(LumiTrackColorPalette.name(for: 0xaf_52_de) == "Purple")
    }

    @Test("Unknown and absent colors remain explicit")
    func fallbackLabels() {
        #expect(LumiTrackColorPalette.name(for: 0x12_34_56) == "#123456")
        #expect(LumiTrackColorPalette.accessibilityLabel(for: nil) == "No track color")
        #expect(
            LumiTrackColorPalette.accessibilityLabel(for: 0x32_80_ff)
                == "Track color: Blue"
        )
    }
}
