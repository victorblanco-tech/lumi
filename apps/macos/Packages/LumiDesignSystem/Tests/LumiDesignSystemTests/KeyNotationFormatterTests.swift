import Testing
@testable import LumiDesignSystem

private let expectedCamelotMajor = [
    "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"
]
private let expectedCamelotMinor = [
    "5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"
]

@Test("Camelot formatting covers every major and minor pitch class")
func formatsAllCamelotKeys() {
    let formatter = KeyNotationFormatter(notation: .camelot)

    for pitchClass in PitchClass.allCases {
        let major = MusicalKey(pitchClass: pitchClass, mode: .major)
        let minor = MusicalKey(pitchClass: pitchClass, mode: .minor)
        #expect(formatter.string(from: major) == expectedCamelotMajor[pitchClass.rawValue])
        #expect(formatter.string(from: minor) == expectedCamelotMinor[pitchClass.rawValue])
    }
}

@Test("Classic formatting covers every major and minor pitch class")
func formatsAllClassicKeys() {
    let formatter = KeyNotationFormatter(notation: .classic)
    let expectedPitchClasses = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"
    ]

    for pitchClass in PitchClass.allCases {
        let major = MusicalKey(pitchClass: pitchClass, mode: .major)
        let minor = MusicalKey(pitchClass: pitchClass, mode: .minor)
        let classic = expectedPitchClasses[pitchClass.rawValue]
        #expect(formatter.string(from: major) == classic)
        #expect(formatter.string(from: minor) == "\(classic)m")
    }
}

@Test("Changing notation never mutates canonical musical data")
func preservesCanonicalKey() {
    let key = MusicalKey(pitchClass: .a, mode: .minor)

    #expect(KeyNotationFormatter(notation: .camelot).string(from: key) == "8A")
    #expect(KeyNotationFormatter(notation: .classic).string(from: key) == "Am")
    #expect(key == MusicalKey(pitchClass: .a, mode: .minor))
}
