# 0114 Simulation profile, cell metric and canonical forms
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10 and point 6 (the toolchain pin). Production: G-EQ (the f32 form) and G-DET on both operating systems at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4.2, §8, §9; SPIKE-105 F5, §11 (points 3, 9B, 9C), §13.5 (D9, D10), §18; ADR 0010

## Context
- An edit must not advance erosion by an arbitrary amount of time; simulated time and editor time are separate.
- A uniform cell area and distance bias drainage area by 18 % on average, river initiation by 7-14 % of channel cells and slopes up to 1.5x at corners (F5).
- A re-solve and a fresh build must agree bit for bit (CON-14); reading in-memory f64 in one path and stored f32 in the other breaks that (`bake/tile.rs:322`).
- A chaotic carve has no pointwise reference; correctness is structural.

## Decision
1. A `SimulationProfile` in the WorldDefinition holds every parameter that can change a result: rounds, steps per round, `K dt`, quality, process and routing parameters, the seed policy and the quantisation constants (A3 §8). It is immutable for a solve; simulated duration is rounds x steps x `K dt`. Every solve starts from the definition.
2. Every grid exposes true per-cell area and neighbour distance from the mapping; the uniform `dx` is retired; constants that read area are re-tuned once.
3. Three representations are kept apart: the canonical persistent form, the stage computation, the runtime composition (f64). Each value's forms and its single conversion point are in A3 §9's table.
4. Each value one stage passes to another has one canonical stored form; both a fresh run and a re-solve read that form. Round surfaces are f32 on the regional grid, narrowed once at the end of a round; a kernel computes in f64.
5. Determinism is CON-14, CON-28 and CON-30. This ADR adds: each source of divergence has a named control (A3 §9), shown by the actual builds (G-DET), not inferred; ties break by global cell id.
6. Open, proposed for the owner: a toolchain pin before the world build; CI uses `stable`.
7. Reproducibility (bit equality) and correctness (connectivity, conservation, seams, locality, relief) are separate gates.

## Consequences
- Extends ADR 0010; uses ADR 0003's mapping. Changes erosion constants through the metric (D9).
- Gates G-EQ, G-DET. CI's determinism manifest must add a fixture of the round solver and H1.
- Point 4's f32 round surfaces are not yet built (main is f64 across rounds); GEN-166 builds them (architect D24, D27).
