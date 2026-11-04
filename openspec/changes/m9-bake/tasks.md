Revised 2026-10-31 (SPIKE-70): the carve moves to a face pass before the tile driver (PR 4), the tile driver keeps only local work (PR 5), and thermal gets a timing gate (PR 3); 7 PRs instead of 6.

## 1. PR order (each green; the switch stays off until the owner's look)
| PR | Scope | Requirements | Checks (tier) |
|---|---|---|---|
| 1 | Channels `k` and `m`: computed from the bake, stored beside it with block ranges, versioned; the `k` curve and the coast taper | GEN-093 to GEN-099, STRM-067, STRM-068 | A: synthetic bake, 1 and 8 threads byte-identical; the channels' block ranges contain every value; the taper is 1 at the coast band |
| 2 | Runtime base `m + k (e - m) + k creases` behind the switch; bounds from the channels | GEN-099, GEN-102 | A: switch off bit for bit today's field; bounds nest (LOD-012) and contain sampled heights; B: plan-check on `global16384` with the channels passes; the level-step probe per level at the four sites |
| 3 (3a: flood, receivers, order, carve, breach; 3b: thermal, deposition and the timing gate, after PR 4) | Bake kernels: flood, receivers, order, carve with uplift and an area cap, breach, active-cell thermal, deposition sweep | GEN-100, GEN-101 | A: synthetic surfaces (a cone, a pit, a ramp) against expected results; thread-count independence; mass kept by thermal within f64 rounding; B: active-cell thermal on one tile under 5 s (timing gate); `numerics-reviewer` |
| 4 | Face carve (pass 1): the base sampled once per 814 m cell per face, the carve at about 800 m, its change stored per face, checkpoint and `--resume` | GEN-100, STRM-065, TEST-084 | A: a small face end to end, resume gives identical bytes; B: one face timed and its memory; the cube-edge step |
| 5 | Tile driver and the erosion layer (pass 2): the face change upsampled, local carve, active-cell thermal, deposition; checkpoints, `--resume`, the layer's writer and reader; `generate --erosion-layer DIR`; `cargo xtask` command under the heavy lock | GEN-101, STRM-065, STRM-066 | A: a 256-cell bake end to end; resume after a killed run gives identical bytes; B: one tile per landform timed; the core-edge seam against off-edge (SPIKE-70's measure) |
| 6 | Runtime adds the layer behind the switch and calls `with_crease_band(CreaseBand::of_the_bake(sea_level))` (GEN-109); the layer's block ranges in the bounds | GEN-102, STRM-066 | A: switch off unchanged; bounds contain the layered heights; C: goldens unchanged with the switch off |
| 7 | The overnight bake and the scorecard report on the baked field (four hill sites plus the other landforms) | TEST-081 to TEST-084 | the bake at most 10 h (criterion 7, the run itself); the scorecard report; stills at 300 m, 30 m, 1.8 m for the owner; TEST-081 criterion 1, TEST-082 class shares and hypsometry (seed 1), TEST-083 plans with no capped tile, TEST-084 the core-edge seam (B) |

## 2. Steps
- [x] 2.1 Ask the planner for requirement IDs; write the spec deltas (generator-pipeline, streaming-cache) before PR 1; testing (PR 7): TEST-081 to TEST-084
- [x] 2.2 PR 1: channels (1a: `relief_channels.rs`, the compute side with `max_step` a parameter; 1b: the stored form, `relief_store.rs`, kinds 10 and 11, STRM-067 and STRM-068); spec-guardian on the deltas pending
- [ ] 2.3 PR 2: runtime base behind the switch (`ErosionField::with_relief`, `relief_lattice.rs`, tier A on synthetic channels: done locally); plan-check and the level-step probe with the switch on (report, not a gate yet): not done, they need the baked channels of `global16384`
- [ ] 2.4 PR 3: kernels (3a: `bake/{flood,route,carve}.rs`, tier A on a pit, a ramp, a cone, a dam and a coast, window-independent routing, refusals, a pinned fingerprint; 3b: `bake/{thermal,deposit}.rs`, tier A on towers, noise, a valley with a pit and a carved ramp, tier B cost gate on one synthetic 3,584^2 tile: upland 0.5 s and 1 % of the full run's cell evaluations, a steeper one 11 s and 7.8 %, with the steep-ground cost in the design; the real-tile gate is PR 5's); `numerics-reviewer`: 3a and 3b done
- [ ] 2.5 PR 4: face carve (`bake/face.rs`), the maps of the carved base (`bake/map.rs`, `preview::{elevation_map, change_map}`), the layer store with a per-face checkpoint (`cache/layer_store.rs`, kind 12), `cargo xtask bake pass1` (one face at a time, `--planet-res` coarse only); tier A done locally; one face timed at 1024, 2048 and 4096 cells (a correctness run, ADR 0087: see the design, point 8); cube-edge step measured (see the design, point 8)
- [ ] 2.5b PR 5: tile driver (`bake/tile.rs`: window, incise-only local carve with the area cap, thermal, deposition, the core's change), tile layer store kind 13 (PROPOSED: the owner's approval of the persisted format is pending) with resume and the drop of pass 1 (`cache/layer_tiles.rs`), `cargo xtask bake layer` (planet pass or `--region`, `--keep-pass1`, maps and `tiles.txt`), tier A on a synthetic planet incl. the shared core edge (TEST-084 judged half); not done: one tile per landform timed on real ground and TEST-084 at SPIKE-70's four sites (need the store), the pyramid (STRM-066, PR 6)
- [ ] 2.6 PR 6: runtime layer
- [ ] 2.7 PR 7: overnight bake (machine reserved, BelowNormal, CPU state logged); scorecard; stills; the owner's look (M9 draft, R4 row)
- [ ] 2.8 With the switch on: gate and envelope (criterion 6) in a perf window (ADR 0087 point 3)
- [ ] 2.9 `verifier` before each PR is declared done; `spec-guardian` before archiving

## 3. Inputs from the spikes (start values for the spec)
| Item | Value | Source |
|---|---|---|
| Hill factor | about 4-6 | SPIKE-65, SPIKE-66 |
| Mountain factor for 0.8 of real | about 7.5 | SPIKE-66 |
| Ring | 10 km | SPIKE-66 |
| Carve | U/K 20, K dt 0.05, 20 steps, re-route every 5th | SPIKE-62, SPIKE-65 |
| Breach | 0.1 %, at most 50 m | SPIKE-66 |
| Thermal | 33 degrees, +-25 %, 1/8 per pass, at most 300 passes (1/16 and 100 until SPIKE-98) | SPIKE-64, SPIKE-98 |
| Deposition | kc 1e5, zeta 0.5 | SPIKE-64 |
| Layer | 7-10 GB | SPIKE-61 |
| Face carve | about 800 m (one point sample per cell), about 17-23 min a face at 10 threads with the area by multiple flow direction (SPIKE-71's 3 min and 5.5 GB are superseded) | SPIKE-70, SPIKE-71, SPIKE-98 |
| Local carve cap | about 1 km^2 | SPIKE-70 |
| Bake estimate | about 6-7.5 h (central 6.9 h) for 2,140 land tiles at 4 at a time (superseded: SPIKE-98 projects about 42-63 h as built on today's machine, a cheaper pass 2 is its subject) | SPIKE-61, SPIKE-70, SPIKE-71, SPIKE-98 |
- [ ] 2.x PR `change/pass1-mfd`: the drainage area by multiple flow direction (Freeman 1.1, receivers D8), the C1 read of the pass-1 change, kernel versions 2, pass 1 at 12,288 by default (GEN-100, GEN-101); tier A: area shared and jitter-independent, a smooth read, thread-count equality; `numerics-reviewer`, `spec-guardian`, `verifier`
- [ ] 2.y PR `change/pass2-cost`: SPIKE-98 candidate B: the base sampled every 4 cells and interpolated, halo 128, thermal 300 passes of 1/8, pass-2 kernel version 3 (GEN-101).
