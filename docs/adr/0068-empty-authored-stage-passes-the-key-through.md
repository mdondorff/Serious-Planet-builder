# 0068 An authored stage with an empty footprint passes its source key through
Status: accepted (opus-planner under the owner's delegation of 2026-10-12, 2026-10-19; built by sonnet-m4 in stage-pass-through). Amends ADR 0066 point 4.
Date: 2026-10-19
Source: ADR 0066 point 4; window-13 smoke run (sonnet-m4, 2026-10-19); STRM-028; WDEF-007

## Context
- ADR 0066 point 4 keys every authored stage by its version, layer, index, source key and query entry, also on tiles no item touches.
- Window 13's smoke run: the first stamp of a world re-keyed all 3,039 resident tiles (110 touched), in both the 128-bit persistent keys and the 64-bit plan mesh keys; the edit never became exact.
- Stacking an empty edit layer from the start fixes the re-key but splits the keyspace: an editing world could not reuse the plain world's store or coarse pack.

## Decision
1. A stage may declare **pass-through**: on a tile whose footprint result (cap plus halo, WDEF-007) is empty, its output equals its source bit for bit, and its key (128-bit and 64-bit) is the source key.
2. Pass-through is a per-stage contract (`passes_through`, default false). The stamp stage claims it; a stage that can change output where no item is never claims it.
3. Wrappers forward the claim. A stage is stacked only for a layer that has items.
4. Each claiming stage carries tier-A tests: empty-footprint output equals the source (heights, climate, bounds), keys equal the source keys, and a first item re-keys only the tiles its footprint names.

## Consequences
- Untouched tiles keep the plain world's keys under any number of layers; the coarse pack and the store serve editing sessions.
- A version bump of a claiming stage re-keys only tiles with items; a change that alters output elsewhere must drop the claim.
- Easy to reverse: drop the claim, at the cost of world-wide re-keys.

## Licences of adopted code or techniques
None.
