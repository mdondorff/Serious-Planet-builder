# Change: lod-capping

## Why
- SPIKE-26: 45 of 231 stops of the owner's items refused a base-LOD plan; since VIEW-037 a refusal keeps the last plan, so one steep spike freezes the refinement of the whole view. The brush stacks are real 220 m spikes (bounds exact), so no bound can fix them. ADR 0074 (accepted, opus-planner 2026-10-21) decides that a tile too steep to morph is not refined and the plan degrades locally. This is the first of its three changes: the selection, the cover and the plan.

## What changes
- `morph_room`: the one predicate for `LodParams::validate`, the selection and the plan's backstop; a bound that is not a number leaves no room (the old code let `f64::max` drop a NaN).
- The selection finds the unmorphable tiles of the position-only selection (no frustum) and builds the ceiling (`Ceiling`): `min over u of (level(parent(u)) + edge distance)`; the walk keeps from splitting a tile whose children the ceiling forbids and the leaves are clamped to it. The residency set R and the plan of any view agree on it; the draw set stays inside R.
- The residency set's bounds carry the ceiling; the cover keeps a forbidden tile as a leaf and clamps as the plan does (with every tile of R resident the cover is the plan node for node).
- `SelectedNode::capped` and `TerrainDraw::capped` (a runtime flag; the plan's text is unchanged).
- `LodParams::drawable_half_range` (the equation of `drawable_slope` solved per level).
- LOD-008 (the rejection becomes capping; the backstop stays), STRM-015, STRM-016; ADR 0074 accepted.
- Not in this change: the stitch and fine masks of the vertex shader (the second change: until then a capped tile next to a finer one can show a T-junction that the skirts hide), the F3 colour and the log line (the third).

## Tests
- `lod_capping.rs` (tier A): a spike of the owner's kind is capped, not refused, at the ground and at 200 m, every leaf morphable, balanced, nothing capped on the reference field; the cover equals the plan node for node on eight view directions and the draw set stays inside R; bounds that are not numbers.
- `lod.rs` unit tests: the ceiling over random unmorphable sets (a partition, under the ceiling, balanced, the finest, order independent), `morph_room`.
- Two existing tests change with the behaviour: the steep field's plan is capped instead of rejected, and the steep field's level sweep spans 6 levels, not 8.

Expected golden changes: none (no capped tile where nothing is steep; the plan's text and hash are unchanged).
