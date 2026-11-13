## ADDED Requirements
### Requirement: VIEW-072 The base under a commit is the bake's base
A world that opened a commit in memory (VIEW-069) SHALL give its base B as the bake read it: `bake_base()` returns the C1 read (GEN-107) of the exact lattice of `BakeInputs::base` over the commit's rectangle, equal bit for bit to `bake_region`'s base closure at every cell of the commit's tiles, and the commit's `base_id`; every other world returns none. The drawn ground is B plus the crease detail of the field (GEN-109) plus the layer's change, so it differs from B + L by the detail and not by the base.

Verify: B
Status: active
Source: Lead 2026-11-13 (P2 E1; the premise was a base difference, the probe found none), crusader's #125 line check, VIEW-069, REND-067

#### Scenario: B at the cells of the commit
- **WHEN** a commit baked over a region is opened in memory and B is read at cells inside its tiles
- **THEN** each value equals the C1 read of the production base sampled at the lattice nodes, bit for bit, and the id is the commit's `base_id`

#### Scenario: Another world
- **WHEN** a world of the in-memory bake is asked for its bake base
- **THEN** it has none
