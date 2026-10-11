## Why

M3 queue item 3 (ADR 0006 item 4, ADR 0007): generators still take a bare seed and keys hash the seed, so nothing proves that a key covers what a generator read, and repro bundles record `definition_hash 0`. This change adds the minimal WorldDefinition and the read-recording context, folds the M2 `HeightField` into the generator design, and makes keys and bundles use them. TOML nodes, the cell index and authored layers follow in the next change.

## What Changes

- `world-def`: `BodyId`, `WorldDefinition` (a sorted set of named nodes: world seed, body radii), canonical node hashes (path and exact value bits, independent of insertion order), `base_hash`, and `GenContext`, the only way generators read the definition; it records every read with its node hash (including the derived body seed, so a body's id is part of the read set) and hashes the read set.
- `generators`: `HeightGenerator` (`field(ctx, body)` returns the pure `HeightField`), `ReferenceGenerator`, `SteepGenerator` (test-only), `Bound` (a field carrying its context's read-set hash). `HeightField::inputs_hash` replaces the seed in height and mesh keys; a field built directly from a seed keys exactly like one built through a context over a definition with that seed.
- Repro bundles record the real definition hash (single planet of the recorded seed and radius); replay refuses a hash this build cannot rebuild, and a zero hash (bundle from before the definition) with a message to record again.
- Generator contract tests (reads only through the context, key covers exactly what was read, agrees with the direct seed field, missing nodes are errors).
- Modified: WDEF-003, WDEF-004 (planned to active for the seed and body nodes); WDEF-001, 002, 005 stay planned.
