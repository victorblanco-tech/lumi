import CoreTransferable
import UniformTypeIdentifiers

public struct LibraryTrackTransfer: Codable, Equatable, Hashable, Sendable, Transferable {
    public let trackID: UInt64
    public let timelineRevision: UInt64

    public init(trackID: UInt64, timelineRevision: UInt64) {
        self.trackID = trackID
        self.timelineRevision = timelineRevision
    }

    public static var transferRepresentation: some TransferRepresentation {
        CodableRepresentation(contentType: .lumiLibraryTrack)
    }
}

public extension UTType {
    static let lumiLibraryTrack = UTType(exportedAs: "nl.blancoservices.lumi.library-track")
}
