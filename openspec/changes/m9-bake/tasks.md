## 1. PR order (each green; the switch stays off until the owner's look)
| PR | Scope | Requirements | Checks (tier) |
|---|---|---|---|
| 1 | Channels `k` and `m`: computed from the bake, stored beside it with block ranges, versioned; the `k` curve and the coast taper | TBD (generator-pipeline) | A: synthetic bake, 1 and 8 threads byte-identical; the channels' block ranges contain every value; the taper is 1 at the coast band |
| 2 | Runtime base `m + k (e - m) + k creases` behind the switch; bounds from the channels | TBD (generator-pipeline) | A: switch off bit for bit today's field; bounds nest (LOD-012) and contain sampled heights; B: plan-check on `global16384` with the channels passes; the level-step probe per level at the four sites |
| 3 | Bake kernels: flood, receivers, order, carve with uplift, breach, active-cell thermal, deposition sweep | TBD (generator-pipeline) | A: synthetic surfaces (a cone, a pit, a ramp) against expected results; thread-count independence; mass kept by thermal within f32 rounding; `numerics-reviewer` |
| 4 | Tile driver and the erosion layer: window sampling, inflow, checkpoints, `--resume`, the layer's writer and reader; `generate --erosion-layer DIR`; `cargo xtask` command under the heavy lock | TBD (generator-pipeline, streaming-cache) | A: a 256-cell bake end to end; resume after a killed run gives identical bytes; B: one tile per landform timed; the seam step across core edges |
| 5 | Runtime adds the layer behind the switch; the layer's block ranges in the bounds | TBD (generator-pipeline, streaming-cache) | A: switch off unchanged; bounds contain the layered heights; C: goldens unchanged with the switch off |
| 6 | The overnight bake and the scorecard report on the baked field (four hill sites plus the other landforms) | TBD (testing) | the bake at most 10 h (criterion 7, the run itself); the scorecard report; stills at 300 m, 30 m, 1.8 m for the owner |

## 2. Steps
- [ ] 2.1 Ask the planner for requirement IDs; write the spec deltas (generator-pipeline, streaming-cache, testing) before PR 1
- [ ] 2.2 PR 1: channels; spec-guardian on the deltas
- [ ] 2.3 PR 2: runtime base behind the switch; plan-check and the level-step probe with the switch on (report, not a gate yet)
- [ ] 2.4 PR 3: kernels; `numerics-reviewer`
- [ ] 2.5 PR 4: tile driver and layer; one tile per landform timed (a correctness run, ADR 0087); seam step measured; if over 1 m p99, the halo blend (design.md 4)
- [ ] 2.6 PR 5: runtime layer
- [ ] 2.7 PR 6: overnight bake (machine reserved, BelowNormal, CPU state logged); scorecard; stills; the owner's look (M9 draft, R4 row)
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
| Thermal | 33 degrees, +-25 %, 1/16 per pass | SPIKE-64 |
| Deposition | kc 1e5, zeta 0.5 | SPIKE-64 |
| Layer | 7-10 GB | SPIKE-61 |
| Bake estimate | 2.5-4 h for 2,140 land tiles at 4 at a time | SPIKE-61, SPIKE-62, SPIKE-65 |
