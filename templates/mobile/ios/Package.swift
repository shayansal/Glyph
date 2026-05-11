// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "GlyphspaceMobile",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "GlyphspaceMobile", targets: ["GlyphspaceMobile"])
    ],
    targets: [
        .target(name: "GlyphspaceMobile")
    ]
)
