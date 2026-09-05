import AppKit
import LumiDesignSystem
import SwiftUI

@main
struct LumiApp: App {
    @NSApplicationDelegateAdaptor(LumiApplicationDelegate.self)
    private var applicationDelegate
    @StateObject private var engineStatus: EngineStatusModel
    @State private var preferences = LumiPreferences()

    init() {
        let engineStatus = EngineStatusModel()
        _engineStatus = StateObject(wrappedValue: engineStatus)
        applicationDelegate.shutdown = {
            await engineStatus.stop()
        }
    }

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
                .onChange(of: engineStatus.lightingTimingSettings?.savedTimingOffsetMillis) { _, millis in
                    if let millis {
                        // Mirror only: persisting a Remote edit must not echo a
                        // command back into the live scheduling lane.
                        preferences.lightingTimingOffsetMillis = millis
                    }
                }
                .task {
                    await engineStatus.start()
                    if let midi = engineStatus.lightingTimingSettings {
                        if let saved = midi.savedTimingOffsetMillis {
                            preferences.lightingTimingOffsetMillis = saved
                        } else if !midi.timingSavePending && midi.timingSaveError == nil {
                            // One-time migration of the previous Mac preference.
                            await engineStatus.setLightingTimingOffset(preferences.lightingTimingOffsetMillis)
                        }
                    }
                    if preferences.abletonLinkAutoStart {
                        await engineStatus.setAbletonLinkEnabled(true)
                    }
                }
        }
        .defaultSize(width: 1_280, height: 820)
    }
}

@MainActor
final class LumiApplicationDelegate: NSObject, NSApplicationDelegate {
    var shutdown: (() async -> Void)?
    private var terminationInProgress = false

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminationInProgress, let shutdown else {
            return .terminateNow
        }
        terminationInProgress = true
        Task { @MainActor in
            await shutdown()
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
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
