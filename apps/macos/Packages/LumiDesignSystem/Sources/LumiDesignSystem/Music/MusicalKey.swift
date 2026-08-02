public enum PitchClass: Int, CaseIterable, Codable, Equatable, Sendable {
    case c
    case cSharp
    case d
    case dSharp
    case e
    case f
    case fSharp
    case g
    case gSharp
    case a
    case aSharp
    case b
}

public enum KeyMode: String, CaseIterable, Codable, Equatable, Sendable {
    case major
    case minor
}

public struct MusicalKey: Codable, Equatable, Sendable {
    public let pitchClass: PitchClass
    public let mode: KeyMode

    public init(pitchClass: PitchClass, mode: KeyMode) {
        self.pitchClass = pitchClass
        self.mode = mode
    }
}
