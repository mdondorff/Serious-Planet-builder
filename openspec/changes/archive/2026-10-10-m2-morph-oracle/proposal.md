## Why

ADR 0009 review follow-up 4 (docs/status.md on change/adr-0009-review, owner asked): the plan's node-level `morph` value was uploaded in the node uniform but never read by the shader, and the shader's per-vertex morph had only a tier-C test with a loose bound (0.06, 90 % of five vertices).

## What Changes

- The node uniform no longer carries the dead values (`morph`, level): it holds the morph interval only. The plan keeps `TerrainDraw::morph` as a documented CPU diagnostic (it stays in the plan text and snapshots).
- New tier-B oracle `crates/render/tests/oracle_morph.rs`: every visible interior vertex (not level 0, not near the screen edge or grazing, not hidden by a nearer node) of every node in three scripted views, about 30,000 vertices, is compared with the f64 CPU twin of the shader formula; the bound is derived from the data (the neighbour steps weighted by how far the pixel centre lies from the vertex in pixels, times 1.5, plus 2/255 of 8-bit quantisation), and the worst error must stay below 0.05. The verifier's mutation probes (distance x1.002, +0.01 on the morph value) pass a looser first version of the bound; the tightened bound fails both.
- Added: REND-014 (vertex-shader morph, tier B).
