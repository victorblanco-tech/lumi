import Foundation

public enum PlannedAutoloopStatus: String, Equatable, Sendable {
    case active
    case next
    case planned
    case completed
}

public struct PlannedAutoloopPresentation: Equatable, Identifiable, Sendable {
    public let phraseIndex: UInt64
    public let phraseName: String
    public let autoloopName: String
    public let bankNumber: UInt64?
    public let slotNumber: UInt64?
    public let staticLookName: String?
    public let status: PlannedAutoloopStatus
    public let locked: Bool
    public let holdsCurrentLook: Bool

    public var id: UInt64 { phraseIndex }
}

public enum PlannedAutoloopPresenter {
    public static func items(
        deck: DeckSnapshot,
        plan: PlanSnapshot?,
        isMaster: Bool,
        playheadBeat: Double? = nil
    ) -> [PlannedAutoloopPresentation] {
        guard let plan else { return [] }
        let statusBeat = UInt64(max(0, playheadBeat ?? Double(deck.beat)).rounded(.down))
        let firstUpcomingIndex = plan.cues.first(where: { cue in
            !isMaster || cue.startBeat > statusBeat
        })?.phraseIndex

        return plan.cues.map { cue in
            let phrase = deck.phrases.first(where: { $0.index == cue.phraseIndex })
            let phraseName = cue.libraryResolution?.roleName
                ?? phrase?.roleName
                ?? phrase?.kind.capitalized
                ?? "Phrase \(cue.phraseIndex + 1)"
            let action = actionDetails(cue.action)
            return PlannedAutoloopPresentation(
                phraseIndex: cue.phraseIndex,
                phraseName: phraseName,
                autoloopName: cue.libraryResolution?.entryName ?? action.name,
                bankNumber: cue.libraryResolution?.bankNumber ?? action.bank,
                slotNumber: cue.libraryResolution?.autoloopNumber ?? action.slot,
                staticLookName: cue.libraryResolution?.modifierChoices
                    .first(where: { $0.kind == "atmosphere" })?.name,
                status: status(
                    cue: cue,
                    isMaster: isMaster,
                    firstUpcomingIndex: firstUpcomingIndex,
                    statusBeat: statusBeat
                ),
                locked: cue.locked,
                holdsCurrentLook: action.holdsCurrentLook
            )
        }
    }

    private static func status(
        cue: PlanCueSnapshot,
        isMaster: Bool,
        firstUpcomingIndex: UInt64?,
        statusBeat: UInt64
    ) -> PlannedAutoloopStatus {
        guard isMaster else {
            return cue.phraseIndex == firstUpcomingIndex ? .next : .planned
        }
        if cue.endBeat <= statusBeat { return .completed }
        if cue.startBeat <= statusBeat { return .active }
        return cue.phraseIndex == firstUpcomingIndex ? .next : .planned
    }

    private static func actionDetails(
        _ action: PlanActionSnapshot
    ) -> (name: String, bank: UInt64?, slot: UInt64?, holdsCurrentLook: Bool) {
        switch action {
        case let .applyLook(_, _, _, sceneName, _, loopBank, loopSlot):
            (sceneName, loopBank, loopSlot, false)
        case .holdCurrentLook:
            ("Hold current look", nil, nil, true)
        }
    }
}
