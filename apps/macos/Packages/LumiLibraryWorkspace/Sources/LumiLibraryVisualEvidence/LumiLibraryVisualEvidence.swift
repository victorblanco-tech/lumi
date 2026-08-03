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

    private struct EditorVariant {
        let name: String
        let notation: KeyNotationPreference
        let hostColorScheme: ColorScheme
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

            try render(view, named: variant.name, to: outputDirectory)
        }

        let editorVariants = [
            EditorVariant(
                name: "track-editor-dark-camelot",
                notation: .camelot,
                hostColorScheme: .dark
            ),
            EditorVariant(
                name: "track-editor-light-host-classic",
                notation: .classic,
                hostColorScheme: .light
            )
        ]
        for variant in editorVariants {
            let hostCanvas = variant.hostColorScheme == .dark
                ? Color(red: 0.055, green: 0.063, blue: 0.078)
                : Color(red: 0.965, green: 0.97, blue: 0.98)
            let view = ZStack {
                hostCanvas
                TrackLightingEditorView(
                    analysis: TrackEditorFixtures.ready,
                    keyNotation: variant.notation,
                    rendersInteractiveControls: false
                )
                .padding(18)
            }
            .environment(\.colorScheme, variant.hostColorScheme)
            .environment(\.locale, Locale(identifier: "en"))
            .frame(width: width, height: height)
            try render(view, named: variant.name, to: outputDirectory)
        }

        let settings = PhraseRoleSettingsFixtures.ready()
        let phraseRoleView = ZStack {
            Color(red: 0.055, green: 0.063, blue: 0.078)
            PhraseRoleSettingsView(
                settings: settings,
                appearance: .constant(.dark),
                keyNotation: .constant(.camelot),
                rendersInteractiveControls: false
            )
        }
        .environment(\.colorScheme, .dark)
        .environment(\.locale, Locale(identifier: "en"))
        .frame(width: width, height: height)
        try render(phraseRoleView, named: "phrase-role-settings-dark", to: outputDirectory)

        let mappingView = ZStack {
            Color(red: 0.965, green: 0.97, blue: 0.98)
            PhraseRoleSettingsView(
                settings: settings,
                appearance: .constant(.light),
                keyNotation: .constant(.classic),
                initialSection: .sourceMapping,
                rendersInteractiveControls: false
            )
        }
        .environment(\.colorScheme, .light)
        .environment(\.locale, Locale(identifier: "en"))
        .frame(width: width, height: height)
        try render(mappingView, named: "phrase-source-mapping-light", to: outputDirectory)
    }

    @MainActor
    private static func render<Content: View>(
        _ view: Content,
        named name: String,
        to outputDirectory: URL
    ) throws {
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
            throw VisualEvidenceError.renderFailed(name)
        }
        let output = outputDirectory.appendingPathComponent("\(name).png")
        try pngData.write(to: output, options: .atomic)
        print(output.path)
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
