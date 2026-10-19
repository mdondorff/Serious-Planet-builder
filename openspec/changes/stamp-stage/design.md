## Design

Source: ADR 0066 (points 1, 2, 4, 5), ADR 0021 point 7, SPIKE-13 (the disc stamp `A (1 - (d/r)^2)^2`), docs/briefings/m6-editor-plan.md row E2. Builds on E1 steps 1, 1b and 2 (merged).

### Stamp profiles
A stamp has an anchor (unit direction), `reach_m` = its radius R, `max_delta_m` = its bound M, and parameters in metres: `amount_m` (signed), and per kind below. The arc distance `d = radius * atan2(|a x p|, a . p)` (f64, `libm`) and `x = d / R`. Zero outside `x >= 1`. Every profile is C1 at `x = 1` and its absolute value never exceeds `M` (the amount is clamped to `M` at load, refused above it).

| Kind | Delta at a point with running height `h` |
|---|---|
| `hill` | `amount * (1 - x^2)^2` |
| `crater` | `amount * (-bowl(x) + 0.2 * rim(x))` with `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` on `0.7 <= x <= 1` and 0 elsewhere, `s(t) = t^2 (3 - 2t)`; `amount` is the depth (positive) |
| `plateau` | `w(x) * clamp(target - h, -M, M)` with `target = h_anchor + amount`, `h_anchor` the running height at the anchor, `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))` |

Stamps apply in application order (priority, then id); a delta is added to the running height. The running height at the anchor of a plateau is the height after the stamps before it, evaluated by the same rule at the anchor (a plateau reads the stage below and the earlier stamps at one point more, which the footprint covers because the anchor is inside its reach).

### The stage
`StampStage { base: Arc<dyn HeightField>, def: Arc<WorldDefinition>, body, layer, stack_index, stage_version }` implements `HeightField`:
- `height(dir)` = base height plus the running stamps of the layer whose reach contains `dir` (items found by the footprint query of the cell of `dir`: a point query is a tile query at the finest level; the cell index finds the candidates).
- `max_abs_height_m` = base plus the sum of the layer's `max_delta_m` over the body (a safe over-estimate for field-wide uses).
- `height_bounds(centre, angular_radius)` = the base bounds widened on both sides by the sum of `max_delta_m` of the items whose reach touches the cap (f64, application order, rounded up): the bounds of ADR 0066 point 5.
- `reads_cells` is true and `read_records_for(tile)` is the base's records plus the layer's footprint or summary reads of the tile (E1), so both key paths follow one source.
- `HeightField::upstream()` (new, default `None`) returns the base. `field_inputs` in `cache::key` sets the key's `source` to `height_key(upstream, same tile, same resolution)`; the mesh key chains the stage's height key as it chains any height key. Output parameters gain `PARAM_STAGE_LAYER` (the layer name) and `PARAM_STAGE_INDEX` (its index in ADR 0007's stack, not its priority); `generator_version` is the stage version and `implementation_id` is `stamp-stage`.
- Precondition (ADR 0066 point 4): the stage reads the base only as a height field at points and the base key is field-wide; true for the global bake and the eroded field. A per-tile base field needs neighbour keys and a new ADR.

### Items
`ItemSpec` gains `kind: StampKind` and `params: [f64; 3]` (`amount_m` and two kind parameters reserved, 0 today); both are in the item hash (`planet-builder footprint item v1` keeps its layout for the old fields and appends the kind byte and the parameter bits only when the kind is not `none`, so the pinned known answers of E1 stay). The ULID switch is crusader's and lands before; this change adds fields, not types.

### Tests (tier A unless noted)
- Profile known answers at fixed `x` per kind; C1 at `x = 1` (value and slope within 1e-12 of 0); zero beyond the reach; `|delta| <= M` on a dense sweep; plateau flattens a ramp inside `x <= 0.7` and does nothing beyond 1; application order matters (hill then plateau against plateau then hill).
- Determinism: the heights of a tile at 1 and 3 threads and in reversed point order are bit-identical (GEN-020).
- Keys: the tile key of a stamped tile chains the base tile key (changing the base seed changes it, changing a stamp far away does not), the stage version, the layer and the index each change it; an edit changes the keys of exactly the tiles the footprint finds (the E1 test with a real stamp).
- Contract (ADR 0021 point 7): a recording context observes `height` over the tile and its halo; every item it touches is in the footprint result the tile declared, and the output is unchanged beyond `reach_m` and within `max_delta_m` (the E1 contract test with `StampStage` in place of the test field).
- Tile alone (E1 gate, GEN-017): a stamped tile over the eroded field over an in-memory bake equals the same samples cut from its parent's grid, whatever the generation order.
- Bounds: planning a stamped steep field at levels 16 to 18 produces no empty morph interval (the E1 step 3 check starts here; the cache key part is step 3).
- Tier B/C: a stamped coast scene rendered in the debug relief view and written to `target/review/stamps/`; a recorded-hash test per adapter family (like `gpu_water`'s `KNOWN`); no PNG golden moves.

### Not in this change
Item files and their ULIDs (E4), commands and undo (E4), the preview (E3), splines and regions (E5), brushes (E6), the `SelectionCache` key (E1 step 3), a stamp UI (U3).

### Open for the owner
None that is hard to reverse: the profiles are parameters of version 1 of the stage; a change bumps `stage_version` and re-keys the layer.
