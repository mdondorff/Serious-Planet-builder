# 0114 Simulation profile, cell metric and canonical forms
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100); the owner accepts after the external review and the D2-D10 pass
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4.2, §8, §9; SPIKE-105 F5, §11 (points 3, 9B, 9C), §13.5 (D9, D10)

## Context
- An edit must not advance erosion by an arbitrary amount of time; simulated time and editor time are separate.
- A uniform cell area and distance bias drainage area by 18 % on average, river initiation by 7-14 % of channel cells and slopes up to 1.5x at corners (F5).
- A re-solve and a fresh build must agree bit for bit (CON-14); reading in-memory f64 in one path and stored f32 in the other breaks that (`bake/tile.rs:322`).
- A chaotic carve has no pointwise reference; correctness is structural.

## Decision
1. A `SimulationProfile` in the WorldDefinition holds rounds, steps per round, `K dt`, quality and process parameters; simulated duration is rounds x steps x `K dt`. Every solve starts from the definition.
2. Every grid exposes true per-cell area and neighbour distance from the mapping; the uniform `dx` is retired; constants that read area are re-tuned once.
3. Each value one stage passes to another has one canonical stored form; both a fresh run and a re-solve read that form. Round surfaces are f32 on the regional grid.
4. Determinism stays CON-14 and ADR 0010: Windows and Linux on x86_64 supported; no GPU feedback; ties by global cell id.
5. Reproducibility (bit equality) and correctness (connectivity, conservation, seams, locality, relief) are separate gates.

## Consequences
- Extends ADR 0010; uses ADR 0003's mapping. Changes erosion constants through the metric (D9).
- Gates G-EQ, G-DET.
