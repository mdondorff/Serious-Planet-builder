## Why
M6 plan row E2: the first authored stage. E1 made edits cheap to find (footprint query, region summaries) and cheap to show (refresh with stale-while-revalidate); nothing yet changes a height. A stamp (crater, hill, plateau) is the smallest authored item that does: a relative delta with a footprint. It exercises everything ADR 0066 decided: per-item `reach_m` and `max_delta_m`, the chained stage key, bounds by reach, and the declared-versus-recorded reads contract with a generator that really reads items.

## What Changes
- **Stamp kinds and profiles** (GEN-026): `hill`, `crater`, `plateau`, each a C1 profile of the arc distance to the anchor over `reach_m`, deterministic (libm, fixed evaluation order), bounded by the item's `max_delta_m`, applied in application order (priority, id).
- **The stage** (GEN-026, STRM-027): `StampStage` wraps a base `HeightField` and one layer of a world definition. Its height at a point is the base height plus the stamps whose reach contains the point. Its tile keys chain: source = the key of the same tile of the stage below (the base first), plus the stage version, the layer, its index in the stack and the layer's footprint or summary entries (ADR 0066 point 4). A stamp edit re-keys the tiles the footprint finds and nothing else; a base change re-keys all.
- **Bounds** (GEN-026): the stage's `height_bounds` are the base bounds widened by the sum of `max_delta_m` of the items the tile's cap touches (ADR 0066 point 5), so planning never sees an empty morph interval.
- **Stamp items** (WDEF-009): `ItemSpec` carries a `kind` and its parameters; both are in the item hash. Item files (TOML) are not read or written here.
- The contract tests of ADR 0021 point 7 and the tile-alone test over the eroded base (E1 left it for the first real stage) run against real stamps.

## Impact
- Specs: GEN-026 (added), STRM-027 (added), WDEF-009 (added).
- Code: `generators` (stamp profiles, `StampStage`, `HeightField::upstream`), `cache` (the chain in `field_inputs`, two output parameter tags, `Kind` unchanged), `world-def` (`ItemSpec` kind and parameters, hash). `render`, `frame` and shaders are untouched.
- Goldens: none moves (no world has stamps). The stamped scene is checked by determinism and recorded-hash tests and by relief images written to `target/review` for inspection; a PNG golden would need the owner's `/bless` and is not part of this change.
- Dependencies: the 128-bit item id switch (sonnet-crusader, E4's first task) lands first; `ItemSpec` is shared. Item files, commands and undo are E4; the vertex-stage preview is E3.
- numerics-reviewer: the profiles and the stage (generators); spec-guardian before archive.
