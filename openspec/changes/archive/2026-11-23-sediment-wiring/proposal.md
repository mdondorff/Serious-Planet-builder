## Why
Briefing item 10 (inbox #104, A3 section 6.7, G-SED): GEN-150 made deposition, pit spreading and thermal run on a cube grid; nothing called them after the rounds, and G-SED (eroded = deposited + exported) and the breach-lowering departure of K7's deposition were not measured on the prototype region.

## What Changes
- `bake::sediment`: `sediment_pass` (thermal, deposition on the surface's own route, pit spreading, in K7's order) on any grid including a compact landmass; `pit_cells`, `volume_m3`, `breach_lowered_m3`.
- `examples/landmass_scale sed`: the rounds, then the sediment steps on the whole landmass, with pits at each stage, the deposition's volumes and the breach's share of the eroded column.
- GEN-151 added (generator-pipeline).

## Impact
- Code: `generators::bake::{sediment, mod}`, `examples/landmass_scale`, `tests/bake_compact`. No call site in a pass of the bake.
- Not in this change: the steps run on the whole landmass; the per-tile form with a halo (A3 section 6.7) is not built. The wiring into the bake's tile pass is not done.
