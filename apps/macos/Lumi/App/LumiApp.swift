import AppKit
import LumiDesignSystem
import SwiftUI

@main
struct LumiApp: App {
    @StateObject private var engineStatus = EngineStatusModel()
    @State private var preferences = LumiPreferences()

    var body: some Scene {
        WindowGroup {
            FoundationView(
                engineStatus: engineStatus,
                preferences: preferences
            )
                .background(MacWindowConfigurator())
                .onAppear {
                    MacApplicationAppearance.apply(preferences.appearance)
                }
                .onChange(of: preferences.appearance) { _, appearance in
                    MacApplicationAppearance.apply(appearance)
                }
                .onChange(of: preferences.lightingTimingOffsetMillis) { _, millis in
                    Task {
                        await engineStatus.setLightingTimingOffset(millis)
                    }
                }
                .task {
                    await engineStatus.start()
                    await engineStatus.setLightingTimingOffset(
                        preferences.lightingTimingOffsetMillis
                    )
                    if preferences.abletonLinkAutoStart {
                        await engineStatus.setAbletonLinkEnabled(true)
                    }
                }
        }
        .defaultSize(width: 1_280, height: 820)
    }
}

@MainActor
private enum MacApplicationWindow {
    static func configure(_ window: NSWindow) {
        window.contentMinSize = NSSize(width: 1_180, height: 620)
        disableAutomaticHostingSizeMeasurements(in: window.contentViewController)
        disableAutomaticHostingSizeMeasurements(in: window.contentView)
    }

    private static func disableAutomaticHostingSizeMeasurements(
        in viewController: NSViewController?
    ) {
        guard let viewController else { return }
        (viewController as? any MacHostingSizingConfigurable)?.disableAutomaticSizing()
        for child in viewController.children {
            disableAutomaticHostingSizeMeasurements(in: child)
        }
    }

    private static func disableAutomaticHostingSizeMeasurements(in view: NSView?) {
        guard let view else { return }
        (view as? any MacHostingSizingConfigurable)?.disableAutomaticSizing()
        for subview in view.subviews {
            disableAutomaticHostingSizeMeasurements(in: subview)
        }
    }
}

private struct MacWindowConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> WindowConfigurationView {
        WindowConfigurationView()
    }

    func updateNSView(_ nsView: WindowConfigurationView, context: Context) {
        nsView.configureAttachedWindowIfPossible()
    }

    @MainActor
    final class WindowConfigurationView: NSView {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            configureAttachedWindowIfPossible()
            // The representable is already inside the hosting hierarchy, but
            // AppKit may not have linked every superview until the next turn.
            Task { @MainActor [weak self] in
                self?.configureAttachedWindowIfPossible()
            }
        }

        fileprivate func configureAttachedWindowIfPossible() {
            guard let window else { return }
            MacApplicationWindow.configure(window)
        }
    }
}

@MainActor
private protocol MacHostingSizingConfigurable: AnyObject {
    func disableAutomaticSizing()
}

extension NSHostingView: MacHostingSizingConfigurable {
    fileprivate func disableAutomaticSizing() {
        // The NSWindow owns Lumi's explicit minimum size. Asking the root
        // hosting view to continuously derive min/ideal/max sizes from the
        // large Live hierarchy creates a macOS 26 layout feedback loop.
        if !sizingOptions.isEmpty {
            sizingOptions = []
        }
    }
}

extension NSHostingController: MacHostingSizingConfigurable {
    fileprivate func disableAutomaticSizing() {
        if !sizingOptions.isEmpty {
            sizingOptions = []
        }
    }
}

@MainActor
private enum MacApplicationAppearance {
    static func apply(_ preference: AppearancePreference) {
        NSApplication.shared.appearance = switch preference {
        case .dark:
            NSAppearance(named: .darkAqua)
        case .light:
            NSAppearance(named: .aqua)
        case .system:
            nil
        }
    }
}
