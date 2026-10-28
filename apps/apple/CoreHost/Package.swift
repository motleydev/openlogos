// swift-tools-version:5.9
// Minimal macOS test host for the UniFFI Swift bindings (B061). Not the app: it only proves
// that the generated bindings link against the Rust core and behave like the Rust tests.
// `scripts/apple-bindings.sh` builds the XCFramework and copies the generated Swift file in;
// both are gitignored.
import PackageDescription

let package = Package(
    name: "CoreHost",
    platforms: [.macOS(.v13)],
    targets: [
        // Static libraries for macOS (arm64), iOS and the iOS simulator plus the generated
        // C header and module map, assembled by scripts/apple-bindings.sh into artifacts/.
        .binaryTarget(
            name: "OpenLogosCoreFFI",
            path: "Frameworks/OpenLogosCoreFFI.xcframework"
        ),
        // The generated OpenLogosCore.swift: typed records, errors and functions over the FFI.
        .target(
            name: "OpenLogosCore",
            dependencies: ["OpenLogosCoreFFI"],
            path: "Sources/OpenLogosCore"
        ),
        .testTarget(
            name: "CoreHostTests",
            dependencies: ["OpenLogosCore"],
            path: "Tests/CoreHostTests"
        ),
    ]
)
