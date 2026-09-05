import Combine
import Foundation
import LumiProtocol
import OSLog
#if os(iOS)
import UIKit
#endif

@MainActor
public final class RemoteConnectionController: ObservableObject {
    private struct PairingCandidate: Sendable {
        let invitation: RemotePairingInvitation
        let deviceID: String
        let credential: String
    }

    private let model: RemoteSessionModel
    private let releaseChannel: RemoteReleaseChannel
    private let credentialStore: any RemoteCredentialStore
    private let frameDecoder = RemoteFrameDecoder()
    private let commandCoordinator = RemoteCommandCoordinator()
    private let logger = Logger(
        subsystem: "co.victorblan.tech.lumi.remote",
        category: "Connection"
    )
    private var transport: PinnedRemoteTransport?
    private var connectionTask: Task<Void, Never>?
    private var connectionGeneration: UInt64 = 0
    private var services: [RemoteDiscoveredService] = []
    private var pairingCandidate: PairingCandidate?

    public init(
        model: RemoteSessionModel,
        releaseChannel: RemoteReleaseChannel,
        credentialStore: (any RemoteCredentialStore)? = nil
    ) {
        self.model = model
        self.releaseChannel = releaseChannel
        self.credentialStore = credentialStore ?? KeychainRemoteCredentialStore(
            service: "co.victorblan.tech.lumi.remote.credentials",
            releaseChannel: releaseChannel
        )
    }

    public func update(discoveredServices: [RemoteDiscoveredService]) {
        guard services != discoveredServices else { return }
        services = discoveredServices
        reconnect()
    }

    public func acceptPairingURL(_ url: URL, now: Date = .now) {
        do {
            let invitation = try RemotePairingCodeCodec().decode(
                url,
                nowUnixMillis: Self.unixMillis(now)
            )
            guard services.contains(where: {
                $0.identity.installationID == invitation.installationID
            }) else {
                pairingCandidate = PairingCandidate(
                    invitation: invitation,
                    deviceID: UUID().uuidString.lowercased(),
                    credential: try Self.randomCredential()
                )
                model.beginPairing(shortCode: invitation.shortCode)
                return
            }
            pairingCandidate = PairingCandidate(
                invitation: invitation,
                deviceID: UUID().uuidString.lowercased(),
                credential: try Self.randomCredential()
            )
            model.beginPairing(shortCode: invitation.shortCode)
            reconnect()
        } catch {
            model.unavailable(error.localizedDescription)
        }
    }

    public func stop() {
        connectionGeneration &+= 1
        connectionTask?.cancel()
        connectionTask = nil
        let transport = transport
        self.transport = nil
        Task { await transport?.close() }
        commandCoordinator.disconnected()
        model.revokeControllerLease()
    }

    public func setOperationState(_ state: RemoteOperationState) {
        submitStateCommand(target: "operationState") { revision in
            .setOperationState(state, expectedStateRevision: revision)
        }
    }

    public func setAbletonLinkEnabled(_ enabled: Bool) {
        submitStateCommand(target: "abletonLink") { revision in
            .setAbletonLinkEnabled(enabled, expectedStateRevision: revision)
        }
    }

    public func setTimingOffset(_ millis: Int) {
        guard let value = Int16(exactly: millis) else {
            model.reportError("The timing offset is outside the supported range.")
            return
        }
        submitStateCommand(target: "timingOffset") { revision in
            .setOutputTimingOffset(value, expectedStateRevision: revision)
        }
    }

    public func selectTheme(
        plan: RemoteLightPlan,
        cue: RemotePlanCue,
        themeID: UInt64
    ) {
        submitPlanCommand(plan: plan, cue: cue, target: "theme") {
            .selectThemeFromPhrase($0, themeID: themeID)
        }
    }

    public func changePhraseRole(
        plan: RemoteLightPlan,
        cue: RemotePlanCue,
        roleID: String
    ) {
        submitPlanCommand(plan: plan, cue: cue, target: "phraseRole") {
            .changePhraseRole($0, roleID: roleID)
        }
    }

    public func selectAutoloop(
        plan: RemoteLightPlan,
        cue: RemotePlanCue,
        autoloopNumber: UInt8
    ) {
        submitPlanCommand(plan: plan, cue: cue, target: "autoloop") {
            .selectAutoloopForPhrase($0, autoloopNumber: autoloopNumber)
        }
    }

    public func setCueLock(
        plan: RemoteLightPlan,
        cue: RemotePlanCue,
        locked: Bool
    ) {
        submitPlanCommand(plan: plan, cue: cue, target: "cueLock") {
            .setCueLock($0, locked: locked)
        }
    }

    private func reconnect() {
        connectionGeneration &+= 1
        let generation = connectionGeneration
        connectionTask?.cancel()
        connectionTask = nil
        let previousTransport = transport
        transport = nil
        commandCoordinator.disconnected()
        model.revokeControllerLease()
        guard !services.isEmpty else {
            Task { await previousTransport?.close() }
            if let candidate = pairingCandidate {
                model.beginPairing(shortCode: candidate.invitation.shortCode)
            } else {
                model.beginDiscovery()
            }
            return
        }
        connectionTask = Task { [weak self] in
            await previousTransport?.close()
            guard !Task.isCancelled else { return }
            await self?.runConnectionLoop(generation: generation)
        }
    }

    private func runConnectionLoop(generation: UInt64) async {
        while connectionIsCurrent(generation) {
            var attemptedService: RemoteDiscoveredService?
            do {
                guard let service = try await selectedService() else {
                    if connectionIsCurrent(generation) {
                        model.beginPairing(shortCode: pairingCandidate?.invitation.shortCode)
                    }
                    return
                }
                attemptedService = service
                try await connect(to: service, generation: generation)
                return
            } catch RemoteTransportError.gatewayRejected("approvalRequired") {
                guard connectionIsCurrent(generation) else { return }
                model.beginPairing(shortCode: pairingCandidate?.invitation.shortCode)
            } catch RemoteTransportError.gatewayRejected("deviceRevoked") {
                guard connectionIsCurrent(generation) else { return }
                if let service = attemptedService {
                    try? await credentialStore.remove(
                        installationID: service.identity.installationID
                    )
                }
                guard connectionIsCurrent(generation) else { return }
                model.beginPairing(shortCode: pairingCandidate?.invitation.shortCode)
            } catch is CancellationError {
                return
            } catch {
                guard connectionIsCurrent(generation) else { return }
                logger.error(
                    "Remote connection cycle failed: \(String(reflecting: error), privacy: .public)"
                )
                let name = attemptedService?.identity.name ?? "Lumi Mac"
                model.reconnecting(to: name)
            }
            try? await Task.sleep(for: .seconds(1))
        }
    }

    private func selectedService() async throws -> RemoteDiscoveredService? {
        if let invitation = pairingCandidate?.invitation {
            return services.first(where: {
                $0.identity.installationID == invitation.installationID
            })
        }
        for service in services {
            if try await credentialStore.credential(
                for: service.identity.installationID
            ) != nil {
                return service
            }
        }
        return nil
    }

    private func connect(
        to service: RemoteDiscoveredService,
        generation: UInt64
    ) async throws {
        let stored = try await Self.storedCredential(
            for: service.identity.installationID,
            pairingInProgress: pairingCandidate != nil,
            store: credentialStore
        )
        let expectedFingerprint = pairingCandidate?.invitation.installationID
            == service.identity.installationID
            ? pairingCandidate?.invitation.certificateFingerprintSHA256
            : stored?.certificateFingerprintSHA256
        guard let expectedFingerprint else {
            model.beginPairing(shortCode: pairingCandidate?.invitation.shortCode)
            return
        }
        guard connectionIsCurrent(generation) else { return }
        let activeTransport = PinnedRemoteTransport()
        transport = activeTransport
        do {
            try await activeTransport.connect(
                to: service.endpoint,
                certificateFingerprintSHA256: expectedFingerprint
            )
            guard connectionIsCurrent(generation) else { throw CancellationError() }

            guard let hello = Self.authenticationHello(
                stored: stored,
                pairingInvitation: pairingCandidate?.invitation,
                pairingDeviceID: pairingCandidate?.deviceID,
                pairingCredential: pairingCandidate?.credential,
                displayName: Self.deviceName
            ) else {
                model.beginPairing()
                throw RemoteTransportError.invalidAuthenticationResponse
            }

            let response = try await activeTransport.authenticate(hello)
            guard connectionIsCurrent(generation) else { throw CancellationError() }
            guard response.installationID == service.identity.installationID else {
                throw RemoteTrustError.credentialScopeMismatch
            }
            if stored == nil, let candidate = pairingCandidate {
                try await credentialStore.save(
                    RemoteDeviceCredential(
                        installationID: response.installationID,
                        deviceID: candidate.deviceID,
                        credential: candidate.credential,
                        certificateFingerprintSHA256: expectedFingerprint,
                        releaseChannel: releaseChannel
                    )
                )
                pairingCandidate = nil
            }
            commandCoordinator.updateControllerLease(response.controllerLeaseID)
            model.updateControllerDisplayName(response.controllerDisplayName)
            if let lease = response.controllerLeaseID {
                model.grantControllerLease(lease)
            } else {
                model.revokeControllerLease()
            }
            model.connected(to: service.identity.name)
            let processor = RemoteFrameProcessor(model: model, macName: service.identity.name)
            processor.reset(for: service.identity.name)

            while connectionIsCurrent(generation),
                  let data = try await activeTransport.nextFrame() {
                guard connectionIsCurrent(generation) else { return }
                let frame = try frameDecoder.decodeFrame(data)
                if frame.frameKind == .commandResult,
                   let commandID = frame.correlationID {
                    commandCoordinator.resolve(commandID: commandID)
                }
                let decision = try processor.process(data)
                switch decision {
                case .snapshotRequired, .authoritativeSnapshotRequired:
                    try await sendSnapshotRequest(using: activeTransport)
                case .applied, .duplicateIgnored, .unrelated:
                    break
                }
            }
            throw RemoteTransportError.connectionClosed
        } catch {
            await activeTransport.close()
            if transport === activeTransport {
                transport = nil
                commandCoordinator.disconnected()
                model.revokeControllerLease()
            }
            throw error
        }
    }

    static func storedCredential(
        for installationID: String,
        pairingInProgress: Bool,
        store: any RemoteCredentialStore
    ) async throws -> RemoteDeviceCredential? {
        guard !pairingInProgress else { return nil }
        return try await store.credential(for: installationID)
    }

    static func authenticationHello(
        stored: RemoteDeviceCredential?,
        pairingInvitation: RemotePairingInvitation?,
        pairingDeviceID: String?,
        pairingCredential: String?,
        displayName: String
    ) -> RemoteClientHello? {
        if let invitation = pairingInvitation,
           let pairingDeviceID,
           let pairingCredential {
            return .pair(
                invitationID: invitation.invitationID,
                invitationSecret: invitation.invitationSecret,
                deviceID: pairingDeviceID,
                displayName: displayName,
                deviceCredential: pairingCredential
            )
        }
        if let stored {
            return .authenticate(
                deviceID: stored.deviceID,
                credential: stored.credential
            )
        }
        return nil
    }

    private func submitStateCommand(
        target: String,
        payload: (UInt64) -> RemoteCommandPayload
    ) {
        guard let projection = model.projection else { return }
        do {
            let command = try commandCoordinator.makeStateCommand(
                payload,
                projection: projection,
                target: target
            )
            submit(command)
        } catch {
            model.reportError(error.localizedDescription)
        }
    }

    private func submitPlanCommand(
        plan: RemoteLightPlan,
        cue: RemotePlanCue,
        target: String,
        payload: (RemotePlanMutationContext) -> RemoteCommandPayload
    ) {
        guard let projection = model.projection,
              let player = projection.players.first(where: {
                  $0.trackLoadID == plan.trackLoadID
              }) else { return }
        do {
            let command = try commandCoordinator.makePlanCommand(
                plan: plan,
                cue: cue,
                player: player,
                payload: payload,
                target: "\(plan.planID):\(cue.phraseIndex):\(target)"
            )
            submit(command)
        } catch {
            model.rejectCommand("planMutation", reason: error.localizedDescription)
        }
    }

    private func submit(_ command: RemoteCommand) {
        guard let transport else { return }
        let generation = connectionGeneration
        model.markCommandPending(command.commandID)
        Task { [weak self] in
            do {
                try await transport.send(command: command)
            } catch {
                guard let self,
                      self.connectionGeneration == generation,
                      self.transport === transport else { return }
                self.commandCoordinator.resolve(commandID: command.commandID)
                self.model.rejectCommand(command.commandID, reason: error.localizedDescription)
            }
        }
    }

    private func sendSnapshotRequest(using transport: PinnedRemoteTransport) async throws {
        guard let projection = model.projection else { return }
        let command = try commandCoordinator.makeStateCommand(
            { _ in .requestSnapshot },
            projection: projection,
            target: "snapshot"
        )
        try await transport.send(command: command)
        commandCoordinator.resolve(commandID: command.commandID)
    }

    private static var deviceName: String {
        #if os(iOS)
        UIDevice.current.name
        #else
        Host.current().localizedName ?? "iPhone"
        #endif
    }

    private static func randomCredential() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
            throw RemoteTransportError.connectionFailed
        }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    private static func unixMillis(_ date: Date) -> UInt64 {
        UInt64(max(0, date.timeIntervalSince1970 * 1_000))
    }

    private func connectionIsCurrent(_ generation: UInt64) -> Bool {
        !Task.isCancelled && connectionGeneration == generation
    }

    var connectionGenerationForTesting: UInt64 { connectionGeneration }
}
