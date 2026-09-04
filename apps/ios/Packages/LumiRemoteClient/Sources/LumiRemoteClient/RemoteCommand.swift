import Foundation

public enum RemoteCommandPayload: Equatable, Sendable {
    case setOperationState(RemoteOperationState, expectedStateRevision: UInt64)
    case setAbletonLinkEnabled(Bool, expectedStateRevision: UInt64)
    case setOutputTimingOffset(Int16, expectedStateRevision: UInt64)
    case changePhraseRole(RemotePlanMutationContext, roleID: String)
    case selectThemeFromPhrase(RemotePlanMutationContext, themeID: UInt64)
    case selectAutoloopForPhrase(RemotePlanMutationContext, autoloopNumber: UInt8)
    case setCueLock(RemotePlanMutationContext, locked: Bool)
    case requestSnapshot
}

public struct RemotePlanMutationContext: Codable, Equatable, Sendable {
    public let planID: String
    public let trackLoadID: UInt64
    public let expectedPlanRevision: UInt64
    public let phraseIndex: UInt16

    public init(
        planID: String,
        trackLoadID: UInt64,
        expectedPlanRevision: UInt64,
        phraseIndex: UInt16
    ) {
        self.planID = planID
        self.trackLoadID = trackLoadID
        self.expectedPlanRevision = expectedPlanRevision
        self.phraseIndex = phraseIndex
    }

    enum CodingKeys: String, CodingKey {
        case planID = "planId"
        case trackLoadID = "trackLoadId"
        case expectedPlanRevision
        case phraseIndex
    }
}

extension RemoteCommandPayload: Codable {
    private enum CodingKeys: String, CodingKey {
        case kind
        case operationState
        case expectedStateRevision
        case enabled
        case millis
        case planID = "planId"
        case trackLoadID = "trackLoadId"
        case expectedPlanRevision
        case phraseIndex
        case roleID = "roleId"
        case themeID = "themeId"
        case autoloopNumber
        case locked
    }

    private enum Kind: String, Codable {
        case setOperationState
        case setAbletonLinkEnabled
        case setOutputTimingOffset
        case changePhraseRole
        case selectThemeFromPhrase
        case selectAutoloopForPhrase
        case setCueLock
        case requestSnapshot
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .setOperationState:
            self = try .setOperationState(
                container.decode(RemoteOperationState.self, forKey: .operationState),
                expectedStateRevision: container.decode(UInt64.self, forKey: .expectedStateRevision)
            )
        case .setAbletonLinkEnabled:
            self = try .setAbletonLinkEnabled(
                container.decode(Bool.self, forKey: .enabled),
                expectedStateRevision: container.decode(UInt64.self, forKey: .expectedStateRevision)
            )
        case .setOutputTimingOffset:
            self = try .setOutputTimingOffset(
                container.decode(Int16.self, forKey: .millis),
                expectedStateRevision: container.decode(UInt64.self, forKey: .expectedStateRevision)
            )
        case .changePhraseRole:
            self = try .changePhraseRole(
                Self.decodePlanContext(from: container),
                roleID: container.decode(String.self, forKey: .roleID)
            )
        case .selectThemeFromPhrase:
            self = try .selectThemeFromPhrase(
                Self.decodePlanContext(from: container),
                themeID: container.decode(UInt64.self, forKey: .themeID)
            )
        case .selectAutoloopForPhrase:
            self = try .selectAutoloopForPhrase(
                Self.decodePlanContext(from: container),
                autoloopNumber: container.decode(UInt8.self, forKey: .autoloopNumber)
            )
        case .setCueLock:
            self = try .setCueLock(
                Self.decodePlanContext(from: container),
                locked: container.decode(Bool.self, forKey: .locked)
            )
        case .requestSnapshot:
            self = .requestSnapshot
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .setOperationState(state, revision):
            try container.encode(Kind.setOperationState, forKey: .kind)
            try container.encode(state, forKey: .operationState)
            try container.encode(revision, forKey: .expectedStateRevision)
        case let .setAbletonLinkEnabled(enabled, revision):
            try container.encode(Kind.setAbletonLinkEnabled, forKey: .kind)
            try container.encode(enabled, forKey: .enabled)
            try container.encode(revision, forKey: .expectedStateRevision)
        case let .setOutputTimingOffset(millis, revision):
            try container.encode(Kind.setOutputTimingOffset, forKey: .kind)
            try container.encode(millis, forKey: .millis)
            try container.encode(revision, forKey: .expectedStateRevision)
        case let .changePhraseRole(context, roleID):
            try container.encode(Kind.changePhraseRole, forKey: .kind)
            try Self.encode(context, to: &container)
            try container.encode(roleID, forKey: .roleID)
        case let .selectThemeFromPhrase(context, themeID):
            try container.encode(Kind.selectThemeFromPhrase, forKey: .kind)
            try Self.encode(context, to: &container)
            try container.encode(themeID, forKey: .themeID)
        case let .selectAutoloopForPhrase(context, number):
            try container.encode(Kind.selectAutoloopForPhrase, forKey: .kind)
            try Self.encode(context, to: &container)
            try container.encode(number, forKey: .autoloopNumber)
        case let .setCueLock(context, locked):
            try container.encode(Kind.setCueLock, forKey: .kind)
            try Self.encode(context, to: &container)
            try container.encode(locked, forKey: .locked)
        case .requestSnapshot:
            try container.encode(Kind.requestSnapshot, forKey: .kind)
        }
    }

    private static func decodePlanContext(
        from container: KeyedDecodingContainer<CodingKeys>
    ) throws -> RemotePlanMutationContext {
        try RemotePlanMutationContext(
            planID: container.decode(String.self, forKey: .planID),
            trackLoadID: container.decode(UInt64.self, forKey: .trackLoadID),
            expectedPlanRevision: container.decode(UInt64.self, forKey: .expectedPlanRevision),
            phraseIndex: container.decode(UInt16.self, forKey: .phraseIndex)
        )
    }

    private static func encode(
        _ context: RemotePlanMutationContext,
        to container: inout KeyedEncodingContainer<CodingKeys>
    ) throws {
        try container.encode(context.planID, forKey: .planID)
        try container.encode(context.trackLoadID, forKey: .trackLoadID)
        try container.encode(context.expectedPlanRevision, forKey: .expectedPlanRevision)
        try container.encode(context.phraseIndex, forKey: .phraseIndex)
    }
}

public struct RemoteCommand: Codable, Equatable, Identifiable, Sendable {
    public var id: String { commandID }
    public let commandID: String
    public let controllerLeaseID: String
    public let issuedAtUnixMillis: UInt64
    public let command: RemoteCommandPayload

    public init(
        commandID: String,
        controllerLeaseID: String,
        issuedAtUnixMillis: UInt64,
        command: RemoteCommandPayload
    ) {
        self.commandID = commandID
        self.controllerLeaseID = controllerLeaseID
        self.issuedAtUnixMillis = issuedAtUnixMillis
        self.command = command
    }

    enum CodingKeys: String, CodingKey {
        case commandID = "commandId"
        case controllerLeaseID = "controllerLeaseId"
        case issuedAtUnixMillis
        case command
    }
}

public enum RemoteCommandResultStatus: String, Codable, Sendable {
    case accepted
    case duplicate
    case conflict
    case rejected
}

public struct RemoteCommandResult: Codable, Equatable, Sendable {
    public let commandID: String
    public let status: RemoteCommandResultStatus
    public let stateRevision: UInt64?
    public let planRevision: UInt64?
    public let reasonCode: String?

    enum CodingKeys: String, CodingKey {
        case commandID = "commandId"
        case status
        case stateRevision
        case planRevision
        case reasonCode
    }
}

public enum RemoteCommandBuildError: Error, Equatable {
    case noControllerLease
    case duplicatePendingTarget
    case timingOffsetOutOfRange
    case invalidAutoloop
    case invalidPhraseRole
    case playerNoLongerLoaded
    case phraseAlreadyStarted
}

@MainActor
public final class RemoteCommandCoordinator {
    private var controllerLeaseID: String?
    private var pendingTargets: [String: String] = [:]

    public init(controllerLeaseID: String? = nil) {
        self.controllerLeaseID = controllerLeaseID
    }

    public func updateControllerLease(_ leaseID: String?) {
        controllerLeaseID = leaseID
        if leaseID == nil { pendingTargets.removeAll() }
    }

    public func makeStateCommand(
        _ payload: (UInt64) -> RemoteCommandPayload,
        projection: RemoteLiveProjection,
        target: String,
        commandID: String = UUID().uuidString,
        now: Date = .now
    ) throws -> RemoteCommand {
        try make(
            payload(projection.stateRevision),
            target: target,
            commandID: commandID,
            now: now
        )
    }

    public func makePlanCommand(
        plan: RemoteLightPlan,
        cue: RemotePlanCue,
        player: RemotePlayer,
        payload: (RemotePlanMutationContext) -> RemoteCommandPayload,
        target: String,
        commandID: String = UUID().uuidString,
        now: Date = .now
    ) throws -> RemoteCommand {
        guard player.trackLoadID == plan.trackLoadID else {
            throw RemoteCommandBuildError.playerNoLongerLoaded
        }
        guard player.transport.beat < cue.startBeat else {
            throw RemoteCommandBuildError.phraseAlreadyStarted
        }
        let context = RemotePlanMutationContext(
            planID: plan.planID,
            trackLoadID: plan.trackLoadID,
            expectedPlanRevision: plan.revision,
            phraseIndex: cue.phraseIndex
        )
        return try make(payload(context), target: target, commandID: commandID, now: now)
    }

    public func resolve(commandID: String) {
        pendingTargets = pendingTargets.filter { $0.value != commandID }
    }

    public func disconnected() {
        pendingTargets.removeAll()
    }

    private func make(
        _ payload: RemoteCommandPayload,
        target: String,
        commandID: String,
        now: Date
    ) throws -> RemoteCommand {
        guard let controllerLeaseID, !controllerLeaseID.isEmpty else {
            throw RemoteCommandBuildError.noControllerLease
        }
        guard pendingTargets[target] == nil else {
            throw RemoteCommandBuildError.duplicatePendingTarget
        }
        if case let .setOutputTimingOffset(millis, _) = payload,
           !(-250 ... 250).contains(millis) {
            throw RemoteCommandBuildError.timingOffsetOutOfRange
        }
        if case let .selectAutoloopForPhrase(_, number) = payload,
           !(1 ... 32).contains(number) {
            throw RemoteCommandBuildError.invalidAutoloop
        }
        if case let .changePhraseRole(_, roleID) = payload,
           roleID.isEmpty || roleID.count > 128 || roleID.unicodeScalars.contains(where: {
               CharacterSet.controlCharacters.contains($0)
           }) {
            throw RemoteCommandBuildError.invalidPhraseRole
        }
        pendingTargets[target] = commandID
        return RemoteCommand(
            commandID: commandID,
            controllerLeaseID: controllerLeaseID,
            issuedAtUnixMillis: UInt64(max(0, now.timeIntervalSince1970 * 1_000)),
            command: payload
        )
    }
}
