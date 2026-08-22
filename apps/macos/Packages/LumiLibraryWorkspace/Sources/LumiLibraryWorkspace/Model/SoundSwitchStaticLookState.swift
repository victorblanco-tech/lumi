public struct SoundSwitchStaticLookProfileState: Equatable, Sendable {
    public static let builtIn = Self(
        id: "soundswitch-static-looks",
        name: "SoundSwitch Static Looks",
        slotCount: 32,
        midiChannel: 12,
        firstMIDINote: 64
    )

    public let id: String
    public let name: String
    public let slotCount: UInt16
    public let midiChannel: UInt8
    public let firstMIDINote: UInt8

    public func midiNote(for slotNumber: UInt16) -> UInt8? {
        guard (1...slotCount).contains(slotNumber) else { return nil }
        return firstMIDINote + UInt8(slotNumber - 1)
    }
}

public enum SoundSwitchStaticLookSlotStatus: String, Equatable, Sendable {
    case available
    case mapped
    case verified
}

public struct SoundSwitchStaticLookSlotState: Identifiable, Equatable, Sendable {
    public var id: UInt16 { number }

    public let number: UInt16
    public let midiChannel: UInt8
    public let midiNote: UInt8
    public let modifierID: String?
    public let displayName: String?
    public let enabled: Bool
    public let activationVerified: Bool
    public let releaseVerified: Bool
    public let status: SoundSwitchStaticLookSlotStatus
}

public enum SoundSwitchStaticLookProjection {
    public static func slots(
        policy: LightPlanningPolicyState,
        profile: SoundSwitchStaticLookProfileState = .builtIn
    ) -> [SoundSwitchStaticLookSlotState] {
        (1...profile.slotCount).compactMap { number in
            guard let note = profile.midiNote(for: number) else { return nil }
            let modifier = policy.modifiers.first {
                $0.providerKind == "soundswitch"
                    && $0.kind == .atmosphere
                    && $0.midiChannel == profile.midiChannel
                    && $0.midiNote == note
            }
            let status: SoundSwitchStaticLookSlotStatus
            if modifier?.automaticExecutionReady == true {
                status = .verified
            } else if modifier != nil {
                status = .mapped
            } else {
                status = .available
            }
            return SoundSwitchStaticLookSlotState(
                number: number,
                midiChannel: profile.midiChannel,
                midiNote: note,
                modifierID: modifier?.id,
                displayName: modifier?.displayName,
                enabled: modifier?.enabled ?? false,
                activationVerified: modifier?.activationVerified ?? false,
                releaseVerified: modifier?.releaseVerified ?? false,
                status: status
            )
        }
    }

    /// SoundSwitch numbers its four columns from top to bottom: 1–8, 9–16,
    /// 17–24 and 25–32. SwiftUI grids fill rows first, so reorder the slots to
    /// preserve the familiar SoundSwitch surface.
    public static func controllerGridSlots(
        policy: LightPlanningPolicyState,
        profile: SoundSwitchStaticLookProfileState = .builtIn
    ) -> [SoundSwitchStaticLookSlotState] {
        let source = slots(policy: policy, profile: profile)
        let rowsPerColumn = 8
        return (0..<rowsPerColumn).flatMap { row in
            stride(from: row, to: source.count, by: rowsPerColumn).map { source[$0] }
        }
    }
}
