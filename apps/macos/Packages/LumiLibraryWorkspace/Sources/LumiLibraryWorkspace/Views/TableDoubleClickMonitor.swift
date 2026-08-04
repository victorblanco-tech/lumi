import AppKit
import SwiftUI

/// Observes native double-clicks without taking hit testing away from the
/// underlying SwiftUI Table. AppKit consumes row clicks before a table-level
/// SwiftUI TapGesture can reliably see them.
struct TableDoubleClickMonitor: NSViewRepresentable {
    let onDoubleClick: @MainActor () -> Void

    func makeNSView(context: Context) -> TableDoubleClickMonitorView {
        let view = TableDoubleClickMonitorView()
        view.onDoubleClick = onDoubleClick
        return view
    }

    func updateNSView(_ nsView: TableDoubleClickMonitorView, context: Context) {
        nsView.onDoubleClick = onDoubleClick
    }

    static func dismantleNSView(_ nsView: TableDoubleClickMonitorView, coordinator: ()) {
        nsView.removeEventMonitor()
    }
}

final class TableDoubleClickMonitorView: NSView {
    var onDoubleClick: (@MainActor () -> Void)?
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
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseUp) { [weak self] event in
            guard let self,
                  event.window === self.window,
                  event.clickCount == 2
            else { return event }

            let point = self.convert(event.locationInWindow, from: nil)
            guard self.bounds.contains(point) else { return event }
            self.onDoubleClick?()
            return event
        }
    }
}
