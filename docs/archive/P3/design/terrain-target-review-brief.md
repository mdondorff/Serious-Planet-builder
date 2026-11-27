# Reviewer brief: terrain target architecture (A3)

Status: for one external review round before the prototype builds on it (SPIKE-105 §14).

**What it is.** A planet-scale terrain generator, editor and renderer (Rust, wgpu). The target document changes how erosion, hydrology and edits fit together so that edits can update rivers and erosion. Deterministic CPU generation is kept; the GPU is presentation only.

**Read (in order, about 45 min).**
1. `docs/design/terrain-target.md` (the target and data contracts).
2. `spikes/architecture-replan/results/results.md` §8 and §12 (the probes F1-F7, numbers and methods).

**Code excerpts to check the solver against** (the real kernels, in this repository, `crates/generators/src/bake/`):
- `carve.rs`: `carve_core` (one routing per re-route interval, breach, sweeps).
- `route.rs`: `route_weighted`, `mfd_area`.
- `flood.rs`: `priority_flood` (FIFO `flat` queue).
- `spill.rs`: the planet-wide spill level (basins).
- `crates/generators/src/global.rs`: `read_elevation_c1` (the base read).

**Questions.**
1. Is the outer iteration's contract sound? F7 shows a round is exact when the routing is shared; the contract then clips MFD to the catchment and fixes the flood's tie order to make a catchment's round independent. Does anything else couple catchments in a round? Is memoisation of catchment-rounds by the key of their inputs a correct basis for incremental equality with a fresh build?
2. Are the basin and segment identity rules (ids as pure functions of the surface, plus a non-authoritative continuity map) stable enough for gameplay references and for undo?
3. Are the gates in §12 sufficient to tell a sound solver from a reproducible but wrong one?
4. Is anything in the sampler's composition or the canonical stored forms inconsistent with deterministic re-solves, or with hydrological correctness across grids?
5. Any requirement (§1) the design does not actually meet?

**Limits of the evidence.** One seed, one window or face per probe; timings on a shared machine; the store estimate is not yet measured. The owner's firm requirements are in §1 and are not under review.

**Out of scope.** The rendering stack, streaming, and the M9 tuning work; these stay.
