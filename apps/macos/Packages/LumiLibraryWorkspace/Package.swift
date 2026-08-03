// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiLibraryWorkspace",
    defaultLocalization: "en",
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "LumiLibraryWorkspace", targets: ["LumiLibraryWorkspace"]),
        .executable(name: "LumiLibraryVisualEvidence", targets: ["LumiLibraryVisualEvidence"])
    ],
    dependencies: [
        .package(path: "../LumiDesignSystem"),
        .package(path: "../LumiProtocol")
    ],
    targets: [
        .target(
            name: "LumiLibraryWorkspace",
            dependencies: ["LumiDesignSystem", "LumiProtocol"],
            resources: [.process("Resources")]
        ),
        .executableTarget(
            name: "LumiLibraryVisualEvidence",
            dependencies: ["LumiLibraryWorkspace", "LumiDesignSystem"]
        ),
        .testTarget(
            name: "LumiLibraryWorkspaceTests",
            dependencies: ["LumiLibraryWorkspace", "LumiProtocol"]
        )
    ]
)
