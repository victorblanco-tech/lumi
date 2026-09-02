import Combine
import Foundation
import Network
import dnssd

public enum RemoteDiscoveryState: Equatable, Sendable {
    case idle
    case searching
    case ready
    case permissionRequired
    case failed
}

public struct RemoteDiscoveredService: Equatable, Identifiable, Sendable {
    public var id: String { identity.installationID }
    public let identity: RemoteServiceIdentity
    public let endpoint: NWEndpoint

    public init(identity: RemoteServiceIdentity, endpoint: NWEndpoint) {
        self.identity = identity
        self.endpoint = endpoint
    }
}

public enum RemoteDiscoveryMetadata {
    public static let installationIDKey = "id"
    public static let protocolVersionKey = "pv"
    public static let releaseChannelKey = "channel"

    public static func identity(
        serviceName: String,
        textRecord: [String: String],
        expectedChannel: RemoteReleaseChannel
    ) throws -> RemoteServiceIdentity {
        guard let installationID = textRecord[installationIDKey],
              let protocolText = textRecord[protocolVersionKey],
              let protocolVersion = Int(protocolText),
              let releaseText = textRecord[releaseChannelKey],
              let releaseChannel = RemoteReleaseChannel(rawValue: releaseText) else {
            throw RemoteTrustError.invalidServiceIdentity
        }
        let identity = RemoteServiceIdentity(
            name: serviceName,
            installationID: installationID,
            protocolVersion: protocolVersion,
            releaseChannel: releaseChannel
        )
        try identity.validate(for: expectedChannel)
        return identity
    }
}

@MainActor
public final class BonjourRemoteDiscovery: ObservableObject {
    @Published public private(set) var state: RemoteDiscoveryState = .idle
    @Published public private(set) var services: [RemoteDiscoveredService] = []
    @Published public private(set) var rejectedServiceCount = 0

    private let releaseChannel: RemoteReleaseChannel
    private var browser: NWBrowser?

    public init(releaseChannel: RemoteReleaseChannel) {
        self.releaseChannel = releaseChannel
    }

    public func start() {
        stop()
        state = .searching
        let browser = NWBrowser(
            for: .bonjourWithTXTRecord(
                type: releaseChannel.bonjourServiceType,
                domain: nil
            ),
            using: .tcp
        )
        browser.stateUpdateHandler = { [weak self] browserState in
            Task { @MainActor [weak self] in
                self?.apply(browserState)
            }
        }
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            Task { @MainActor [weak self] in
                self?.apply(results)
            }
        }
        self.browser = browser
        browser.start(queue: .main)
    }

    public func stop() {
        browser?.cancel()
        browser = nil
        services = []
        rejectedServiceCount = 0
        state = .idle
    }

    private func apply(_ browserState: NWBrowser.State) {
        switch browserState {
        case .setup, .waiting:
            state = .searching
        case .ready:
            state = .ready
        case let .failed(error):
            state = isLocalNetworkPermissionError(error) ? .permissionRequired : .failed
        case .cancelled:
            if browser == nil { state = .idle }
        @unknown default:
            state = .failed
        }
    }

    private func apply(_ results: Set<NWBrowser.Result>) {
        var accepted: [RemoteDiscoveredService] = []
        var rejected = 0
        for result in results {
            guard case let .service(name, _, _, _) = result.endpoint,
                  case let .bonjour(textRecord) = result.metadata else {
                rejected += 1
                continue
            }
            do {
                let identity = try RemoteDiscoveryMetadata.identity(
                    serviceName: name,
                    textRecord: textRecord.dictionary,
                    expectedChannel: releaseChannel
                )
                accepted.append(RemoteDiscoveredService(identity: identity, endpoint: result.endpoint))
            } catch {
                rejected += 1
            }
        }
        services = accepted.sorted {
            if $0.identity.name != $1.identity.name {
                return $0.identity.name.localizedStandardCompare($1.identity.name) == .orderedAscending
            }
            return $0.identity.installationID < $1.identity.installationID
        }
        rejectedServiceCount = rejected
    }

    private func isLocalNetworkPermissionError(_ error: NWError) -> Bool {
        guard case let .dns(code) = error else { return false }
        return code == kDNSServiceErr_PolicyDenied
    }
}
