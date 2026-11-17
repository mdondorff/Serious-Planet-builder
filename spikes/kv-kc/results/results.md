# SPIKE-132: KV and KC on the G0 fixture (G1f, early)

Opened: 2026-11-17 (lead #157)
Question: do KV (valleys around the graph's beds) and KC (non-basin pits) hold the G0 contracts' budget, divides, protected basins and exceptions, switchable, on synthetic graph inputs before KG lands?
Why it matters: G1f of the river-graph plan; the addendum's section 6 lets the terrain lane prototype on synthetic inputs.
Time box: 1-2 days (used: about 0.5 day).
Success criteria: each switch (KV only, KC only, both) keeps the budget (no overrun, every clamp recorded), leaves the divide buffer, the protected basins and the antecedent ridge's exception as the contract says, and reports per-stage use and residuals; cross-sections of the trunk, a confluence and the ridge.
Fallback if it fails: name the contract rule the prototype cannot meet.
Code location: branch `spike/kv-kc` (local, never merged):
- `crates/generators/src/bake/kv_kc.rs`;
- `crates/generators/tests/kv_kc_g1f.rs`.
Inputs: the fixture's expected graph (TEST-140: L beds, `q_w`, the trunk's `exception(rise_over_budget)`) on `S0` with KS off (labelled). The protected basins are `S0`'s kept L basins: reading 5 names KS1, and with KS off KS1 is `S0`. The budget is the fixture's: the 10 m floor everywhere on this tiny body.

## 1. Stages (plan sections 3.2 and 3.4)
| Stage | Rule |
|---|---|
| KV | Around every L vertex, `V(x) = bed + S_wall max(0, d - W_floor/2)`: `S_wall` 0.20, `W_floor` = 3 x GEN-165's width of `q_w`, `d` the chamfer distance to the nearest vertex (ties by cell id). KV lowers to `V` and never raises. Weight zero within `d_buf` = 200 m of a divide and on protected cells. The lowering is held at the budget left; a clamp writes a residual (feature: the nearest reach). |
| Divide | Cells whose D8 paths (the graph's routing seed) reach different river chains (reaches joined across tile crossings) or the sea; a path reaches a chain within 1.5 cells of a vertex. |
| KC | The cells of KV's output in a depression of the hierarchy (SPIKE-130's sweep), outside protected basins, filled to their tree's spill height within the budget; a deeper fill is held at the budget with a residual. Patches counted with their volume. |
| Switch | KV only, KC only, both; checked with GEN-189's `validate_budget` |

## 2. Results on the fixture
| Measure | KV only | KC only | KV + KC |
|---|---|---|---|
| KV: cells lowered; lowering p50 / p99 / max, m | 1,310; 4.48 / 10.00 / 10.00 | 0 | 1,310; 4.48 / 10.00 / 10.00 |
| KV clamps (residuals) | 525 | 0 | 525 |
| KC: cells filled, patches, volume | 0 | 31, 2, 2.22e6 m3 | 29, 2, 2.10e6 m3 |
| KC fills held at the budget (residuals) | 0 | 21 | 19 |
| Overruns, raises in a lowering stage, unrecorded clamps | 0 | 0 | 0 |
| Divide cells (within `d_buf`); KV changes there | 3,474 (6,741); 0 | same; 0 | same; 0 |
| Protected cells; changed | 243; 0 | 243; 0 | 243; 0 |
| Trunk: largest lowering (budget 10 m); rise over its running minimum after the stage | 10.00; 261.9 m | 0; 271.9 m | 10.00; 261.9 m |
| Residual features already exceptional (cannot be flagged `budget_residual`) | 458 | 21 | 477 |
| Same bits on a second run | yes | yes | yes |

- The antecedent ridge's exception is left uncut: the crest falls only by the budget (345 to 335 m along the trunk), and the rise stays.
- KV is nearly idle on this body. The 10 m budget holds 525 of 1,310 lowered cells at the cap. The valley floor upstream of the ridge lies in the protected basin that `S0`'s C1 trough forms behind the crest, so KV only touches the walls there.
- KC fills the reported basin at 26.3 E (floor 135 m, spill 234.5 m) by its 10 m budget, and a 5-cell coastal pit.

## 3. Findings
1. The prototype meets the switches' hard rules on the fixture:
   - no overrun, every clamp recorded;
   - divides, protected basins and the ridge exception kept;
   - deterministic.
2. Contract gap, a fill clamp: GEN-189's `Stage` states a wanted lowering, not a wanted fill. A fill held at the budget (KC: 19-21 cells) leaves a residual the validator reads as recording nothing. The test checks those apart: the fill equals the budget and the need is above it. Proposal: a `wanted_fill` per stage, its clamp recorded like a lowering clamp.
3. Contract gap, one status per reach: the residual's feature must be `exception(budget_residual)`, but the trunk is already `exception(rise_over_budget)`. That is 458-477 residuals whose feature cannot carry both. Proposal: a list of exceptions per reach, or the residual list as the record for a reach that is already exceptional.
4. Divides meet the rivers at every confluence and junction, so the 200 m buffer leaves KV idle there. The confluence section shows it: every cell is in the buffer. G1f's integrated run should measure the share of graph cells inside the buffer.
5. With KS off, `S0`'s dam basin behind the ridge protects the valley floor upstream (reading 5's reason for KS1). On the integrated run KS1 removes it, and KV then acts on the floor.

## 4. Recommendation
- Keep the KV and KC structure for G1f: cross-section from the nearest vertex, divide buffer from chain catchments, KC from the hierarchy (the flood's grading ramp on flats would make plateau patches).
- Lead decisions for the contract: a wanted fill for fill clamps (finding 2); several exceptions per reach (finding 3).
- Run KV and KC on KS1 once G1b lands, before judging valley shape.

## Visuals
![Cross-sections](images/sections.png)
The KV + KC run on the G0 fixture (CPU, no GPU path), budget 10 m, KS off. Panels: across the trunk at 10 E; across the explicit confluence (every cell within the divide buffer); along the trunk over the antecedent ridge (crest lowered by the budget only, the pit at 26.3 E filled by KC); across the ridge at 25 E. Dots: graph vertices (red), protected basin (blue), divide buffer (grey). CSVs in `sections/`, the run's report in `report.txt`.
