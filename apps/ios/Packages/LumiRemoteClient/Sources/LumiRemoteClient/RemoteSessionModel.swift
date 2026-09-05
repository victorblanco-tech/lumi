import Foundation
import Observation

public enum RemoteConnectionPhase: Equatable, Sendable {
    case unavailable
    case discovering
    case pairing
    case connected(macName: String)
    case reconnecting(macName: String, staleSince: Date)
    case incompatible(requiredProtocol: Int, receivedProtocol: Int)
}

@MainActor
@Observable
public final class RemoteSessionModel {
    public private(set) var connectionPhase: RemoteConnectionPhase = .unavailable
    public private(set) var projection: RemoteLiveProjection?
    public private(set) var pendingCommandIDs: Set<String> = []
    public private(set) var lastError: String?
    public private(set) var controllerLeaseID: String?
    public private(set) var controllerDisplayName: String?
    public private(set) var pairingShortCode: String?
    public private(set) var acceptedCommandFeedbackRevision: UInt64 = 0
    public private(set) var rejectedCommandFeedbackRevision: UInt64 = 0
    private var sourceObservationUnixMillisByPlayer: [UInt8: UInt64] = [:]

    public init() {}

    public var controlsEnabled: Bool {
        connectionIsHealthy && controllerLeaseID != nil
    }

    public var connectionIsHealthy: Bool {
        guard case .connected = connectionPhase else { return false }
        return projection != nil
    }

    public var controlRoleLabel: String {
        controllerLeaseID == nil ? "View only" : "Controller"
    }

    public func updateControllerDisplayName(_ name: String?) {
        controllerDisplayName = name
    }

    public func beginDiscovery() {
        connectionPhase = .discovering
        lastError = nil
        pairingShortCode = nil
    }

    public func beginPairing(shortCode: String? = nil) {
        connectionPhase = .pairing
        lastError = nil
        pairingShortCode = shortCode
    }

    public func connected(to macName: String) {
        connectionPhase = .connected(macName: macName)
        lastError = nil
        pairingShortCode = nil
    }

    public func grantControllerLease(_ leaseID: String) {
        guard !leaseID.isEmpty, leaseID.count <= 128 else { return }
        controllerLeaseID = leaseID
    }

    public func revokeControllerLease() {
        controllerLeaseID = nil
        pendingCommandIDs.removeAll()
    }

    public func apply(
        _ incoming: RemoteLiveProjection,
        from macName: String,
        receivedAt: Date = .now
    ) throws {
        if let current = projection,
           incoming.projectionRevision <= current.projectionRevision {
            throw RemoteContractError.nonIncreasingRevision
        }
        projection = localizeProjection(incoming, receivedAt: receivedAt)
        connectionPhase = .connected(macName: macName)
        lastError = nil
    }

    public func replaceWithSnapshot(
        _ incoming: RemoteLiveProjection,
        from macName: String,
        receivedAt: Date = .now
    ) {
        sourceObservationUnixMillisByPlayer.removeAll(keepingCapacity: true)
        projection = localizeProjection(incoming, receivedAt: receivedAt)
        connectionPhase = .connected(macName: macName)
        pendingCommandIDs.removeAll()
        lastError = nil
    }

    public func applyTransportAnchor(
        playerNumber: UInt8,
        anchor: RemoteTransportAnchor,
        receivedAt: Date = .now
    ) throws {
        guard let current = projection,
              let playerIndex = current.players.firstIndex(where: {
                  $0.playerNumber == playerNumber
              }),
              current.players[playerIndex].trackLoadID == anchor.trackLoadID else {
            throw RemoteContractError.invalidTransportAnchor
        }
        let existing = current.players[playerIndex].transport
        let lastSourceObservation = sourceObservationUnixMillisByPlayer[playerNumber]
            ?? existing.observedAtUnixMillis
        guard anchor.discontinuityRevision >= existing.discontinuityRevision,
              anchor.observedAtUnixMillis >= lastSourceObservation else {
            return
        }
        sourceObservationUnixMillisByPlayer[playerNumber] = anchor.observedAtUnixMillis
        let localizedAnchor = anchor.localized(
            receivedAtUnixMillis: Self.unixMillis(receivedAt)
        )
        var players = current.players
        let player = players[playerIndex]
        players[playerIndex] = RemotePlayer(
            playerNumber: player.playerNumber,
            hardwareModel: player.hardwareModel,
            trackLoadID: player.trackLoadID,
            transport: localizedAnchor,
            track: player.track
        )
        let integrations: RemoteIntegrationStatus
        if current.leaderPlayerNumber == playerNumber {
            integrations = RemoteIntegrationStatus(
                proDJLink: current.integrations.proDJLink,
                lightOutput: current.integrations.lightOutput,
                abletonLink: current.integrations.abletonLink,
                abletonLinkEnabled: current.integrations.abletonLinkEnabled,
                abletonLinkBPMMilli: localizedAnchor.effectiveBPMMilli,
                timingOffsetMillis: current.integrations.timingOffsetMillis,
                pendingTimingOffsetMillis: current.integrations.pendingTimingOffsetMillis
            )
        } else {
            integrations = current.integrations
        }
        projection = RemoteLiveProjection(
            projectionRevision: current.projectionRevision,
            stateRevision: current.stateRevision,
            engineVersion: current.engineVersion,
            operationState: current.operationState,
            leaderPlayerNumber: current.leaderPlayerNumber,
            integrations: integrations,
            players: players,
            livePlan: current.livePlan,
            nextPlan: current.nextPlan,
            themeOptions: current.themeOptions,
            phraseRoleOptions: current.phraseRoleOptions
        )
    }

    private func localizeProjection(
        _ incoming: RemoteLiveProjection,
        receivedAt: Date
    ) -> RemoteLiveProjection {
        let receivedAtUnixMillis = Self.unixMillis(receivedAt)
        var sourceObservations: [UInt8: UInt64] = [:]
        let players = incoming.players.map { player in
            sourceObservations[player.playerNumber] =
                player.transport.observedAtUnixMillis
            return RemotePlayer(
                playerNumber: player.playerNumber,
                hardwareModel: player.hardwareModel,
                trackLoadID: player.trackLoadID,
                transport: player.transport.localized(
                    receivedAtUnixMillis: receivedAtUnixMillis
                ),
                track: player.track
            )
        }
        sourceObservationUnixMillisByPlayer = sourceObservations
        return RemoteLiveProjection(
            projectionRevision: incoming.projectionRevision,
            stateRevision: incoming.stateRevision,
            engineVersion: incoming.engineVersion,
            operationState: incoming.operationState,
            leaderPlayerNumber: incoming.leaderPlayerNumber,
            integrations: incoming.integrations,
            players: players,
            livePlan: incoming.livePlan,
            nextPlan: incoming.nextPlan,
            themeOptions: incoming.themeOptions,
            phraseRoleOptions: incoming.phraseRoleOptions
        )
    }

    private static func unixMillis(_ date: Date) -> UInt64 {
        UInt64(max(0, date.timeIntervalSince1970 * 1_000))
    }

    public func markCommandPending(_ commandID: String) {
        pendingCommandIDs.insert(commandID)
        lastError = nil
    }

    public func acknowledgeCommand(_ commandID: String) {
        guard pendingCommandIDs.remove(commandID) != nil else { return }
        acceptedCommandFeedbackRevision &+= 1
    }

    public func rejectCommand(_ commandID: String, reason: String) {
        pendingCommandIDs.remove(commandID)
        lastError = reason
        rejectedCommandFeedbackRevision &+= 1
    }

    public func reportError(_ reason: String) {
        lastError = reason
        rejectedCommandFeedbackRevision &+= 1
    }

    public func reconnecting(to macName: String, at date: Date = .now) {
        connectionPhase = .reconnecting(macName: macName, staleSince: date)
        pendingCommandIDs.removeAll()
        controllerLeaseID = nil
        pairingShortCode = nil
    }

    /// The authenticated transport/lease is still valid, but its last engine
    /// state is not. Keep the visible Players as stale and wait for fresh state.
    public func awaitingSnapshot(from macName: String, at date: Date = .now) {
        connectionPhase = .reconnecting(macName: macName, staleSince: date)
        pendingCommandIDs.removeAll()
        pairingShortCode = nil
    }

    public func unavailable(_ reason: String? = nil) {
        connectionPhase = .unavailable
        pendingCommandIDs.removeAll()
        controllerLeaseID = nil
        lastError = reason
        pairingShortCode = nil
    }

    public func incompatible(receivedProtocol: Int) {
        connectionPhase = .incompatible(
            requiredProtocol: lumiRemoteProtocolVersion,
            receivedProtocol: receivedProtocol
        )
        pendingCommandIDs.removeAll()
        controllerLeaseID = nil
        pairingShortCode = nil
    }
}

private extension RemoteTransportAnchor {
    func localized(receivedAtUnixMillis: UInt64) -> RemoteTransportAnchor {
        let sourceAgeMillis = publishedAtUnixMillis?
            .saturatingSubtracting(observedAtUnixMillis) ?? 0
        return RemoteTransportAnchor(
            trackLoadID: trackLoadID,
            beat: beat,
            positionMillis: positionMillis,
            effectiveBPMMilli: effectiveBPMMilli,
            playing: playing,
            discontinuityRevision: discontinuityRevision,
            observedAtUnixMillis: receivedAtUnixMillis
                .saturatingSubtracting(sourceAgeMillis),
            publishedAtUnixMillis: receivedAtUnixMillis
        )
    }
}

private extension UInt64 {
    func saturatingSubtracting(_ value: UInt64) -> UInt64 {
        self >= value ? self - value : 0
    }
}
