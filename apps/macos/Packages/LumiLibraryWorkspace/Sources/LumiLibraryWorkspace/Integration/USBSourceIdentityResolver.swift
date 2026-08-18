import Foundation

struct MountedUSBIdentity: Equatable, Sendable {
    let sourceID: String?
    let displayName: String
}

enum USBSourceIdentityResolver {
    static func selectedSourceID(
        for volume: MountedUSBIdentity,
        devices: [RekordboxDeviceState]
    ) -> String? {
        if let sourceID = volume.sourceID,
           devices.contains(where: { $0.sourceID == sourceID }) {
            return sourceID
        }
        if let legacy = devices.first(where: {
            isLegacy($0.sourceID) && namesMatch($0.displayName, volume.displayName)
        }) {
            return legacy.sourceID
        }
        return volume.sourceID
    }

    static func volume(
        _ volume: MountedUSBIdentity,
        matches device: RekordboxDeviceState
    ) -> Bool {
        if isStable(device.sourceID) {
            return volume.sourceID == device.sourceID
        }
        return namesMatch(volume.displayName, device.displayName)
    }

    static func inspection(
        _ inspection: RekordboxDeviceInspectionState,
        matches device: RekordboxDeviceState
    ) -> Bool {
        if isStable(device.sourceID) {
            return inspection.sourceID == device.sourceID
        }
        return inspection.sourceID == device.sourceID
            || namesMatch(inspection.displayName, device.displayName)
    }

    static func displayName(
        for device: RekordboxDeviceState,
        inspection: RekordboxDeviceInspectionState?
    ) -> String {
        guard let inspection, Self.inspection(inspection, matches: device) else {
            return device.displayName
        }
        return inspection.displayName
    }

    static func isStable(_ sourceID: String) -> Bool {
        sourceID.hasPrefix("usb-fs:")
    }

    static func isLegacy(_ sourceID: String) -> Bool {
        sourceID.hasPrefix("usb-volume:") || sourceID.hasPrefix("rekordbox-device:")
    }

    private static func namesMatch(_ lhs: String, _ rhs: String) -> Bool {
        lhs.caseInsensitiveCompare(rhs) == .orderedSame
    }
}
