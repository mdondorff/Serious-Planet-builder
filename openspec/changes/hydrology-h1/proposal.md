## Why
A3 (docs/design/terrain-target.md) section 5.2 to 5.5 and inbox #99: hydrology H1 on the prototype landmass: the channel graph (D8 receivers, segments, confluences) kept apart from the diffuse field, and the basins with floor, spill point, class and identity. Also the peak memory of the resident round (the unmeasured item of GEN-144, a G-SCALE input).

## What Changes
- `bake::hydro`: `hydrology` (the discharge `Q` accumulated along the D8 tree, channel cells by initiation `A_d >= T` and continuation downstream, segments between heads, confluences and mouths with their ids, basins from the cells under the filled surface with the 16-step area curve, the class and the water level of section 5.5, ids by the lowest cell) and `verify` (G-CON and the conservation of the area).
- GEN-145 added (generator-pipeline).
- `examples/hydro_landmass`: H1 on the prototype landmass with the runoff of the orographic climate (`h1`), and the peak working set of the resident route and the 4 rounds (`mem`).

## Impact
- Code: `generators::bake::hydro` (new), `examples/hydro_landmass`. No call site in any pass of the bake; nothing in today's output moves.
- Not in this change: the edit cases E1-E6; the continuity map; storing segments or basins; the basin spill graph of `bake/spill.rs` (the filled surface of the routing is the minimax level plus the epsilon ramp by its construction); the lake's contribution to the water field.
