# Thermal-dam options brief (P3 E4)

Status: options for the lead's pick (spikes, 2026-11-14). Closes the brief of the briefing's CP2 exit; A3 §12.4 "Thermal" (P1); ADR 0112 condition 2.

## Question
Does thermal erosion make the pits and dams that remain after the rounds, and which change removes them at the least cost and risk?

## The kernel today
| Item | Value | Source |
|---|---|---|
| Kernel | Jacobi gather, mass kept; each cell gives each lower neighbour `k` of the height difference above `tan(theta) (1 + perturb u) d` | GEN-101, `bake/thermal.rs` |
| Parameters (pass 2 and K7) | theta 33 deg, perturb 0.25, k 1/8, at most 300 passes, min excess 1 mm | `Pass2Params::standard` (`bake/tile.rs`) |
| Sinks | sea, grid border; in K7 also the burned channel cells (`thermal_fixed`): what they would receive leaves as exported sediment | GEN-176 |
| Order | after the incising carve; before the deposition sweep and the one-cell pit spreading | K7, `sediment_pass` (GEN-151) |
| Coarse-grid caller | `sediment_pass` calls `thermal` with no fixed cells | `bake/sediment.rs` |

## Evidence
| Run | Grid | Thermal's effect | Source |
|---|---|---|---|
| Prototype landmass, sediment pass | 3,072 cells a face edge (k = 1) | pit share 1.024 → 1.038 %; p99 depth 5.4 → 29.8 m; deepest 5.5 → 609.7 m; the deposition fills them to at most 5 m | `openspec/changes/archive/2026-11-23-sediment-wiring/proposal.md` (its design notes in git history, removed at the archive by ADR 0055) |
| Same, k = 2 | 6,144 cells | deepest after thermal 920 m; 344.5 m remain after the deposition; 6.5 % of the final pit cells were made by the pass (thermal, deposition and spreading together) | `openspec/changes/archive/2026-11-23-sediment-flood/proposal.md` (its design notes in git history, removed at the archive by ADR 0055) |
| Mechanism (k = 1, deepest pit) | 3,072 cells | a one-cell canyon floor between walls of 1.5-2 km; thermal lowers the walls onto the floor cells by unequal amounts (79-108 m) and leaves a dam | same |
| R3a and R3b windows, sediment pass at pass-2 parameters | 101.8 m | moved nothing: volume change 0, pits 4.55 → 4.55 % and 3.67 → 3.67 % | `spikes/prototype-gates/results/results.md` |
| Terrain pipeline review, pass 2 | 101.8 m | 0.58 M cells still moving at the pass cap | `spikes/terrain-pipeline-review/results/results.md` |
| R1h tile (65, 7), K7 with channels fixed (production) | 101.8 m, 1.04 M land cells | the post-incision pass (thermal, deposition, spreading) lowers pits 0.456 → 0.144 %; thermal exports 4.45e6 m3 into channel cells (deposition 1.35e7 m3) | this brief: `cargo run --release -p planet-generators --example k7_region` at main 37b26e54 (log not committed) |
| Same tile, K7 without channels | 101.8 m | the same pass raises pits 0.010 → 0.032 % and the deepest 5.7 → 32.3 m | same |

Reading:
- By count thermal adds little (+0.014 points of land at k = 1). It makes the **deepest** dams, and only where a cell is much wider than a talus slope: at 3 km cells a 33-degree difference is 2 km between neighbours, which only single-cell canyon walls reach.
- At L the burned channel cells already act as sinks (GEN-176) and absorb thermal's talus; without them thermal and deposition leave pits up to 32 m on hilly ground.
- The claim of the review (a "large share of the remaining pits") holds for depth on coarse grids, not for count at L.
- K7 reports thermal, deposition and spreading as one stage; thermal's own share at L is not separated (the first fixture below separates it).

## Options
| | Option | Change | Cost | Risk |
|---|---|---|---|---|
| A | Channel cells as sinks on every grid | `sediment_pass` passes H1's channel cells as fixed cells to `thermal_fixed`, as K7 does (GEN-176's rule on the coarse grids) | none at run time (the mask exists); one requirement delta, tier A fixture | dams in channels below the initiation area stay; the exported volume grows (G-SED already books it) |
| B | Drainage-preserving thermal | a transfer may not raise a receiver above its downstream cell's level on the route (or a breach limited to thermal's own deposits after it) | one route and flood a tile (reuse the deposition sweep's route); estimated +10-20 % of the sediment pass, not measured | a new rule inside a kernel with bit-exact and active-set guarantees; order effects between the cap and the route |
| C | Gate thermal by cell size | thermal runs only where the cell is at most about 200 m (L and finer); coarser grids skip it or use a slope cap scaled by cell size | none; less work on coarse grids | coarse previews differ from L in steep places; G-SED's thermal term is 0 there |
| D | Run to convergence | raise the pass cap (300 → until nothing moves) | 0.58 M cells still moving at the cap at L; the pass count is unbounded | does not remove dams: the dam is the converged state of the kernel |
| E | Keep, rely on the deposition | no change: the deposition fills thermal's pits to at most 5 m at 3,072 cells | none | 344.5 m dams remain at 6,144 cells; deeper at 12,288 (not measured) |

## Recommendation
**A plus C**, both easy to reverse.
1. A removes the mechanism where it was measured (talus into a one-cell channel floor); K7 already does it at L, so the rule becomes one rule on every grid.
2. C stops applying a 33-degree repose to cells of 400 m to 3 km, where it describes no landform.
3. B only if fixture 1 below shows dams in channels under the initiation area after A.

## Fixtures and sweeps (ADR 0112 condition 2)
1. Tier A: a one-cell canyon between 1.5 km walls (the k = 1 case): pits and dam height after thermal, with and without fixed channel cells.
2. Tier A: a channel below the initiation area beside a steep wall: the case A leaves open.
3. Sweep on the R1h tile and the prototype landmass: theta 30 / 33 / 37 / 40 deg, k 1/16 / 1/8, perturb 0 / 0.25, cell 102 / 204 / 407 / 814 m: pit share, deepest pit, moved volume, passes run.
4. K7 reports thermal as its own stage (pits after thermal, before the deposition).
