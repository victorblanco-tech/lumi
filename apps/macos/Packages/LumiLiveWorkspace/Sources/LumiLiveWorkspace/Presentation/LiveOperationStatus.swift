import LumiDesignSystem
import SwiftUI

enum LiveOperationStatus: String, CaseIterable, Sendable {
    case off
    case armed
    case live
    case paused

    init(engineState: String) {
        self = Self(rawValue: engineState) ?? .off
    }

    var color: Color {
        switch self {
        case .off: Color.white
        case .armed, .paused: LumiColor.warning
        case .live: LumiColor.destructive
        }
    }

    var pulses: Bool {
        self == .paused
    }

    func showsLiveNow(isPlaying: Bool) -> Bool {
        self == .live && isPlaying
    }
}
