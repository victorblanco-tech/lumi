// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiEngineClient",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .library(name: "LumiEngineClient", targets: ["LumiEngineClient"])
    ],
    dependencies: [
        .package(path: "../LumiProtocol")
    ],
    targets: [
        .target(
            name: "LumiEngineClient",
            dependencies: ["LumiProtocol"]
        ),
        .testTarget(
            name: "LumiEngineClientTests",
            dependencies: ["LumiEngineClient"]
        )
    ]
)
