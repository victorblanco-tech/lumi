enum LiveWorkspaceNoticeTone: Equatable, Sendable {
    case neutral
    case working
    case success
    case warning
    case error
}

struct LiveWorkspaceNoticePresentation: Equatable, Sendable {
    let message: String
    let tone: LiveWorkspaceNoticeTone
}

enum LiveWorkspaceNoticePresenter {
    static func notice(
        state: LiveWorkspaceState,
        localPlaybackFeedback: String?,
        localPlaybackFeedbackIsError: Bool
    ) -> LiveWorkspaceNoticePresentation {
        if case let .rejected(message) = state.planInteraction {
            return .init(message: message, tone: .warning)
        }
        if case let .rejected(message) = state.sessionInteraction {
            return .init(message: message, tone: .warning)
        }
        if state.planInteraction == .submitting {
            return .init(message: "Saving lighting plan…", tone: .working)
        }
        if state.sessionInteraction == .submitting {
            return .init(message: "Applying deck command…", tone: .working)
        }
        if case let .succeeded(message) = state.planInteraction {
            return .init(message: message, tone: .success)
        }
        if case let .succeeded(message) = state.sessionInteraction {
            return .init(message: message, tone: .success)
        }
        if let localPlaybackFeedback, !localPlaybackFeedback.isEmpty {
            return .init(
                message: localPlaybackFeedback,
                tone: localPlaybackFeedbackIsError ? .error : .success
            )
        }
        if let diagnostic = state.diagnostic, !diagnostic.isEmpty {
            return .init(message: diagnostic, tone: diagnosticTone(for: state.condition))
        }
        return switch state.condition {
        case .ready:
            .init(message: "Live workspace ready", tone: .success)
        case .loading:
            .init(message: state.engine.detail, tone: .working)
        case .empty:
            .init(message: state.engine.detail, tone: .neutral)
        case .fallback, .stale, .degraded:
            .init(message: state.engine.detail, tone: .warning)
        case .disconnected, .error:
            .init(message: state.engine.detail, tone: .error)
        }
    }

    private static func diagnosticTone(
        for condition: LiveWorkspaceCondition
    ) -> LiveWorkspaceNoticeTone {
        switch condition {
        case .ready:
            .success
        case .loading:
            .working
        case .empty:
            .neutral
        case .fallback, .stale, .degraded:
            .warning
        case .disconnected, .error:
            .error
        }
    }
}
