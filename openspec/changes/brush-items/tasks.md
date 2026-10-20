## Tasks
1. [ ] world-def: `BrushKind`, `Stroke`, `Stamp::with_strokes`, `StampKind::Brush`; stamp hash with strokes; tests first (hash pinned, a stamp without strokes unchanged).
2. [ ] world-def: `Item.strokes`, schema 3 writer and loader, validation, the hash (AUTH-005), `spec()` mapping, `restaged`; tests first (canonical text, other spelling, refusals, hash, old files unchanged).
3. [ ] world-def: `stamp_refusal` for the brush (WDEF-013); the footprint returns the strokes; tests first.
4. [ ] generators, frame, editor: the new kind adds nothing and compiles (match arms), the preview skips brushes, the editor's constructors default `strokes`.
5. [ ] spec-guardian; archive.
