import LumiRemoteClient
import LumiRemoteFeature
import SwiftUI
import UIKit

@main
struct LumiRemoteApp: App {
    @Environment(\.scenePhase) private var scenePhase
    @State private var session: RemoteSessionModel
    @StateObject private var discovery: BonjourRemoteDiscovery
    @StateObject private var connection: RemoteConnectionController

    init() {
        let session = RemoteSessionModel()
        _session = State(initialValue: session)
        _discovery = StateObject(
            wrappedValue: BonjourRemoteDiscovery(releaseChannel: Self.releaseChannel)
        )
        _connection = StateObject(
            wrappedValue: RemoteConnectionController(
                model: session,
                releaseChannel: Self.releaseChannel
            )
        )
    }

    var body: some Scene {
        WindowGroup {
            RemoteLiveView(
                model: session,
                actions: RemoteLiveActions(
                    setOperationState: connection.setOperationState,
                    setAbletonLinkEnabled: connection.setAbletonLinkEnabled,
                    setTimingOffset: connection.setTimingOffset,
                    selectTheme: connection.selectTheme,
                    selectAutoloop: connection.selectAutoloop,
                    setCueLock: connection.setCueLock
                )
            )
            .preferredColorScheme(.dark)
            .task {
                if scenePhase == .active {
                    UIApplication.shared.isIdleTimerDisabled = true
                    session.beginDiscovery()
                    discovery.start()
                }
            }
            .onChange(of: discovery.services) { _, services in
                connection.update(discoveredServices: services)
            }
            .onChange(of: discovery.state) { _, state in
                switch state {
                case .permissionRequired:
                    session.unavailable("Allow Local Network access for Lumi Remote in iPhone Settings.")
                case .failed:
                    session.unavailable("Lumi Remote could not browse the local network.")
                case .idle, .searching, .ready:
                    break
                }
            }
            .onOpenURL { url in
                connection.acceptPairingURL(url)
            }
            .onChange(of: scenePhase) { _, phase in
                switch phase {
                case .active:
                    UIApplication.shared.isIdleTimerDisabled = true
                    session.beginDiscovery()
                    discovery.start()
                case .background:
                    UIApplication.shared.isIdleTimerDisabled = false
                    connection.stop()
                    discovery.stop()
                case .inactive:
                    break
                @unknown default:
                    break
                }
            }
        }
    }

    private static var releaseChannel: RemoteReleaseChannel {
        guard let rawValue = Bundle.main.object(forInfoDictionaryKey: "LumiReleaseChannel") as? String,
              let channel = RemoteReleaseChannel(rawValue: rawValue) else {
            return .dev
        }
        return channel
    }
}
