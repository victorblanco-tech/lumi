public struct KeyNotationFormatter: Sendable {
    private static let classicPitchClasses = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"
    ]
    private static let camelotMajor = [
        "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"
    ]
    private static let camelotMinor = [
        "5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"
    ]

    public let notation: KeyNotationPreference

    public init(notation: KeyNotationPreference) {
        self.notation = notation
    }

    public func string(from key: MusicalKey) -> String {
        switch notation {
        case .camelot:
            let values = key.mode == .major ? Self.camelotMajor : Self.camelotMinor
            return values[key.pitchClass.rawValue]
        case .classic:
            let pitchClass = Self.classicPitchClasses[key.pitchClass.rawValue]
            return key.mode == .minor ? "\(pitchClass)m" : pitchClass
        }
    }
}
