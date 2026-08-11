import AppKit
import LumiDesignSystem
import SwiftUI

/// Native AppKit table used by Local Playback. `NSTableView` owns row
/// selection, avoiding the competing SwiftUI gestures that made playlist
/// refreshes and row clicks unreliable.
struct LocalPlaybackTrackTable: NSViewRepresentable {
    let tracks: [LibraryTrack]
    let keyNotation: KeyNotationPreference
    @Binding var selection: UInt64?

    func makeCoordinator() -> Coordinator {
        Coordinator(selection: $selection)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let table = NSTableView()
        table.delegate = context.coordinator
        table.dataSource = context.coordinator
        table.headerView = NSTableHeaderView()
        table.rowHeight = 32
        table.intercellSpacing = NSSize(width: 8, height: 2)
        table.usesAlternatingRowBackgroundColors = true
        table.allowsEmptySelection = true
        table.allowsMultipleSelection = false
        table.allowsColumnReordering = true
        table.allowsColumnResizing = true
        table.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle
        table.autosaveName = "LumiLocalPlaybackTrackTable"
        table.autosaveTableColumns = true
        table.setAccessibilityIdentifier("lumi.localPlayback.trackTable")

        addColumn("title", title: "Track Title", width: 300, minimumWidth: 180, to: table)
        addColumn("artist", title: "Artist", width: 200, minimumWidth: 120, to: table)
        addColumn("bpm", title: "BPM", width: 64, minimumWidth: 55, to: table)
        addColumn("key", title: "Key", width: 54, minimumWidth: 44, to: table)
        addColumn("usbSources", title: "USB Sources", width: 150, minimumWidth: 110, to: table)
        addColumn("lumi", title: "Lumi", width: 58, minimumWidth: 48, to: table)

        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = true
        scrollView.backgroundColor = NSColor(calibratedWhite: 0.035, alpha: 1)
        scrollView.documentView = table

        context.coordinator.tableView = table
        context.coordinator.update(
            tracks: tracks,
            keyNotation: keyNotation,
            selection: selection
        )
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.selection = $selection
        context.coordinator.update(
            tracks: tracks,
            keyNotation: keyNotation,
            selection: selection
        )
    }

    private func addColumn(
        _ identifier: String,
        title: String,
        width: CGFloat,
        minimumWidth: CGFloat,
        to table: NSTableView
    ) {
        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier(identifier))
        column.title = title
        column.width = width
        column.minWidth = minimumWidth
        column.resizingMask = [.userResizingMask, .autoresizingMask]
        table.addTableColumn(column)
    }

    @MainActor
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        var selection: Binding<UInt64?>
        weak var tableView: NSTableView?
        private var tracks: [LibraryTrack] = []
        private var keyNotation: KeyNotationPreference = .camelot
        private var isApplyingSelection = false

        init(selection: Binding<UInt64?>) {
            self.selection = selection
        }

        func update(
            tracks: [LibraryTrack],
            keyNotation: KeyNotationPreference,
            selection: UInt64?
        ) {
            guard let tableView else { return }
            let contentChanged = self.tracks != tracks || self.keyNotation != keyNotation
            self.tracks = tracks
            self.keyNotation = keyNotation
            if contentChanged {
                isApplyingSelection = true
                tableView.reloadData()
                isApplyingSelection = false
            }
            applySelection(selection, to: tableView)
        }

        func numberOfRows(in tableView: NSTableView) -> Int {
            tracks.count
        }

        func tableView(
            _ tableView: NSTableView,
            viewFor tableColumn: NSTableColumn?,
            row: Int
        ) -> NSView? {
            guard tracks.indices.contains(row), let tableColumn else { return nil }
            let track = tracks[row]
            switch tableColumn.identifier.rawValue {
            case "title":
                let cell = titleCell(in: tableView)
                cell.configure(track)
                return cell
            case "artist":
                return textCell(track.artist, identifier: "artistCell", in: tableView)
            case "bpm":
                return textCell(
                    String(format: "%.1f", Double(track.bpmMilli) / 1_000),
                    identifier: "bpmCell",
                    monospaced: true,
                    in: tableView
                )
            case "key":
                return textCell(
                    KeyNotationFormatter(notation: keyNotation).string(from: track.musicalKey),
                    identifier: "keyCell",
                    monospaced: true,
                    in: tableView
                )
            case "usbSources":
                let value = track.usbSources.map(\.displayName).joined(separator: ", ")
                return textCell(
                    value.isEmpty ? "—" : value,
                    identifier: "usbSourcesCell",
                    in: tableView
                )
            case "lumi":
                let value = track.timelineRevision.map { "R\($0)" } ?? "—"
                let color = track.timelineRevision == nil ? NSColor.systemOrange : NSColor.systemGreen
                return textCell(
                    value,
                    identifier: "lumiCell",
                    color: color,
                    monospaced: true,
                    in: tableView
                )
            default:
                return nil
            }
        }

        func tableViewSelectionDidChange(_ notification: Notification) {
            guard !isApplyingSelection,
                  let tableView,
                  tracks.indices.contains(tableView.selectedRow) else {
                return
            }
            selection.wrappedValue = tracks[tableView.selectedRow].id
        }

        private func applySelection(_ selectedID: UInt64?, to tableView: NSTableView) {
            isApplyingSelection = true
            defer { isApplyingSelection = false }
            guard let selectedID,
                  let row = tracks.firstIndex(where: { $0.id == selectedID }) else {
                tableView.deselectAll(nil)
                return
            }
            guard tableView.selectedRow != row else { return }
            tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
        }

        private func titleCell(in tableView: NSTableView) -> TrackTitleCellView {
            let identifier = NSUserInterfaceItemIdentifier("titleCell")
            if let cell = tableView.makeView(withIdentifier: identifier, owner: nil) as? TrackTitleCellView {
                return cell
            }
            let cell = TrackTitleCellView()
            cell.identifier = identifier
            return cell
        }

        private func textCell(
            _ value: String,
            identifier: String,
            color: NSColor = .labelColor,
            monospaced: Bool = false,
            in tableView: NSTableView
        ) -> NSTableCellView {
            let viewIdentifier = NSUserInterfaceItemIdentifier(identifier)
            let cell = (tableView.makeView(withIdentifier: viewIdentifier, owner: nil) as? NSTableCellView)
                ?? makeTextCell(identifier: viewIdentifier)
            cell.textField?.stringValue = value
            cell.textField?.textColor = color
            cell.textField?.font = monospaced
                ? NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular)
                : NSFont.systemFont(ofSize: 12)
            return cell
        }

        private func makeTextCell(identifier: NSUserInterfaceItemIdentifier) -> NSTableCellView {
            let cell = NSTableCellView()
            cell.identifier = identifier
            let label = NSTextField(labelWithString: "")
            label.lineBreakMode = .byTruncatingTail
            label.translatesAutoresizingMaskIntoConstraints = false
            cell.textField = label
            cell.addSubview(label)
            NSLayoutConstraint.activate([
                label.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 4),
                label.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -4),
                label.centerYAnchor.constraint(equalTo: cell.centerYAnchor)
            ])
            return cell
        }
    }
}

private final class TrackTitleCellView: NSTableCellView {
    private let swatch = NSView()
    private let label = NSTextField(labelWithString: "")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)

        swatch.wantsLayer = true
        swatch.layer?.cornerRadius = 2
        swatch.translatesAutoresizingMaskIntoConstraints = false

        label.font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        label.lineBreakMode = .byTruncatingTail
        label.translatesAutoresizingMaskIntoConstraints = false
        textField = label

        addSubview(swatch)
        addSubview(label)
        NSLayoutConstraint.activate([
            swatch.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            swatch.centerYAnchor.constraint(equalTo: centerYAnchor),
            swatch.widthAnchor.constraint(equalToConstant: 6),
            swatch.heightAnchor.constraint(equalToConstant: 18),
            label.leadingAnchor.constraint(equalTo: swatch.trailingAnchor, constant: 8),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            label.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(_ track: LibraryTrack) {
        label.stringValue = track.title
        if let rgb = track.colorRGB {
            swatch.layer?.backgroundColor = NSColor(
                srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255,
                green: CGFloat((rgb >> 8) & 0xFF) / 255,
                blue: CGFloat(rgb & 0xFF) / 255,
                alpha: 1
            ).cgColor
        } else {
            swatch.layer?.backgroundColor = NSColor.darkGray.cgColor
        }
    }
}
