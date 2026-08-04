import AppKit
import SwiftUI

/// Adds native trackpad and horizontal mouse-wheel panning without taking over
/// pointer hit testing from the SwiftUI waveform gestures underneath it.
struct HorizontalScrollMonitor: NSViewRepresentable {
    let onScroll: @MainActor (Double) -> Void

    func makeNSView(context: Context) -> HorizontalScrollMonitorView {
        let view = HorizontalScrollMonitorView()
        view.onScroll = onScroll
        return view
    }

    func updateNSView(_ nsView: HorizontalScrollMonitorView, context: Context) {
        nsView.onScroll = onScroll
    }

    static func dismantleNSView(_ nsView: HorizontalScrollMonitorView, coordinator: ()) {
        nsView.removeEventMonitor()
    }
}

final class HorizontalScrollMonitorView: NSView {
    var onScroll: (@MainActor (Double) -> Void)?
    private var eventMonitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            removeEventMonitor()
        } else {
            installEventMonitorIfNeeded()
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    func removeEventMonitor() {
        if let eventMonitor {
            NSEvent.removeMonitor(eventMonitor)
            self.eventMonitor = nil
        }
    }

    private func installEventMonitorIfNeeded() {
        guard eventMonitor == nil else { return }
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
            guard let self,
                  event.window === self.window,
                  abs(event.scrollingDeltaX) > abs(event.scrollingDeltaY),
                  abs(event.scrollingDeltaX) > 0.01
            else { return event }

            let point = self.convert(event.locationInWindow, from: nil)
            guard self.bounds.contains(point) else { return event }
            self.onScroll?(event.scrollingDeltaX)
            return nil
        }
    }
}
