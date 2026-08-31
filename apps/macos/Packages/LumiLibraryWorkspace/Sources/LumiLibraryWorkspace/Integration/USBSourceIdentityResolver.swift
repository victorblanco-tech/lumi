import CryptoKit
import DiskArbitration
import Foundation
import IOKit

/// Creates an opaque identity stored only in Lumi's local database. Physical
/// testing showed that even a tiny FAT root write can stall an otherwise
/// healthy USB source, so identity metadata never touches removable media.
enum USBLocalSourceIdentity {
    static func generated() -> String {
        "usb-local:\(UUID().uuidString.lowercased())"
    }
}

/// Builds an identity that remains stable across mounts while keeping
/// independent removable media separate. Some equal-model USB media expose an
/// identical or unreliable serial and cloned FAT media can share a filesystem
/// UUID. Version 2 therefore fingerprints all available evidence. A later
/// rename intentionally requires explicit re-authorization instead of silently
/// rebinding an existing trusted source by label.
public enum USBStableSourceIdentity {
    public static func sourceID(
        fileSystemUUID: String?,
        displayName: String,
        hardwareSerial: String? = nil
    ) -> String? {
        let normalizedSerial = hardwareSerial?
            .trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let normalizedUUID = fileSystemUUID?
            .trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let normalizedName = displayName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current)
            .lowercased()
        guard normalizedSerial?.isEmpty == false || normalizedUUID?.isEmpty == false else {
            return nil
        }
        let evidence = [normalizedSerial ?? "", normalizedUUID ?? "", normalizedName]
            .joined(separator: "|")
        return "usb-fs:v2-\(fingerprint(evidence))"
    }

    /// Resolves the serial published by the physical USB device that owns a
    /// mounted filesystem. Some media omit it, so callers must retain the
    /// filesystem/name fallback above.
    public static func hardwareSerial(for volumeURL: URL) -> String? {
        guard let bsdName = bsdName(for: volumeURL) else { return nil }
        let service = bsdName.withCString { name in
            guard let matching = IOBSDNameMatching(kIOMainPortDefault, 0, name) else {
                return io_service_t(IO_OBJECT_NULL)
            }
            return IOServiceGetMatchingService(kIOMainPortDefault, matching)
        }
        guard service != IO_OBJECT_NULL else { return nil }
        defer { IOObjectRelease(service) }

        let searchOptions = IOOptionBits(kIORegistryIterateRecursively | kIORegistryIterateParents)
        for key in ["USB Serial Number", "kUSBSerialNumberString"] {
            guard let value = IORegistryEntrySearchCFProperty(
                service,
                kIOServicePlane,
                key as CFString,
                kCFAllocatorDefault,
                searchOptions
            ) else { continue }
            if let serial = value as? String,
               !serial.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines).isEmpty {
                return serial
            }
        }
        return nil
    }

    private static func bsdName(for volumeURL: URL) -> String? {
        guard let session = DASessionCreate(kCFAllocatorDefault),
              let disk = DADiskCreateFromVolumePath(
                  kCFAllocatorDefault,
                  session,
                  volumeURL as CFURL
              ),
              let name = DADiskGetBSDName(disk) else { return nil }
        return String(cString: name)
    }

    private static func fingerprint(_ value: String) -> String {
        let digest = SHA256.hash(data: Data(value.utf8))
        return digest.prefix(8).map { String(format: "%02x", $0) }.joined()
    }
}

struct MountedUSBIdentity: Equatable, Sendable {
    let sourceID: String?
    let displayName: String
}

enum USBSourceIdentityResolver {
    static func selectedSourceID(
        for volume: MountedUSBIdentity,
        devices: [RekordboxDeviceState]
    ) -> String? {
        let exact = volume.sourceID.flatMap { sourceID in
            devices.first(where: { $0.sourceID == sourceID })
        }
        let migrationCandidates = devices.filter {
            isMigratableLegacy($0.sourceID)
                && namesMatch($0.displayName, volume.displayName)
        }
        if let exact {
            if namesMatch(exact.displayName, volume.displayName) {
                return exact.sourceID
            }
            // Some equal-model USB media publish the same unreliable hardware
            // serial. Prefer the unique legacy/name identity in that collision
            // case; otherwise preserve the stable identity across a real rename.
            if migrationCandidates.count == 1 {
                return migrationCandidates[0].sourceID
            }
            return exact.sourceID
        }
        if migrationCandidates.count == 1 {
            return migrationCandidates[0].sourceID
        }
        return volume.sourceID
    }

    static func volume(
        _ volume: MountedUSBIdentity,
        matches device: RekordboxDeviceState
    ) -> Bool {
        if volume.sourceID == device.sourceID { return true }
        if isMigratableLegacy(device.sourceID) {
            return namesMatch(volume.displayName, device.displayName)
        }
        return false
    }

    static func inspection(
        _ inspection: RekordboxDeviceInspectionState,
        matches device: RekordboxDeviceState
    ) -> Bool {
        if inspection.sourceID == device.sourceID { return true }
        if isMigratableLegacy(device.sourceID) {
            return namesMatch(inspection.displayName, device.displayName)
        }
        return false
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
        sourceID.hasPrefix("usb-fs:") || sourceID.hasPrefix("usb-local:")
    }

    static func isLegacy(_ sourceID: String) -> Bool {
        sourceID.hasPrefix("usb-volume:") || sourceID.hasPrefix("rekordbox-device:")
    }

    static func isMigratableLegacy(_ sourceID: String) -> Bool {
        isLegacy(sourceID)
            || isUUIDOnlyFilesystemIdentity(sourceID)
            || sourceID.hasPrefix("usb-fs:hardware-")
    }

    /// Dev builds before hardware-backed USB identity stored only
    /// `usb-fs:<filesystem UUID>`. Treat that exact one-component shape as a
    /// one-time migration candidate, never modern hardware/name identities.
    static func isUUIDOnlyFilesystemIdentity(_ sourceID: String) -> Bool {
        guard sourceID.hasPrefix("usb-fs:") else { return false }
        let identity = String(sourceID.dropFirst("usb-fs:".count))
        return !identity.isEmpty
            && !identity.contains(":")
            && !identity.hasPrefix("hardware-")
            && !identity.hasPrefix("v2-")
    }

    private static func namesMatch(_ lhs: String, _ rhs: String) -> Bool {
        lhs.caseInsensitiveCompare(rhs) == .orderedSame
    }
}
