import Foundation
import UIKit

public final class GlyphspaceRuntimeBridge {
    public private(set) var acceptedPatchCount: Int = 0

    public init() {}

    public func loadWorld(_ bytes: Data) -> Bool {
        !bytes.isEmpty
    }

    public func applyPatch(_ bytes: Data) -> Bool {
        guard !bytes.isEmpty else { return false }
        acceptedPatchCount += 1
        return true
    }

    public func accessibilityLabel(for glyphId: String, fallback: String) -> String {
        "\(fallback), Glyphspace glyph \(glyphId)"
    }
}
