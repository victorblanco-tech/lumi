// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiRemoteFeature",
    platforms: [
        .macOS(.v15),
        .iOS(.v18)
    ],
    products: [
        .library(name: "LumiRemoteFeature", targets: ["LumiRemoteFeature"])
    ],
    dependencies: [
        .package(path: "../LumiRemoteClient"),
        .package(path: "../../../macos/Packages/LumiDesignSystem")
    ],
    targets: [
        .target(
            name: "LumiRemoteFeature",
            dependencies: ["LumiRemoteClient", "LumiDesignSystem"]
        ),
        .testTarget(
            name: "LumiRemoteFeatureTests",
            dependencies: ["LumiRemoteFeature"]
        )
    ]
)
