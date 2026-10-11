# 0022 Stage reads, scopes and halos
Status: proposed
Date: 2026-10-11
Source: spec-guardian review of m3-stage-model (2026-10-11); constitution CON-08, CON-06, CON-23; ADR 0006, ADR 0021; background report §5 (global-coarse, regional with a halo, local)

## Context
GEN-007 (m3-stage-model) refuses a field graph in which a field reads a field of its own stage or a later one. That is the plainest reading of CON-08 ("a stage reads only outputs of earlier stages") and it makes cycles impossible by construction. Real pipelines will want to relax it, and the reviews named where: hydrology is a chain of regional steps (fill depressions, flow direction, accumulation; slope, then erosion), and local layers are composed at one point (detail noise times a mask, an erosion filter over detail). The review also found three gaps in what a key must say: the meaning and unit of the halo, the scope of an output, and the resolution and face mapping.

## Options (with trade-offs)
1. **Keep the strict rule.** Every regional step is one field that computes its whole chain internally; local layers are merged into one field. Simple and constitutional, but it loses per-layer versions (a change to one step re-keys the whole chain) and per-layer debug views.
2. **Allow same-stage reads under a rule that keeps tiles independent.** A field may read a field of its own stage if the graph stays acyclic (a real topological sort replaces the order by stage), the scope is the same (the same tile, never a neighbouring tile's regional output), and, for regional fields, the producer's computed extent is at least the consumer's halo plus its stencil (halos add up along the chain). Local fields compose at one point, so the scope rule is trivially met.
3. **Allow any read of a lower-or-equal stage without a rule.** Cycles and neighbour-tile reads become possible; tile independence (GEN-004..006) can no longer be argued from the graph.

## Decision
None yet. *Leaning:* option 1 until SPIKE-03 shows which regional chains it needs, then option 2. If the owner reads CON-08 literally (no same-stage reads at all), option 2 is a constitution amendment and a stop point; if the owner reads it as "no later-stage reads and no cross-tile reads", option 2 is an ADR.

## Points that are decided in code now (and can be revised here)
- **Scope** is typed: `Scope::Global` for the global bake, `Scope::Tile(tile)` for regional and local outputs; the kind is hashed into the key, a scope of the wrong kind for the stage panics, and tile `raw() == 0` never aliases the global scope.
- **Halo** is an *output* parameter: how far beyond the tile edge the field computes, as a fraction of the tile edge (ADR 0006), canonical at `-0.0 == 0.0`. A halo in integer samples would be more canonical for non-power-of-two resolutions; with power-of-two resolutions and fractions of 1/4 or 1/2 both are equivalent. *Open:* whether the unit should become integer samples when a field needs another fraction.
- **The key** covers the stage key version (`STAGE_KEY_VERSION`, separate from the height and mesh key format), field id, version, stage, implementation id, halo, scope kind and value, body id, reference-surface id, face-mapping id, resolution, the recorded read set and the keys of the inputs (looked up by name; their order does not matter). Known answers are pinned and compared on both CI operating systems.
- **One key per input field.** A local tile at a regional tile's border, beyond that tile's halo, would need several upstream keys for one input field; the model has one. Revisit with the first regional field (the halo is meant to make this impossible: a regional tile's output covers every local point of the tile).
- **Declared versus recorded reads** (ADR 0021 point 7): the graph checks declared reads only.

## Consequences
- The strict rule costs nothing until a chain needs relaxing, and relaxing it is a graph-construction change plus a topological sort, with no change to the key layout.
- The owner decides CON-08's reading before SPIKE-03 or the first regional chain.

## Licences of adopted code or techniques
None.
