# AI Contract

AI adapters implement:

```rust
trait AiPatchGenerator {
    fn propose_patch(&self, world: &GlyphWorld, request: &UserEditRequest, context: &AiContext) -> PatchProposal;
}
```

Adapters return proposals, explanations, confidence, rejected operations, warnings, and before/after summaries. Policy validation decides what can be accepted.

