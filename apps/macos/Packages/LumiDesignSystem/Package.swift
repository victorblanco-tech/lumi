// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiDesignSystem",
    platforms: [
        .macOS(.v15),
        .iOS(.v18)
    ],
    products: [
        .library(name: "LumiDesignSystem", targets: ["LumiDesignSystem"])
    ],
    targets: [
        .target(name: "LumiDesignSystem"),
        .testTarget(
            name: "LumiDesignSystemTests",
            dependencies: ["LumiDesignSystem"]
        )
    ]
)
