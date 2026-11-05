## ADDED Requirements
### Requirement: GEN-128 Arid class rules
The default class source SHALL give the badlands class a weight where the rock is soft, the climate semi-arid, the height moderate and the relief broken, and the sand-sea class a weight where the climate is arid, the height low and the sand supply high, by smooth ramps (no class weight steps beyond GEN-097's limit between lattice cells), from the aridity of GEN-117 and the hardness of GEN-116. The weights SHALL be blended over the base weights at read time, sum to 1 with each rule capped, be a pure function of the seed, and be keyed by `ARID_RULES_VERSION`; the climate SHALL NOT read them.

Verify: A
Status: planned
Source: owner 2026-11-05, ADR 0104 point 4, planner inbox #73, #74, GEN-095, GEN-116, GEN-117

#### Scenario: The rules
- **WHEN** the weights are read over the class lattice of a seed on soft semi-arid broken ground, on arid low supplied ground and elsewhere
- **THEN** badlands has weight only on the first, sand sea only on the second, every sum is 1, no `k` step exceeds the limit, and the keys differ from those without the rules
