import Testing
@testable import LumiDesignSystem

@Suite("Phrase color palette")
struct LumiPhraseColorPaletteTests {
    @Test("Configured colors override defaults and role aliases remain canonical")
    func configuredColorsAndAliases() {
        let palette = LumiPhraseColorPalette(roleColors: [
            "drop": 0x1234AB,
            "intro": 0x010203,
        ])

        #expect(palette.rgb(for: "drop") == 0x1234AB)
        #expect(palette.rgb(for: "intro-outro") == 0x010203)
        #expect(palette.rgb(for: "breakdown-3") == 0x7A47D4)
        #expect(palette.rgb(for: "unknown-role") == 0x33AD99)
    }

    @Test("Out-of-range input cannot override the safe role default")
    func rejectsOutOfRangeInput() {
        let palette = LumiPhraseColorPalette(roleColors: ["drop": 0xFF12_3456])
        #expect(palette.rgb(for: "drop") == 0xEB3342)
    }
}
