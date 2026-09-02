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
    public private(set) var pairingShortCode: String?
    public private(set) var acceptedCommandFeedbackRevision: UInt64 = 0
    public private(set) var rejectedCommandFeedbackRevision: UInt64 = 0

    public init() {}

    public var controlsEnabled: Bool {
        guard case .connected = connectionPhase else { return false }
        return projection != nil && controllerLeaseID != nil
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

    public func apply(_ incoming: RemoteLiveProjection, from macName: String) throws {
        if let current = projection,
           incoming.projectionRevision <= current.projectionRevision {
            throw RemoteContractError.nonIncreasingRevision
        }
        projection = incoming
        connectionPhase = .connected(macName: macName)
        lastError = nil
    }

    public func replaceWithSnapshot(_ incoming: RemoteLiveProjection, from macName: String) {
        projection = incoming
        connectionPhase = .connected(macName: macName)
        pendingCommandIDs.removeAll()
        lastError = nil
    }

    public func applyTransportAnchor(
        playerNumber: UInt8,
        anchor: RemoteTransportAnchor
    ) throws {
        guard let current = projection,
              let playerIndex = current.players.firstIndex(where: {
                  $0.playerNumber == playerNumber
              }),
              current.players[playerIndex].trackLoadID == anchor.trackLoadID else {
            throw RemoteContractError.invalidTransportAnchor
        }
        let existing = current.players[playerIndex].transport
        guard anchor.discontinuityRevision >= existing.discontinuityRevision,
              anchor.observedAtUnixMillis >= existing.observedAtUnixMillis else {
            return
        }
        var players = current.players
        let player = players[playerIndex]
        players[playerIndex] = RemotePlayer(
            playerNumber: player.playerNumber,
            hardwareModel: player.hardwareModel,
            trackLoadID: player.trackLoadID,
            transport: anchor,
            track: player.track
        )
        let integrations: RemoteIntegrationStatus
        if current.leaderPlayerNumber == playerNumber {
            integrations = RemoteIntegrationStatus(
                proDJLink: current.integrations.proDJLink,
                lightOutput: current.integrations.lightOutput,
                abletonLink: current.integrations.abletonLink,
                abletonLinkEnabled: current.integrations.abletonLinkEnabled,
                abletonLinkBPMMilli: anchor.effectiveBPMMilli,
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
            themeOptions: current.themeOptions
        )
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
