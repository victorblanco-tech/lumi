import LumiProtocol

public struct LightPlanningSnapshotDecoder: Sendable {
    public init() {}

    public func decode(_ envelope: MessageEnvelope) throws -> LightPlanningState {
        guard envelope.messageType == .snapshot,
              case let .object(library)? = envelope.payload["library"],
              case let .object(lightPlanning)? = library["lightPlanning"],
              case let .object(policy)? = lightPlanning["policy"],
              case let .object(execution)? = lightPlanning["execution"] else {
            throw LightPlanningSnapshotError.missingState
        }
        let ruleValues = try array(policy, "rules", maximum: 8_192)
        let modifierValues = try array(policy, "modifiers", maximum: 256)
        let modifierRuleValues = try array(policy, "modifierRules", maximum: 4_096)
        return LightPlanningState(
            policy: LightPlanningPolicyState(
                revision: try unsigned(policy, "revision"),
                themeCooldownTracks: try smallUnsigned(policy, "themeCooldownTracks"),
                autoloopCooldownUses: try smallUnsigned(policy, "autoloopCooldownUses"),
                duplicatePlanWindow: try smallUnsigned(policy, "duplicatePlanWindow"),
                rules: try ruleValues.map(decodeRule),
                modifiers: try modifierValues.map(decodeModifier),
                modifierRules: try modifierRuleValues.map(decodeModifierRule)
            ),
            trackColors: try optionalArray(lightPlanning, "trackColors", maximum: 64).map { value in
                guard case let .object(color) = value else {
                    throw LightPlanningSnapshotError.invalidState
                }
                return LightPlanTrackColorState(
                    rgb: try decodeRGB(required(color, "rgb")),
                    name: try string(color, "name"),
                    trackCount: try unsigned(color, "trackCount")
                )
            },
            execution: LightPlanningExecutionState(
                compiledBeforePlayback: try boolean(execution, "compiledBeforePlayback"),
                realtimePolicyEvaluation: try boolean(execution, "realtimePolicyEvaluation"),
                staticLookOutput: try string(execution, "staticLookOutput"),
                colorOverrideOutput: try string(execution, "colorOverrideOutput")
            ),
            preview: try decodePreview(lightPlanning["preview"])
        )
    }

    private func decodeRule(_ value: JSONValue) throws -> LightPlanAutoloopRule {
        guard case let .object(rule) = value,
              let behavior = LightPlanColorBehavior(rawValue: try string(rule, "colorBehavior")) else {
            throw LightPlanningSnapshotError.invalidState
        }
        return LightPlanAutoloopRule(
            themeID: try unsigned(rule, "themeId"),
            roleID: try string(rule, "roleId"),
            variantID: try string(rule, "variantId"),
            enabled: try boolean(rule, "enabled"),
            selectionWeight: try smallUnsigned(rule, "selectionWeight"),
            colorBehavior: behavior,
            colorRGB: try array(rule, "colorRgb", maximum: 64).map { value in
                guard case let .number(number) = value,
                      number >= 0, number <= Double(UInt32.max),
                      number.rounded(.towardZero) == number else {
                    throw LightPlanningSnapshotError.invalidState
                }
                return UInt32(number)
            }
        )
    }

    private func decodeModifier(_ value: JSONValue) throws -> LightPlanOutputModifier {
        guard case let .object(modifier) = value,
              let kind = LightPlanModifierKind(rawValue: try string(modifier, "kind")) else {
            throw LightPlanningSnapshotError.invalidState
        }
        return LightPlanOutputModifier(
            id: try string(modifier, "id"),
            providerKind: try string(modifier, "providerKind"),
            kind: kind,
            displayName: try string(modifier, "displayName"),
            enabled: try boolean(modifier, "enabled"),
            midiChannel: try smallUnsigned(modifier, "midiChannel"),
            midiNote: try smallUnsigned(modifier, "midiNote"),
            activationVerified: try boolean(modifier, "activationVerified"),
            releaseVerified: try boolean(modifier, "releaseVerified")
        )
    }

    private func decodeModifierRule(_ value: JSONValue) throws -> LightPlanModifierRule {
        guard case let .object(rule) = value,
              let scope = LightPlanModifierScope(rawValue: try string(rule, "scope")),
              let behavior = LightPlanColorBehavior(rawValue: try string(rule, "colorBehavior")) else {
            throw LightPlanningSnapshotError.invalidState
        }
        return LightPlanModifierRule(
            modifierID: try string(rule, "modifierId"),
            roleID: try string(rule, "roleId"),
            applicationRate: try smallUnsigned(rule, "applicationRate"),
            selectionWeight: try smallUnsigned(rule, "selectionWeight"),
            cooldownUses: try smallUnsigned(rule, "cooldownUses"),
            scope: scope,
            colorBehavior: behavior,
            colorRGB: try array(rule, "colorRgb", maximum: 64).map(decodeRGB)
        )
    }

    private func decodePreview(_ value: JSONValue?) throws -> LightPlanPreview? {
        guard let value, value != .null else { return nil }
        guard case let .object(preview) = value else {
            throw LightPlanningSnapshotError.invalidState
        }
        return LightPlanPreview(
            trackID: try unsigned(preview, "trackId"),
            trackTitle: try string(preview, "trackTitle"),
            themeID: try unsigned(preview, "themeId"),
            policyRevision: try unsigned(preview, "policyRevision"),
            variationSeed: try string(preview, "variationSeed"),
            signature: try string(preview, "signature"),
            phrases: try array(preview, "phrases", maximum: 2_048).map { value in
                guard case let .object(phrase) = value,
                      let phraseIndex = UInt16(exactly: try unsigned(phrase, "phraseIndex")),
                      let startBeat = UInt32(exactly: try unsigned(phrase, "startBeat")),
                      let endBeat = UInt32(exactly: try unsigned(phrase, "endBeat")),
                      let autoloopNumber = UInt16(exactly: try unsigned(phrase, "autoloopNumber")) else {
                    throw LightPlanningSnapshotError.invalidState
                }
                return LightPlanPreviewPhrase(
                    phraseIndex: phraseIndex,
                    startBeat: startBeat,
                    endBeat: endBeat,
                    roleID: try string(phrase, "roleId"),
                    roleName: try string(phrase, "roleName"),
                    variantID: try string(phrase, "variantId"),
                    autoloopName: try string(phrase, "autoloopName"),
                    autoloopNumber: autoloopNumber,
                    reason: try string(phrase, "reason"),
                    effectiveWeight: try smallUnsigned(phrase, "effectiveWeight"),
                    colorInfluence: try string(phrase, "colorInfluence"),
                    repeatProtection: try string(phrase, "repeatProtection")
                )
            }
        )
    }

    private func decodeRGB(_ value: JSONValue) throws -> UInt32 {
        guard case let .number(number) = value,
              number >= 0, number <= Double(UInt32.max),
              number.rounded(.towardZero) == number else {
            throw LightPlanningSnapshotError.invalidState
        }
        return UInt32(number)
    }

    private func array(
        _ object: [String: JSONValue],
        _ key: String,
        maximum: Int
    ) throws -> [JSONValue] {
        guard case let .array(values)? = object[key], values.count <= maximum else {
            throw LightPlanningSnapshotError.invalidState
        }
        return values
    }

    private func optionalArray(
        _ object: [String: JSONValue],
        _ key: String,
        maximum: Int
    ) throws -> [JSONValue] {
        guard let value = object[key] else { return [] }
        guard case let .array(values) = value, values.count <= maximum else {
            throw LightPlanningSnapshotError.invalidState
        }
        return values
    }

    private func required(_ object: [String: JSONValue], _ key: String) throws -> JSONValue {
        guard let value = object[key] else { throw LightPlanningSnapshotError.invalidState }
        return value
    }

    private func string(_ object: [String: JSONValue], _ key: String) throws -> String {
        guard case let .string(value)? = object[key], !value.isEmpty else {
            throw LightPlanningSnapshotError.invalidState
        }
        return value
    }

    private func boolean(_ object: [String: JSONValue], _ key: String) throws -> Bool {
        guard case let .boolean(value)? = object[key] else {
            throw LightPlanningSnapshotError.invalidState
        }
        return value
    }

    private func unsigned(_ object: [String: JSONValue], _ key: String) throws -> UInt64 {
        guard case let .number(value)? = object[key], value >= 0,
              value <= Double(UInt64.max), value.rounded(.towardZero) == value else {
            throw LightPlanningSnapshotError.invalidState
        }
        return UInt64(value)
    }

    private func smallUnsigned(_ object: [String: JSONValue], _ key: String) throws -> UInt8 {
        guard let value = UInt8(exactly: try unsigned(object, key)) else {
            throw LightPlanningSnapshotError.invalidState
        }
        return value
    }
}

public enum LightPlanningSnapshotError: Error {
    case missingState
    case invalidState
}
