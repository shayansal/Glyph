package glyphspace.host

class GlyphspaceRuntimeBridge {
    var acceptedPatchCount: Int = 0
        private set

    fun loadWorld(bytes: ByteArray): Boolean = bytes.isNotEmpty()

    fun applyPatch(bytes: ByteArray): Boolean {
        if (bytes.isEmpty()) return false
        acceptedPatchCount += 1
        return true
    }

    fun accessibilityLabel(glyphId: String, fallback: String): String =
        "$fallback, Glyphspace glyph $glyphId"
}
