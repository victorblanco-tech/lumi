import LumiDesignSystem

struct LiveSystemStatus: Equatable {
    let label: String
    let component: LumiComponentState

    init(engine: ProviderCondition, providers: [ProviderCondition]) {
        switch engine {
        case .empty: self = .init(label: "Stopped", component: .empty)
        case .loading: self = .init(label: "Starting", component: .loading)
        case .stale: self = .init(label: "Reconnecting", component: .stale)
        case .error: self = .init(label: "Unavailable", component: .error)
        case .degraded: self = .init(label: "Attention", component: .degraded)
        case .ready:
            if providers.contains(where: { [.error, .degraded, .stale].contains($0) }) {
                self = .init(label: "Attention", component: .degraded)
            } else if providers.contains(.loading) {
                self = .init(label: "Starting", component: .loading)
            } else {
                // Empty/disabled optional providers and unloaded Players are
                // not faults, but an unstarted engine must never look ready.
                self = .init(label: "Ready", component: .ready)
            }
        }
    }

    private init(label: String, component: LumiComponentState) {
        self.label = label
        self.component = component
    }
}
