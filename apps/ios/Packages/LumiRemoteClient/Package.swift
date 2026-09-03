// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LumiRemoteClient",
    platforms: [
        .macOS(.v15),
        .iOS(.v18)
    ],
    products: [
        .library(name: "LumiRemoteClient", targets: ["LumiRemoteClient"])
    ],
    dependencies: [
        .package(path: "../../../macos/Packages/LumiProtocol")
    ],
    targets: [
        .target(
            name: "LumiRemoteClient",
            dependencies: ["LumiProtocol"]
        ),
        .testTarget(
            name: "LumiRemoteClientTests",
            dependencies: ["LumiRemoteClient", "LumiProtocol"]
        )
    ]
)
