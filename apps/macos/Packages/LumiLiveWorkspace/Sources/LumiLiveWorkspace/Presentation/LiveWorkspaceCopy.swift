import Foundation

struct LiveWorkspaceCopy {
    let appTitle = localized("app.title")
    let subtitle = localized("workspace.subtitle")
    let live = localized("navigation.live")
    let plans = localized("navigation.plans")
    let library = localized("navigation.library")
    let integrations = localized("navigation.integrations")
    let settings = localized("navigation.settings")
    let comingSoon = localized("navigation.comingSoon")
    let appearance = localized("preference.appearance")
    let keyNotation = localized("preference.keyNotation")
    let engine = localized("provider.engine")
    let runtime = localized("provider.runtime")
    let deckSource = localized("provider.deckSource")
    let planner = localized("provider.planner")
    let outputProvider = localized("provider.output")
    let liveDeck = localized("deck.live")
    let nextDeck = localized("deck.next")
    let bpm = localized("deck.bpm")
    let key = localized("deck.key")
    let liveDeckSource = localized("workspace.deckSource")
    let nextPlan = localized("workspace.nextPlan")
    let phrasePlan = localized("workspace.phrasePlan")
    let inspector = localized("workspace.inspector")
    let theme = localized("workspace.theme")
    let themeSource = localized("workspace.themeSource")
    let scene = localized("workspace.scene")
    let reason = localized("workspace.reason")
    let origin = localized("workspace.origin")
    let librarySource = localized("workspace.librarySource")
    let lumiTimeline = localized("workspace.lumiTimeline")
    let phraseRole = localized("workspace.phraseRole")
    let loopStrategy = localized("workspace.loopStrategy")
    let loopVariant = localized("workspace.loopVariant")
    let dryRunEntry = localized("workspace.dryRunEntry")
    let regenerate = localized("workspace.regenerate")
    let lockCue = localized("workspace.lockCue")
    let unlockCue = localized("workspace.unlockCue")
    let savingPlan = localized("workspace.savingPlan")
    let applyingCommand = localized("workspace.applyingCommand")
    let waitingDecks = localized("workspace.waitingDecks")
    let waitingPlan = localized("workspace.waitingPlan")
    let waitingTimeline = localized("workspace.waitingTimeline")
    let waitingSimulator = localized("workspace.waitingSimulator")
    let demoSession = localized("simulator.title")
    let loadDemo = localized("simulator.load")
    let pauseDemo = localized("simulator.pause")
    let resumeDemo = localized("simulator.resume")
    let nextTrack = localized("simulator.nextTrack")
    let resetDemo = localized("simulator.reset")
    let timeline = localized("timeline.title")
    let speed = localized("simulator.speed")
    let paused = localized("simulator.paused")
    let playing = localized("simulator.playing")
    let arm = localized("operation.arm")
    let start = localized("operation.start")
    let pause = localized("operation.pause")
    let off = localized("operation.off")
    let ready = localized("state.ready")
    let loading = localized("state.loading")
    let empty = localized("state.empty")
    let fallback = localized("state.fallback")
    let stale = localized("state.stale")
    let degraded = localized("state.degraded")
    let disconnected = localized("state.disconnected")
    let error = localized("state.error")
    let unavailable = localized("value.unavailable")
    let hold = localized("cue.hold")

    func phrase(_ kind: String) -> String {
        Self.localized("phrase.\(kind)")
    }

    func category(_ category: String) -> String {
        Self.localized("category.\(category)")
    }

    func themeReason(_ reason: String) -> String {
        Self.localized("themeReason.\(reason)")
    }

    private static func localized(_ key: String) -> String {
        String(localized: String.LocalizationValue(key), bundle: .module)
    }
}

private func localized(_ key: String) -> String {
    String(localized: String.LocalizationValue(key), bundle: .module)
}
