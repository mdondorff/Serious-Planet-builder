## Context

Background report §2 (CDLOD), §15 (M2). ADR 0003 (tile ids), ADR 0009 (selection is a pure CPU function).

## Decisions

- **Split rule:** a node is split while `surface_distance < split_distance(level)`, where `split_distance` is the distance at which the node's sample spacing (face-centre worst case) projects to `tau_px` pixels. Defaults: 1080p, 60 degrees, 6 px, 32 cells.
- **Distance is to the reference sphere,** not to terrain: padding bounding spheres by the maximum terrain height forced full subdivision around a ground camera (80k nodes for ten views). Height only pads culling.
- **Horizon culling inside selection,** with the camera's and the highest terrain's horizon angles. Orbit view: 50 nodes; ground view: about 6,400 (about 350 per level, mostly the ring around the camera).
- **Balancing:** leaves are split until neighbours differ by at most one level (across face edges too, using tile neighbours).
- **Morph:** a leaf morphs from 0 to 1 over the last `morph_band` of its parent's split distance, measured to the parent's bounding sphere, so a child is exactly fully morphed when its parent stops splitting (no pop on merge). Tested by sweeping the camera away from a nadir point in 3 % height steps (merges happen at about 20 levels).
- **Not yet:** frustum culling (needs camera orientation; would cut the ring by about 3x), per-vertex morph and skirts (render change), height-aware distance.

## Risks

- Node counts are high for a ground camera until frustum culling exists; the budget test (8000) will be tightened then.
- Tau, cells and band are tunable parameters; the perf measurement at the M2 gate decides them.

## Review follow-ups (verifier)

- Horizon culling used the chord radius as angular radius and could cull visible tiles (up to 229 km at level 0); it now uses the true angular radius (max angle over corners and edge midpoints, 0.1 % margin). Regression cameras (random and beyond a cube corner) and below-surface cameras are tested; a camera below the reference sphere is treated as sitting on it.
- `balance_leaves` is public and tested on hand-built unbalanced trees (including face edges): the distance rule alone already balances almost all real views, so only a direct test exercises it.
- Morph continuity now also asserts a step bound within one level; the wall-clock test was removed; LOD-001 and LOD-002 wording was aligned with what exists (plan integration comes with terrain draws; snapshots are per-view counts and a hash).
