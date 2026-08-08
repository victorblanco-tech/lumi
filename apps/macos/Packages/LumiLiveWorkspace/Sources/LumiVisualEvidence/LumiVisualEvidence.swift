import AppKit
import Foundation
import LumiDesignSystem
import LumiLiveWorkspace
import SwiftUI

@main
struct LumiVisualEvidenceCommand {
    private static let width: CGFloat = 1_280
    private static let height: CGFloat = 1_200

    private struct Variant {
        let name: String
        let state: LiveWorkspaceState
        let appearance: AppearancePreference
        let keyNotation: KeyNotationPreference
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
                name: "local-playback-library-next-dark-camelot",
                state: LiveWorkspaceFixtures.libraryBacked,
                appearance: .dark,
                keyNotation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "ready-dark-camelot",
                state: LiveWorkspaceFixtures.ready,
                appearance: .dark,
                keyNotation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "ready-light-classic",
                state: LiveWorkspaceFixtures.live,
                appearance: .light,
                keyNotation: .classic,
                colorScheme: .light
            ),
            Variant(
                name: "fallback-dark-camelot",
                state: LiveWorkspaceFixtures.fallback,
                appearance: .dark,
                keyNotation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "stale-light-camelot",
                state: LiveWorkspaceFixtures.stale,
                appearance: .light,
                keyNotation: .camelot,
                colorScheme: .light
            ),
            Variant(
                name: "loading-dark-camelot",
                state: LiveWorkspaceFixtures.loading,
                appearance: .dark,
                keyNotation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "disconnected-light-camelot",
                state: LiveWorkspaceFixtures.disconnected,
                appearance: .light,
                keyNotation: .camelot,
                colorScheme: .light
            ),
            Variant(
                name: "edited-locked-dark-camelot",
                state: LiveWorkspaceFixtures.editedPaused,
                appearance: .dark,
                keyNotation: .camelot,
                colorScheme: .dark
            ),
            Variant(
                name: "revision-conflict-light-classic",
                state: LiveWorkspaceFixtures.revisionConflictOff,
                appearance: .light,
                keyNotation: .classic,
                colorScheme: .light
            )
        ]

        for variant in variants {
            let canvas = variant.colorScheme == .dark
                ? Color(red: 0.055, green: 0.063, blue: 0.078)
                : Color(red: 0.965, green: 0.97, blue: 0.98)
            let view = ZStack {
                canvas
                LiveWorkspaceView(
                    state: variant.state,
                    productVersion: "0.1.0-dev",
                    appearance: .constant(variant.appearance),
                    keyNotation: .constant(variant.keyNotation),
                    allowsScrolling: false
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
            "Usage: LumiVisualEvidence --output <directory>"
        case let .renderFailed(name):
            "Could not render visual evidence variant '\(name)'."
        }
    }
}
