// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiProtocol",
    platforms: [
        .macOS(.v15),
        .iOS(.v18)
    ],
    products: [
        .library(name: "LumiProtocol", targets: ["LumiProtocol"])
    ],
    targets: [
        .target(name: "LumiProtocol"),
        .testTarget(
            name: "LumiProtocolTests",
            dependencies: ["LumiProtocol"]
        )
    ]
)
