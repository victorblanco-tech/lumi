// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiLiveWorkspace",
    defaultLocalization: "en",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .library(name: "LumiLiveWorkspace", targets: ["LumiLiveWorkspace"]),
        .executable(name: "LumiVisualEvidence", targets: ["LumiVisualEvidence"])
    ],
    dependencies: [
        .package(path: "../LumiDesignSystem"),
        .package(path: "../LumiProtocol")
    ],
    targets: [
        .target(
            name: "LumiLiveWorkspace",
            dependencies: ["LumiDesignSystem", "LumiProtocol"],
            resources: [.process("Resources")]
        ),
        .executableTarget(
            name: "LumiVisualEvidence",
            dependencies: ["LumiLiveWorkspace", "LumiDesignSystem"]
        ),
        .testTarget(
            name: "LumiLiveWorkspaceTests",
            dependencies: ["LumiLiveWorkspace", "LumiProtocol"]
        )
    ]
)
