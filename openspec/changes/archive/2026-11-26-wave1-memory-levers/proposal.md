## Why
- The river-graph prototype (RG) is parked (owner, 2026-11-26). Its park plan, step 3, moves wave 1's memory levers that fit B's own regional bake onto main (lead, 2026-11-26).
- On the spike, wave 1 passed G3-6 on the full count: 9.26 GB of peak working set at 1.489e8 land cells (tag `archive/rg-parked-2026-11-26`, commit d9c320dd7).
- B's bake on main runs K5 resident (`solve_rounds`) and K6 as a route, a second flood and hydrology on the MFD area.

## What changes
- GEN-192: the lean wave-1 paths, each giving the bits of the path it replaces:
  - K5's in-flight catchment cap (`RoundCache::with_inflight_cap`);
  - hydrology on the channel initiation as bits (`initiation_bits`, `hydrology_initiated`), with the MFD area freed;
  - hydrology's cascade order and component flags freed before the basins.
- B's region bake (`region_bake.rs`) uses them:
  - K5 as the partitioned rounds (GEN-143), capped, with a cache that keeps nothing;
  - the front takes the partitioned route's filled surface instead of a second flood, and frees the weights and the area.
- `landmass_scale bwave1`: B's wave 1 measured (a counting allocator, MARK lines with the peak working set and peak commit, a guard).
- TEST-157: tier-A tests of the bits, each able to fail (see its Detail).
- Bit-identical outputs (CON-14): ADR 0118's switch-off does not apply. The fixture commit's file hash is `ef7c8a8e482014d6` before and after, at 1 and 8 threads.
- Left on the tag:
  - base1's parallel decide: base1, the selective drain, is an RG stage, and the park puts no RG stage on main;
  - RG's K6 levers of `rg_h1`'s budget mode (every basin crossed, no evaporation).

Expected golden changes: none (generator memory paths, bit-identical).

## Impact
- `crates/generators/src/bake/round.rs`, `hydro.rs`; `crates/cache/src/region_bake.rs`; `crates/generators/examples/landmass_scale.rs`.
- Tests: `crates/generators/tests/bake_lean_memory.rs`, `crates/cache/tests/region_bake.rs`.
- Specs: GEN-192 (generator-pipeline), TEST-157 (testing).
