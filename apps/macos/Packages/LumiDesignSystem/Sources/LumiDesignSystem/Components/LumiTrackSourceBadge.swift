import SwiftUI

/// Presentation of a loaded track's origin, independent of the mounted USB.
public struct LumiTrackSourceBadge: View {
    let state: String
    let sourceName: String?
    let colorID: UInt8?
    let sourcePlayer: UInt8?
    let loadedPlayer: UInt8
    let slot: String?

    public init(state: String, sourceName: String?, colorID: UInt8?,
                sourcePlayer: UInt8?, loadedPlayer: UInt8, slot: String?) {
        self.state = state
        self.sourceName = sourceName
        self.colorID = state == "trusted" ? colorID : nil
        self.sourcePlayer = sourcePlayer
        self.loadedPlayer = loadedPlayer
        self.slot = slot
    }

    nonisolated public static func label(state: String, sourceName: String?,
                                        sourcePlayer: UInt8?, loadedPlayer: UInt8,
                                        slot: String?) -> String {
        let origin: String
        switch slot {
        case "USB_SLOT":
            origin = switch state {
            case "trusted": sourceName ?? "Identified USB"
            case "conflict": "USB identity conflict"
            case "resolving": "Identifying USB…"
            default: "USB not identified"
            }
        case "SD_SLOT": origin = "SD"
        case "CD_SLOT": origin = "CD"
        case "COLLECTION": origin = "rekordbox"
        default: origin = "Not identified"
        }
        let route: String
        if let sourcePlayer {
            route = sourcePlayer == loadedPlayer
                ? " · local" : " · via Player \(sourcePlayer) / LINK"
        } else {
            route = ""
        }
        return "Source · \(origin)\(route)"
    }

    public var body: some View {
        let label = Self.label(state: state, sourceName: sourceName,
                               sourcePlayer: sourcePlayer, loadedPlayer: loadedPlayer, slot: slot)
        let rgb = LumiPlayerUSBBadge.colorRGB(for: colorID)
        let tint = rgb.map {
            Color(red: Double(($0 >> 16) & 255) / 255,
                  green: Double(($0 >> 8) & 255) / 255, blue: Double($0 & 255) / 255)
        } ?? LumiColor.textSecondary
        HStack(spacing: 4) {
            Image(systemName: "arrow.down.doc.fill").foregroundStyle(tint)
            Text(label)
                .foregroundStyle(LumiColor.textSecondary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .font(LumiTypography.caption)
        .frame(height: 16, alignment: .leading)
        .help(label)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
    }
}
