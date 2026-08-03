import AppKit
import Foundation
import LumiDesignSystem
import LumiLibraryWorkspace
import SwiftUI

@main
struct LumiLibraryVisualEvidenceCommand {
    private static let width: CGFloat = 1_180
    private static let height: CGFloat = 820

    private struct Variant {
        let name: String
        let state: LibraryWorkspaceState
        let notation: KeyNotationPreference
        let colorScheme: ColorScheme
    }

    @MainActor
    static func main() throws {
        let outputDirectory = try outputDirectoryURL()
        try FileManager.default.createDirectory(
            at: outputDirectory,
            withIntermediateDirectories: true
        )
        let variants = [
            Variant(
                name: "library-ready-dark-camelot",
                state: LibraryWorkspaceFixtures.ready,
                notation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "library-ready-light-classic",
                state: LibraryWorkspaceFixtures.ready,
                notation: .classic,
                colorScheme: .light
            ),
            Variant(
                name: "library-empty-dark",
                state: LibraryWorkspaceFixtures.empty,
                notation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "library-importing-light",
                state: LibraryWorkspaceFixtures.importing,
                notation: .camelot,
                colorScheme: .light
            ),
            Variant(
                name: "library-stale-dark",
                state: LibraryWorkspaceFixtures.stale,
                notation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "library-degraded-light",
                state: LibraryWorkspaceFixtures.degraded,
                notation: .classic,
                colorScheme: .light
            ),
            Variant(
                name: "library-conflict-dark",
                state: LibraryWorkspaceFixtures.conflict,
                notation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "library-error-light",
                state: LibraryWorkspaceFixtures.error,
                notation: .camelot,
                colorScheme: .light
            )
        ]

        for variant in variants {
            let canvas = variant.colorScheme == .dark
                ? Color(red: 0.055, green: 0.063, blue: 0.078)
                : Color(red: 0.965, green: 0.97, blue: 0.98)
            let view = ZStack {
                canvas
                LibraryWorkspaceView(
                    state: variant.state,
                    keyNotation: .constant(variant.notation),
                    rendersInteractiveControls: false
                )
            }
            .environment(\.colorScheme, variant.colorScheme)
            .environment(\.locale, Locale(identifier: "en"))
            .frame(width: width, height: height)

            let renderer = ImageRenderer(content: view)
            renderer.proposedSize = ProposedViewSize(width: width, height: height)
            renderer.scale = 1
            guard let image = renderer.nsImage,
                  let tiffData = image.tiffRepresentation,
                  let representation = NSBitmapImageRep(data: tiffData),
                  representation.pixelsWide == Int(width),
                  representation.pixelsHigh == Int(height),
                  let pngData = representation.representation(using: .png, properties: [:]),
                  pngData.count > 10_000 else {
                throw VisualEvidenceError.renderFailed(variant.name)
            }
            let output = outputDirectory.appendingPathComponent("\(variant.name).png")
            try pngData.write(to: output, options: .atomic)
            print(output.path)
        }
    }

    private static func outputDirectoryURL() throws -> URL {
        let arguments = CommandLine.arguments
        guard let outputIndex = arguments.firstIndex(of: "--output"),
              arguments.indices.contains(outputIndex + 1) else {
            throw VisualEvidenceError.missingOutputDirectory
        }
        return URL(fileURLWithPath: arguments[outputIndex + 1], isDirectory: true)
    }
}

private enum VisualEvidenceError: LocalizedError {
    case missingOutputDirectory
    case renderFailed(String)

    var errorDescription: String? {
        switch self {
        case .missingOutputDirectory:
            "Usage: LumiLibraryVisualEvidence --output <directory>"
        case let .renderFailed(name):
            "Could not render Library evidence variant '\(name)'."
        }
    }
}
