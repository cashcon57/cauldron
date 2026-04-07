// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "CauldronApp",
    platforms: [
        .macOS(.v26)
    ],
    targets: [
        .executableTarget(
            name: "CauldronApp",
            path: "Sources/CauldronApp",
            linkerSettings: [
                .unsafeFlags(["-L", "../target/release"]),
                .linkedLibrary("cauldron_bridge")
            ]
        )
    ]
)
