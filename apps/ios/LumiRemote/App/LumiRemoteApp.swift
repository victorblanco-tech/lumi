import LumiRemoteClient
import LumiRemoteFeature
import SwiftUI

@main
struct LumiRemoteApp: App {
    @State private var session = RemoteSessionModel()
    @StateObject private var discovery = BonjourRemoteDiscovery(
        releaseChannel: LumiRemoteApp.releaseChannel
    )

    var body: some Scene {
        WindowGroup {
            RemoteLiveView(
                model: session,
                actions: RemoteLiveActions(
                    setOperationState: { _ in },
                    setAbletonLinkEnabled: { _ in },
                    setTimingOffset: { _ in },
                    selectTheme: { _, _, _ in },
                    selectAutoloop: { _, _, _ in },
                    setCueLock: { _, _, _ in }
                )
            )
            .task {
                session.beginDiscovery()
                discovery.start()
            }
            .onChange(of: discovery.services) { _, services in
                if services.isEmpty {
                    session.beginDiscovery()
                } else {
                    // A discovered Mac exposes no show state. Explicit Mac
                    // approval and pinned TLS still precede connection.
                    session.beginPairing()
                }
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
