# 0067 Stamp key hash and height bounds by delta range
Status: accepted (opus-planner under the owner's delegation of 2026-10-12, 2026-10-19; built by sonnet-m4 in stamp-stage). Amends ADR 0066 points 2 and 5, and point 6 in one respect: for stamp kinds the key uses the derived stamp hash (decision points 1 and 3), so the file hash and its recorded versions no longer enter tile keys; the file format and the file hash are unchanged (status line extended 2026-10-19 after the spec-guardian review of stamp-stage).
Date: 2026-10-19
Source: ADR 0066; stamp-stage (#365, #366, change/stamp-stage-cache); WDEF-010; GEN-026

## Context
- ADR 0066 point 2 puts an item's "content hash" in each footprint result row; point 6 defines the item hash over the whole parsed file form, generator and stage versions included.
- Stamps need a hash that changes exactly when their output can change; the recorded versions are already in the chained stage key (ADR 0066 point 4) and the base key.
- ADR 0066 point 5 widens a tile's bounds by the sum of `max_delta_m`; a 60 m hill with a 3 km reach then emptied the morph interval at level 19 (plan refused, InvalidLod).

## Decision
1. **A stamp's content hash is derived from the stamp**: BLAKE3-128 under `planet-builder stamp v1` over the kind and the parameter bits (WDEF-010). For stamp kinds it, not the item file hash, enters the footprint row.
2. The footprint row hash still covers the id, the priority, the content hash, the anchor bits, `reach_m` and `max_delta_m` (`footprint_item_hash128`), so a move, a resize or a parameter change re-keys every tile the item touches.
3. The item file hash of ADR 0066 point 6 stays the file's identity; name, notes and recorded versions re-key no tile.
4. **Bounds by delta range**: a tile's height bounds are widened by each stamp's delta range over the tile's cap (exact for monotone profiles at the near and far distance), not by the sum of `max_delta_m`. `max_delta_m` stays the declared bound and the contract test still checks it.

## Consequences
- A generator or stage version change re-keys stamped tiles through the stage key chain, not through the item hash; a tier-A test asserts it.
- Every new stamp kind needs a monotone profile, or a bounds rule of its own, before it ships.
- Easy to reverse: nothing keyed by these hashes is persisted beyond the tile cache.

## Licences of adopted code or techniques
None.
