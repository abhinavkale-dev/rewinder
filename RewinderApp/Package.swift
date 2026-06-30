// swift-tools-version: 6.2
import PackageDescription
import Foundation

let rustProfile = ProcessInfo.processInfo.environment["REWINDER_RUST_PROFILE"] ?? "debug"
let rustLibSearchPath = "../src-tauri/target/\(rustProfile)"

let frameworks = [
    "Carbon", "CoreGraphics", "CoreFoundation", "WebKit", "ApplicationServices",
    "CoreVideo", "JavaScriptCore", "Security", "AppKit", "CoreData", "CoreText",
    "CoreImage", "CloudKit", "QuartzCore", "Foundation",
]

let frameworkFlags = frameworks.flatMap { ["-framework", $0] }

let package = Package(
    name: "RewinderApp",
    platforms: [.macOS(.v26)],
    targets: [
        .target(name: "CRewinderFFI"),
        .executableTarget(
            name: "RewinderApp",
            dependencies: ["CRewinderFFI"],
            linkerSettings: [
                .unsafeFlags(
                    ["-L\(rustLibSearchPath)", "-lrewinder_lib"]
                        + frameworkFlags
                        + ["-lobjc", "-liconv", "-lc", "-lm"]
                )
            ]
        ),
    ]
)
