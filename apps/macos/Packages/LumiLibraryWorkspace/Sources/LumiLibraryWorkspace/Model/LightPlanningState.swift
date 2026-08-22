import Foundation
import LumiProtocol

public enum LightPlanColorBehavior: String, CaseIterable, Identifiable, Sendable {
    case neutral
    case prefer
    case only

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .neutral: "Neutral"
        case .prefer: "Prefer"
        case .only: "Only"
        }
    }
}

public struct LightPlanAutoloopRule: Identifiable, Equatable, Sendable {
    public var id: String { "\(themeID):\(roleID):\(variantID)" }
    public let themeID: UInt64
    public let roleID: String
    public let variantID: String
    public var enabled: Bool
    public var selectionWeight: UInt8
    public var colorBehavior: LightPlanColorBehavior
    public var colorRGB: [UInt32]

    public init(
        themeID: UInt64,
        roleID: String,
        variantID: String,
        enabled: Bool = true,
        selectionWeight: UInt8 = 2,
        colorBehavior: LightPlanColorBehavior = .neutral,
        colorRGB: [UInt32] = []
    ) {
        self.themeID = themeID
        self.roleID = roleID
        self.variantID = variantID
        self.enabled = enabled
        self.selectionWeight = selectionWeight
        self.colorBehavior = colorBehavior
        self.colorRGB = colorRGB
    }
}

public enum LightPlanModifierKind: String, CaseIterable, Identifiable, Sendable {
    case atmosphere
    case color
    public var id: String { rawValue }
    public var label: String { self == .atmosphere ? "Static Look" : "Color Override" }
}

public struct LightPlanOutputModifier: Identifiable, Equatable, Sendable {
    public let id: String
    public var providerKind: String
    public var kind: LightPlanModifierKind
    public var displayName: String
    public var enabled: Bool
    public var midiChannel: UInt8
    public var midiNote: UInt8
    public var activationVerified: Bool
    public var releaseVerified: Bool

    public var automaticExecutionReady: Bool {
        enabled && activationVerified && releaseVerified
    }
}

public enum LightPlanModifierScope: String, CaseIterable, Identifiable, Sendable {
    case phrase
    case track
    public var id: String { rawValue }
    public var label: String { self == .phrase ? "Phrase" : "Whole Track" }
}

public struct LightPlanModifierRule: Identifiable, Equatable, Sendable {
    public var id: String { "\(modifierID):\(roleID):\(scope.rawValue)" }
    public var modifierID: String
    public var roleID: String
    public var applicationRate: UInt8
    public var selectionWeight: UInt8
    public var cooldownUses: UInt8
    public var scope: LightPlanModifierScope
    public var colorBehavior: LightPlanColorBehavior
    public var colorRGB: [UInt32]
}

public struct LightPlanningPolicyState: Equatable, Sendable {
    public var revision: UInt64
    public var themeCooldownTracks: UInt8
    public var autoloopCooldownUses: UInt8
    public var duplicatePlanWindow: UInt8
    public var rules: [LightPlanAutoloopRule]
    public var modifiers: [LightPlanOutputModifier]
    public var modifierRules: [LightPlanModifierRule]

    public init(
        revision: UInt64 = 1,
        themeCooldownTracks: UInt8 = 1,
        autoloopCooldownUses: UInt8 = 2,
        duplicatePlanWindow: UInt8 = 4,
        rules: [LightPlanAutoloopRule] = [],
        modifiers: [LightPlanOutputModifier] = [],
        modifierRules: [LightPlanModifierRule] = []
    ) {
        self.revision = revision
        self.themeCooldownTracks = themeCooldownTracks
        self.autoloopCooldownUses = autoloopCooldownUses
        self.duplicatePlanWindow = duplicatePlanWindow
        self.rules = rules
        self.modifiers = modifiers
        self.modifierRules = modifierRules
    }

    public func payload() -> JSONValue {
        .object([
            "revision": .number(Double(revision)),
            "themeCooldownTracks": .number(Double(themeCooldownTracks)),
            "autoloopCooldownUses": .number(Double(autoloopCooldownUses)),
            "duplicatePlanWindow": .number(Double(duplicatePlanWindow)),
            "rules": .array(rules.map { rule in
                .object([
                    "themeId": .number(Double(rule.themeID)),
                    "roleId": .string(rule.roleID),
                    "variantId": .string(rule.variantID),
                    "enabled": .boolean(rule.enabled),
                    "selectionWeight": .number(Double(rule.selectionWeight)),
                    "colorBehavior": .string(rule.colorBehavior.rawValue),
                    "colorRgb": .array(rule.colorRGB.map { .number(Double($0)) })
                ])
            }),
            "modifiers": .array(modifiers.map { modifier in
                .object([
                    "id": .string(modifier.id),
                    "providerKind": .string(modifier.providerKind),
                    "kind": .string(modifier.kind.rawValue),
                    "displayName": .string(modifier.displayName),
                    "enabled": .boolean(modifier.enabled),
                    "midiChannel": .number(Double(modifier.midiChannel)),
                    "midiNote": .number(Double(modifier.midiNote)),
                    "activationVerified": .boolean(modifier.activationVerified),
                    "releaseVerified": .boolean(modifier.releaseVerified)
                ])
            }),
            "modifierRules": .array(modifierRules.map { rule in
                .object([
                    "modifierId": .string(rule.modifierID),
                    "roleId": .string(rule.roleID),
                    "applicationRate": .number(Double(rule.applicationRate)),
                    "selectionWeight": .number(Double(rule.selectionWeight)),
                    "cooldownUses": .number(Double(rule.cooldownUses)),
                    "scope": .string(rule.scope.rawValue),
                    "colorBehavior": .string(rule.colorBehavior.rawValue),
                    "colorRgb": .array(rule.colorRGB.map { .number(Double($0)) })
                ])
            })
        ])
    }
}

public struct LightPlanPreviewPhrase: Identifiable, Equatable, Sendable {
    public var id: UInt16 { phraseIndex }
    public let phraseIndex: UInt16
    public let startBeat: UInt32
    public let endBeat: UInt32
    public let roleID: String
    public let roleName: String
    public let variantID: String
    public let autoloopName: String
    public let autoloopNumber: UInt16
    public let reason: String
    public let effectiveWeight: UInt8
    public let colorInfluence: String
    public let repeatProtection: String
    public let modifiers: [LightPlanPreviewModifier]
}

public struct LightPlanPreviewModifier: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let kind: LightPlanModifierKind
    public let scope: LightPlanModifierScope
    public let midiChannel: UInt8
    public let midiNote: UInt8
    public let reason: String
    public let colorInfluence: String
}

public struct LightPlanPreview: Equatable, Sendable {
    public let trackID: UInt64
    public let trackTitle: String
    public let themeID: UInt64
    public let policyRevision: UInt64
    public let variationSeed: String
    public let signature: String
    public let phrases: [LightPlanPreviewPhrase]
}

public struct LightPlanningExecutionState: Equatable, Sendable {
    public let compiledBeforePlayback: Bool
    public let realtimePolicyEvaluation: Bool
    public let staticLookOutput: String
    public let colorOverrideOutput: String
}

public struct LightPlanTrackColorState: Identifiable, Equatable, Sendable {
    public var id: UInt32 { rgb }
    public let rgb: UInt32
    public let name: String
    public let trackCount: UInt64
}

public struct LightPlanningState: Equatable, Sendable {
    public let policy: LightPlanningPolicyState
    public let trackColors: [LightPlanTrackColorState]
    public let execution: LightPlanningExecutionState
    public let preview: LightPlanPreview?

    public static let loading = Self(
        policy: .init(),
        trackColors: [],
        execution: .init(
            compiledBeforePlayback: true,
            realtimePolicyEvaluation: false,
            staticLookOutput: "verifiedAutomatic",
            colorOverrideOutput: "pocRequired"
        ),
        preview: nil
    )
}
