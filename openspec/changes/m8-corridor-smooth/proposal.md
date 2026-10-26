## Why
Look review 2b, river-r1 at 300 m: the lit valley wall shows stepped strips at the corridor's bend. The solved raster there is a clean flat floor and planar walls; the strips are the facets of a corridor that kinks: the routed path (a least-cost path on a raster of 64 m, joined at nodes) has four vertices within 800 m with turns of 45 to 90 degrees and two short stubs, and each segment's wall meets the next in a fan on the outer bank and a crease on the inner one (diagnosis to the planner, 2026-10-26).

## What Changes
- `Router::polyline` returns the smoothed polyline (GEN-084): the routed path averaged with the paths of the segments it continues from and into over a tent of 256 m half width, by at most 64 m; nodes of sources, outlets and confluences stay. `Router::raw_polyline` is the routed path (memoised on its own); `smooth_chain` is the pure filter.
- `AnchorLaw::smooth_m` and `smooth_max_m` (production 256 m and 64 m, 0 in the default law: the routed path as before), part of the definition hash and of the routing memo's law key; `routing_reach_m` adds `smooth_max_m` (GEN-078's reach).
- `ANET_VERSION` 10: every chunk is re-keyed.

## Impact
- Spec: generator-pipeline GEN-084 (new), GEN-078 (the reach names the smoothing).
- Code: `generators/route.rs`, `generators/anet.rs`; tests in `anet_network.rs` (kink, joint and confluence, borders/halos/threads).
- Measured at the two sites (chunk solve, smoothing off against on): river-r1 cut min -46.7 to -46.9 m, p1 -41.8 to -41.7, median -0.73 to -0.72, corridor cells 633 to 728, largest difference of a cell's height 5.7 m (p99 3.2 m), corridor moved at most 4.2 cells (34 m); hills B cut min -79.6 to -79.5, p1 -31.6 to -31.5, median -10.80 both, corridor cells 640 to 770, largest difference of a cell's height 35 m (p99.9 15 m; the valley moves up to 6.4 cells = 51 m on steep ground), largest drop between linked cells 2.35 to 2.92 m. Chunk solve time is unchanged within noise (5.7 to 5.8 s river, 4.8 to 6.1 s hills B on a busy machine).
- GEN-081's read set (crusader) uses `routing_reach_m`: it grows by 64 m (0.01 % of a chunk's discs).
- Moved goldens: chunk scenes with a river in view (checked by `cargo xtask test gpu`; lit views escalate to the owner's /bless).
