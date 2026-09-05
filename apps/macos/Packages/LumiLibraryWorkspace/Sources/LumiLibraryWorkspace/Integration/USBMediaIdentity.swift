import Foundation

/// Called only by background inventory work. An identity marker contains no
/// music, playlist data, credentials or user information.
public struct USBMediaIdentity: Decodable, Sendable {
    public let schemaVersion: Int
    public let mediaId: UUID
    public let sourceId: String

    public static func read(from root: URL) -> USBMediaIdentity? {
        let file = root.appendingPathComponent(".lumi-media.json")
        guard let values = try? file.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey]),
              values.isRegularFile == true, values.isSymbolicLink != true,
              let size = values.fileSize, size <= 4096,
              let handle = try? FileHandle(forReadingFrom: file) else { return nil }
        defer { try? handle.close() }
        guard let data = try? handle.read(upToCount: 4097), data.count <= 4096,
              let marker = try? JSONDecoder().decode(Self.self, from: data),
              marker.schemaVersion == 1,
              (8...200).contains(marker.sourceId.utf8.count),
              marker.sourceId.hasPrefix("usb-fs:") || marker.sourceId.hasPrefix("usb-local:"),
              marker.sourceId.utf8.allSatisfy({ byte in
                  (48...57).contains(byte) || (65...90).contains(byte) || (97...122).contains(byte)
                      || [58, 45, 95].contains(byte)
              }) else { return nil }
        return marker
    }
}
