import Testing
@testable import LumiLiveWorkspace

struct LiveSystemStatusTests {
    @Test func unstartedEngineNeverReportsReady() {
        #expect(LiveSystemStatus(engine: .empty, providers: [.empty]).label == "Stopped")
        #expect(LiveSystemStatus(engine: .loading, providers: [.empty]).label == "Starting")
        #expect(LiveSystemStatus(engine: .stale, providers: [.ready]).label == "Reconnecting")
        #expect(LiveSystemStatus(engine: .error, providers: [.ready]).label == "Unavailable")
    }

    @Test func optionalDisabledProvidersAreNotErrors() {
        let status = LiveSystemStatus(engine: .ready, providers: [.ready, .empty, .empty])
        #expect(status.label == "Ready")
        #expect(status.component == .ready)
    }

    @Test func pendingAndFailedProvidersHaveHonestStatus() {
        #expect(LiveSystemStatus(engine: .ready, providers: [.loading]).label == "Starting")
        for condition: ProviderCondition in [.stale, .degraded, .error] {
            let status = LiveSystemStatus(engine: .ready, providers: [condition])
            #expect(status.label == "Attention")
            #expect(status.component == .degraded)
        }
    }
}
