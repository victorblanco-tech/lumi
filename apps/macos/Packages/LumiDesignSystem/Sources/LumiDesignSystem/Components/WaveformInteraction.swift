import SwiftUI

#if canImport(AppKit)
import AppKit
#endif

public enum LumiWaveformZoomAnchor: String, CaseIterable, Identifiable, Sendable {
    case mouse
    case playhead

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .mouse: "Mouse pointer"
        case .playhead: "Playhead"
        }
    }
}

/// Continuous beat-space viewport shared by the Track Editor and Live decks.
/// Navigation remains independent from phrase mutation quantization.
public struct LumiWaveformViewport: Equatable, Sendable {
    public let startBeat: Double
    public let visibleBeats: Double
    public let totalBeats: UInt64
    public let beatsPerBar: UInt8

    public init(
        startBeat: Double,
        visibleBeats: Double,
        totalBeats: UInt64,
        beatsPerBar: UInt8 = 4
    ) {
        let safeTotal = max(1, totalBeats)
        let safeVisible = min(max(1, visibleBeats), Double(safeTotal))
        self.startBeat = min(max(0, startBeat), Double(safeTotal) - safeVisible)
        self.visibleBeats = safeVisible
        self.totalBeats = safeTotal
        self.beatsPerBar = max(1, beatsPerBar)
    }

    public var endBeat: Double { startBeat + visibleBeats }
    public var visibleBars: Double { visibleBeats / Double(beatsPerBar) }

    public func x(forBeat beat: Double, width: Double) -> Double {
        (beat - startBeat) / visibleBeats * max(0, width)
    }

    public func beat(atX x: Double, width: Double) -> Double {
        guard width > 0 else { return startBeat }
        return startBeat + min(max(0, x / width), 1) * visibleBeats
    }

    public func moving(byBeats delta: Double) -> Self {
        Self(
            startBeat: startBeat + delta,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            beatsPerBar: beatsPerBar
        )
    }

    public func panned(byPixels delta: Double, width: Double) -> Self {
        guard width > 0 else { return self }
        return moving(byBeats: delta / width * visibleBeats)
    }

    public func centered(onBeat beat: Double) -> Self {
        Self(
            startBeat: beat - visibleBeats / 2,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            beatsPerBar: beatsPerBar
        )
    }

    public func zoomed(to beats: Double, aroundBeat beat: Double) -> Self {
        let anchor = visibleBeats > 0 ? (beat - startBeat) / visibleBeats : 0.5
        let boundedAnchor = min(max(0, anchor), 1)
        return Self(
            startBeat: beat - boundedAnchor * beats,
            visibleBeats: beats,
            totalBeats: totalBeats,
            beatsPerBar: beatsPerBar
        )
    }
}

public struct LumiWaveformZoomControls: View {
    @Binding private var zoom: Double
    @Binding private var zoomAnchor: LumiWaveformZoomAnchor
    @Binding private var reversesHorizontalScroll: Bool
    private let visibleBars: Double
    private let sliderWidth: CGFloat
    private let accessibilityPrefix: String

    public init(
        zoom: Binding<Double>,
        visibleBars: Double,
        zoomAnchor: Binding<LumiWaveformZoomAnchor>,
        reversesHorizontalScroll: Binding<Bool>,
        sliderWidth: CGFloat = 150,
        accessibilityPrefix: String
    ) {
        _zoom = zoom
        self.visibleBars = visibleBars
        _zoomAnchor = zoomAnchor
        _reversesHorizontalScroll = reversesHorizontalScroll
        self.sliderWidth = sliderWidth
        self.accessibilityPrefix = accessibilityPrefix
    }

    public var body: some View {
        HStack(spacing: LumiSpacing.small) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(LumiColor.textSecondary)
            Slider(value: $zoom, in: 0...1)
                .frame(width: sliderWidth)
                .accessibilityLabel("Waveform zoom")
                .accessibilityValue(String(format: "Visible %.1f bars", visibleBars))
            Text(String(format: "%.1f bars", visibleBars))
                .font(LumiTypography.technical.weight(.semibold))
                .frame(width: 62, alignment: .trailing)
            Menu {
                Picker("Zoom around", selection: $zoomAnchor) {
                    ForEach(LumiWaveformZoomAnchor.allCases) { anchor in
                        Text(anchor.title).tag(anchor)
                    }
                }
                Divider()
                Toggle("Reverse horizontal scroll", isOn: $reversesHorizontalScroll)
            } label: {
                Image(systemName: "slider.horizontal.3")
            }
            #if os(macOS)
            .menuStyle(.borderlessButton)
            #endif
            .fixedSize()
            .help("Waveform mouse and trackpad settings")
            .accessibilityLabel("Waveform interaction settings")
            .accessibilityIdentifier("\(accessibilityPrefix).interactionSettings")
        }
        .accessibilityIdentifier("\(accessibilityPrefix).zoom")
    }
}

/// Captures vertical wheel zoom and horizontal trackpad/mouse panning without
/// taking pointer hit testing away from the waveform underneath it.
#if os(macOS)
public struct LumiWaveformInteractionMonitor: NSViewRepresentable {
    private let onScroll: @MainActor (Double) -> Void
    private let onZoom: @MainActor (_ delta: Double, _ pointerFraction: Double) -> Void

    public init(
        onScroll: @escaping @MainActor (Double) -> Void,
        onZoom: @escaping @MainActor (_ delta: Double, _ pointerFraction: Double) -> Void
    ) {
        self.onScroll = onScroll
        self.onZoom = onZoom
    }

    public func makeNSView(context: Context) -> LumiWaveformInteractionMonitorView {
        let view = LumiWaveformInteractionMonitorView()
        view.onScroll = onScroll
        view.onZoom = onZoom
        return view
    }

    public func updateNSView(_ nsView: LumiWaveformInteractionMonitorView, context: Context) {
        nsView.onScroll = onScroll
        nsView.onZoom = onZoom
    }

    public static func dismantleNSView(
        _ nsView: LumiWaveformInteractionMonitorView,
        coordinator: ()
    ) {
        nsView.removeEventMonitor()
    }
}

public final class LumiWaveformInteractionMonitorView: NSView {
    var onScroll: (@MainActor (Double) -> Void)?
    var onZoom: (@MainActor (_ delta: Double, _ pointerFraction: Double) -> Void)?
    private var eventMonitor: Any?

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            removeEventMonitor()
        } else {
            installEventMonitorIfNeeded()
        }
    }

    public override func hitTest(_ point: NSPoint) -> NSView? { nil }

    func removeEventMonitor() {
        if let eventMonitor {
            NSEvent.removeMonitor(eventMonitor)
            self.eventMonitor = nil
        }
    }

    private func installEventMonitorIfNeeded() {
        guard eventMonitor == nil else { return }
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
            guard let self, event.window === self.window else { return event }
            let point = self.convert(event.locationInWindow, from: nil)
            guard self.bounds.contains(point) else { return event }
            if abs(event.scrollingDeltaY) >= abs(event.scrollingDeltaX),
               abs(event.scrollingDeltaY) > 0.01 {
                let fraction = min(max(0, point.x / max(1, self.bounds.width)), 1)
                self.onZoom?(event.scrollingDeltaY, fraction)
            } else if abs(event.scrollingDeltaX) > 0.01 {
                self.onScroll?(event.scrollingDeltaX)
            } else {
                return event
            }
            return nil
        }
    }
}

/// Window-local shortcut that keeps working after controls receive focus. Text
/// editing gets priority, so a space in Search never starts playback.
public struct LumiSpacebarMonitor: NSViewRepresentable {
    private let onSpace: @MainActor () -> Void

    public init(onSpace: @escaping @MainActor () -> Void) {
        self.onSpace = onSpace
    }

    public func makeNSView(context: Context) -> LumiSpacebarMonitorView {
        let view = LumiSpacebarMonitorView()
        view.onSpace = onSpace
        return view
    }

    public func updateNSView(_ nsView: LumiSpacebarMonitorView, context: Context) {
        nsView.onSpace = onSpace
    }

    public static func dismantleNSView(_ nsView: LumiSpacebarMonitorView, coordinator: ()) {
        nsView.removeEventMonitor()
    }
}

public final class LumiSpacebarMonitorView: NSView {
    var onSpace: (@MainActor () -> Void)?
    private var eventMonitor: Any?

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            removeEventMonitor()
        } else {
            installEventMonitorIfNeeded()
        }
    }

    public override func hitTest(_ point: NSPoint) -> NSView? { nil }

    func removeEventMonitor() {
        if let eventMonitor {
            NSEvent.removeMonitor(eventMonitor)
            self.eventMonitor = nil
        }
    }

    private func installEventMonitorIfNeeded() {
        guard eventMonitor == nil else { return }
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self,
                  event.window === self.window,
                  event.keyCode == 49,
                  event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
                  !Self.isEditingText(in: event.window) else {
                return event
            }
            self.onSpace?()
            return nil
        }
    }

    private static func isEditingText(in window: NSWindow?) -> Bool {
        guard let responder = window?.firstResponder else { return false }
        if let textView = responder as? NSTextView {
            return textView.isEditable
        }
        return responder is NSTextField
    }
}
#endif
