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
#if LUMI_DOCUMENTATION_CAPTURE
        .commands {
            CommandGroup(after: .saveItem) {
                Button("Export Retina Documentation Image") {
                    MacApplicationWindow.exportDocumentationImage()
                }
            }
        }
#endif
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
#if LUMI_DOCUMENTATION_CAPTURE
    // Opt-in local documentation build only. Exports this application's own
    // rendered content, not the desktop or other applications' windows.
    static func exportDocumentationImage() {
        guard let window = NSApp.mainWindow, let contentView = window.contentView else { return }
        func editorSplit(in view: NSView) -> NSSplitView? {
            if let split = view as? NSSplitView, !split.isVertical { return split }
            return view.subviews.lazy.compactMap { editorSplit(in: $0) }.first
        }
        if let split = editorSplit(in: contentView) {
            let preference = UserDefaults.standard.double(
                forKey: "co.victorblan.tech.lumi.library.editor-split.preferredEditorHeight"
            )
            if preference > 0 { split.setPosition(preference, ofDividerAt: 0) }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            writeDocumentationImage(window: window)
        }
    }

    private static func writeDocumentationImage(window: NSWindow) {
        guard let contentView = window.contentView else { return }
        let bounds = contentView.bounds
        let scale = window.backingScaleFactor
        guard let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: Int((bounds.width * scale).rounded()),
            pixelsHigh: Int((bounds.height * scale).rounded()),
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
        ) else { return }
        bitmap.size = bounds.size
        contentView.cacheDisplay(in: bounds, to: bitmap)
        guard let data = bitmap.representation(using: .png, properties: [:]) else { return }
        do {
            try data.write(to: URL(fileURLWithPath: "/tmp/lumi-editor-retina.png"), options: .atomic)
        } catch {
            NSAlert(error: error).runModal()
        }
    }
#endif

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
