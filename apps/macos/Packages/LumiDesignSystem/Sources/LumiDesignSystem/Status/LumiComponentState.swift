import SwiftUI

public enum LumiComponentState: String, CaseIterable, Equatable, Hashable, Sendable {
    case loading
    case empty
    case ready
    case stale
    case degraded
    case error

    public var systemImage: String {
        switch self {
        case .loading: "clock.arrow.circlepath"
        case .empty: "circle.dashed"
        case .ready: "checkmark.circle.fill"
        case .stale: "clock.badge.exclamationmark"
        case .degraded: "exclamationmark.triangle.fill"
        case .error: "xmark.octagon.fill"
        }
    }

    public var titleKey: LocalizedStringKey {
        switch self {
        case .loading: "design.state.loading"
        case .empty: "design.state.empty"
        case .ready: "design.state.ready"
        case .stale: "design.state.stale"
        case .degraded: "design.state.degraded"
        case .error: "design.state.error"
        }
    }

    public var color: Color {
        switch self {
        case .loading, .empty: LumiColor.textSecondary
        case .ready: LumiColor.success
        case .stale, .degraded: LumiColor.warning
        case .error: LumiColor.destructive
        }
    }
}
