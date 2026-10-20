## Why
M6 plan row E5, third part: regions. Owner answers (2026-10-19): a region is a closed points list; its mask clips a chosen layer's edits, inside or outside, like a layer mask; surface types are a data table (rock, sand, grass, snow, forest, ice, desert, wetland, urban). The item model (points, `mask_layer`) is `line-items`; the line stage is `line-stage`. This change gives the region its table, its validation, its geometry and its mask in the stamp stage. The overlay that tints a region by its surface type is a later change (`region-stage`, VIEW-030).

## What Changes
- **Surface type table** (WDEF-012): nine entries (name, debug tint) in a fixed order; a region stores the index.
- **Kind `region`** (WDEF-012): a closed polygon of 3 to 64 points, parameters `surface_type` and `mask_mode` (0 none, 2 clip inside), `mask_layer` for mode 2; validation at the put (non-self-crossing, within one reach, indexes in range); a mode-2 region is filed in the layer it clips.
- **Mask stage** (GEN-029): the stamp stage of that layer leaves the base where a mode-2 region contains the point (all deltas of the layer cut, a hard edge).
- **Mode 1 (clip outside) is refused** with a message: it needs the region in the read set of every tile of the layer, which the footprint and the summaries do not do; `openspec/changes/region-mode1/design.md` lays out what that takes, for the owner's decision.
- **Not in this change**: mode 1, the overlay (VIEW-030, `region-stage`), a feathered edge, regions over 40 km, the editor's region tools (crusader).

## Impact
- Specs: WDEF-012 (added, world-definition), GEN-029 (added, generator-pipeline).
- Code: `world-def` (`region.rs` geometry and table, `stamp.rs` kind, `lib.rs` put refusals, `item.rs` spec/validation), `generators` (`stamp.rs`: the cut).
- Goldens: none moves. Stage version: bump to 3 (a region cuts heights, new output) — stamped tiles re-key.
- numerics-reviewer (generators, polygon tests in f64), spec-guardian before archive; owner confirms ADR 0069 and decides mode 1.
