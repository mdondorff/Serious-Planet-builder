## Design

Source: ADR 0066 (points 1, 2, 4, 5, 6), ADR 0021 point 7, SPIKE-13 (the disc stamp `A (1 - (d/r)^2)^2`), docs/briefings/m6-editor-plan.md row E2. Builds on E1 steps 1, 1b and 2 (merged). Revised after the spec-guardian review of 2026-10-19.

### Stamp profiles
A stamp has an anchor (unit direction), `reach_m` = its radius R (> 0), `max_delta_m` = its bound M, a kind and parameters in metres. The arc distance is `d = radius * atan2(|a x p|, a . p)` (f64, `libm`) and `x = d / R`. The delta is zero for `x >= 1`. Every profile is C1 at `x = 1` and never exceeds `M` in absolute value; the plateau is only C0 where its clamp bites inside the reach.

| Kind | Parameters | Delta at a point with running height `h` |
|---|---|---|
| `hill` | `amount_m` (signed) | `amount * (1 - x^2)^2` |
| `crater` | `amount_m` (the depth, at least 0) | `amount * (-bowl(x) + 0.2 * rim(x))`, `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` on `0.7 <= x <= 1` and 0 elsewhere, `s(t) = t^2 (3 - 2t)` |
| `plateau` | `target_m` (an absolute height above the reference sphere) | `w(x) * clamp(target - h, -M, M)`, `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))` |

`|amount_m| <= M` for a hill and a crater, and a plateau changes the ground by at most `M` through its clamp. A plateau takes an absolute target instead of "the height at the anchor plus an offset": a plateau that read the stage below at its anchor would read ground outside the tile (an earlier stamp that reaches the anchor is up to `R_B + R_P` from the tile, outside the footprint test of WDEF-007), so a tile could go stale after an edit to that stamp, and a plateau in a layer above another layer would read a per-tile base at a point its key does not cover. With an absolute target a delta depends on the running height at the point itself and on the stamps whose reach contains it, all declared by the footprint (ADR 0066 point 4's precondition and ADR 0021 point 7 hold; no departure, no new ADR).

Stamps apply in application order (priority, then id), each to the running height.

### Reading items without a context
`height(dir)` has no `GenContext` to record against, and the footprint API takes a tile and a halo. The stage therefore gets its items once per batch, through a reader:
- `trait ItemReader { fn items(&self, body, layer, cap: Cap) -> Vec<FootprintItem> }`, implemented for `Arc<WorldDefinition>` through a new `WorldDefinition::footprint_items_cap(body, layer, cap)` (the candidate enumeration of `footprint_items` for a cap instead of a tile; `footprint_items` becomes the tile form of it) and by a recording reader in the contract test.
- `heights_until` / `heights_threaded_until` (the batch paths, GEN-020) take the bounding cap of the batch's points enlarged by the longest reach, fetch the items once, then evaluate every point against them in application order; `height(dir)` is a batch of one. Results do not depend on the batch.

### The stage
`StampStage { base: Arc<dyn HeightField>, items: Arc<dyn ItemReader>, body, layer, stack_index, stage_version }` implements `HeightField`:
- `height`, `heights_until`, `heights_threaded_until` as above.
- `max_abs_height_m` = the base's plus the sum of the layer's `max_delta_m`, computed once at construction.
- `height_bounds(centre, angular_radius)` = the base bounds widened on both sides by the sum of `max_delta_m` of the items whose reach touches the cap (`footprint_items_cap`, application order, f64, rounded up): ADR 0066 point 5. The widening is done here and only here; E1 step 3 adds the `SelectionCache` key and the check at levels 16 to 18 (its tasks 3.1 and 3.2 say so).
- Everything else delegates to the base: `body`, `seed`, `surface_id`, `climate`, `climate_version_word`, `heights_and_climate`, `water_cull_level` (GEN-021). `source_key()` is `None` (the base key already carries the bake).
- `reads_cells` is true and `read_records_for(tile)` is the base's records plus the layer's footprint (or, over the limit, summary) reads of the tile with the stage's halo `STAGE_HALO = 0.25` of the tile edge (the sample stencil and skirt need a small fraction; a larger declared halo only adds false positives; it is a key parameter and a change bumps `stage_version`).
- Chain: two new trait methods with defaults, `HeightField::upstream() -> Option<&dyn HeightField>` and `stage_params() -> Option<(&str, u32)>` (layer name and stack index). `cache::key::field_inputs` sets `source = Some(height_key(upstream, same tile, same resolution))` when `upstream` is some (preferred over `source_key`), puts the stage params into the new output parameter tags `PARAM_STAGE_LAYER = 8` and `PARAM_STAGE_INDEX = 9`, takes `generator_version` from the stage version and `implementation_id` `stamp-stage`. The mesh key chains the stage's height key as it chains any height key. Wrappers that forward `source_key` by hand (`Bound` in generators, `app/skeleton.rs`, `app/examples/plan_ablation.rs`, `frame/tests/support/origin_batch.rs`, `generators/water.rs`, the streamer's cancellable wrapper) forward `upstream` and `stage_params` as well; a tier-A test builds each and checks the forwarding.
- Why not `stage_key` (STRM-011): that function keys `FieldGraph` outputs of the global bake in its own `planet-builder stage key v1` context and scope; a stamped tile is a `Kind::Height` record that the mesh key, the tile store and the packs already chain on. ADR 0066's "128-bit stage key" is this chained height key: the same fields (stage version, layer, index, previous stage's key, the layer's query entries).
- Precondition (ADR 0066 point 4): the stage reads the base only as a height field at points, and a stage over a per-tile stage is allowed because each point read is declared by that tile's own footprint; a base that needs neighbour keys needs a new ADR.

### Items
`ItemSpec` gains `kind: StampKind` (none, hill, crater, plateau) and `params: [f64; 3]` (`amount_m` or `target_m`, two reserved at 0). `content_hash` for a stamp is derived from the kind and parameters by `ItemSpec::stamp(...)` (one source of truth; `put_item_spec` refuses a stamp whose `content_hash` disagrees). `FootprintItem` and `ItemRec` carry the kind and the parameter bits; `item_hash128` appends them only when the kind is not none, so every E1 known answer for items of kind none is unchanged (the fields are fixed width, so the suffix is unambiguous), and the summary leaves follow. `put_item_spec` refuses a stamp with `reach_m <= 0`, a non-finite parameter, `|amount| > max_delta_m`, a negative crater depth. `ItemSpec` literals in the E1 tests gain `kind: StampKind::None` and `params: [0.0; 3]` (listed in tasks). The id type is crusader's (E4's first task); item files (TOML) are E4: E2 owns `StampKind` and `params` in `ItemSpec`, E4 maps them to the file. The ADR 0066 consequence "item schema with the first change that reads item files" is met by E4, which reads them.

### Tests
Tier A unless noted.
- Profiles: known answers at fixed `x` per kind (crater -1 at 0, -0.0430 at 0.7, 0.2 at 0.85, 0 at 1), C1 at `x = 1`, zero beyond, `|delta| <= M` on a dense sweep, plateau flattens a ramp inside `x <= 0.7` and is continuous at 0.7, application order matters.
- Determinism: a tile's heights at 1 and 3 threads, batch order reversed, and one point at a time are bit-identical (GEN-020).
- Keys (STRM-027): the stamped tile key chains the base tile key, the stage version, the layer, the index each change it; a stamp far away does not; an edit changes the keys of exactly the tiles the footprint finds (at least those the reach touches, none beyond the margin); a two-layer stack: a base change re-keys both layers, an edit in layer 1 re-keys layer 2's tiles of that place and no other; every wrapper forwards the chain; the unstaged key of the base field equals the pinned `KNOWN_HEIGHT` of `keys.rs`.
- Contract (ADR 0021 point 7): the recording reader observes the stage over the tile and its halo; every item it returned is in the footprint result the tile declared, and the output is unchanged beyond `reach_m` of every stamp and within `max_delta_m`.
- Tile alone (E1's gate, GEN-017): a stamped tile over the eroded field over an in-memory bake and over a stored bake equals the same samples cut from its parent's grid, whatever the generation order.
- Bounds: planning a steep base field with stamps at levels 16 to 18 gives no empty morph interval, and every sampled height lies within `height_bounds`.
- Tier C (CON-18, adapter by adapter): a stamped coast scene drawn in the debug relief view; the frame hash is recorded per adapter family like `gpu_water`'s `KNOWN` (the assertion), and the images are written to `target/review/stamps/` for inspection (not an assertion). No PNG golden moves or is added.

### Not in this change
Item files and ULIDs and the commands (E4), the vertex-stage preview (E3), splines and regions (E5), brushes (E6), the `SelectionCache` key (E1 step 3), a stamp UI (U3).

### Open for the owner
None that is hard to reverse: the profiles, the halo and the parameter layout are version 1 of the stage; a change bumps `stage_version` and re-keys the layer.
