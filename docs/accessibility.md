# Accessibility

Glyphspace maps spatial UI back into an accessibility tree with roles, labels, focus order, keyboard actions, spatial descriptions, and user preferences. The web SDK renders a DOM mirror alongside the canvas surface.

The app kernel treats accessibility as a second renderer. `accessibility_frame()` derives a verified frame from each visual `AppFrame`, including focus order, validation report, and spoken spatial descriptions such as position and size. Conformance tests assert that visible glyphs have accessibility nodes and that personalization cannot remove mandatory semantics.
