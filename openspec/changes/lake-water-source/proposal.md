# lake-water-source

Why: E1 (P1, lead 2026-11-12): the viewer flies a `bake-region` commit with its lakes drawn. A commit holds the lakes (the basin tables of the six faces and each L tile's lake water mask), but nothing reads them into the frame's water: a basin shows as dry ground. Item 15 PR 4, split into 4a (the water field answers the lakes) and 4b (the water pass draws only the masked cells); `openspec/changes/runtime-composition/design.md` section 15.

What (4a): `planet_generators::lake_water::LakeProducer`, a `WaterProducer` over a `LakeMaskSource`; `planet_cache::lake_read::CommitLakes`, the source over a commit (the tables and the decoded masks, read under the commit the layer and the rivers are read under, CON-22); the viewer's water field lists it after the sea when `--erosion` opens a commit. A lake's body is packed as REND-058 states; the liquid is the sea's (architect, 2026-11-11).

Limit until 4b: a tile that holds a lake cell takes the lake's level for the whole tile's grid (the one-level-a-tile limit of `water.rs`), so the water is drawn wherever the ground is under the level inside that tile. 4b draws only the masked cells.

Out of scope: the water pass's mask texture (4b), a lake liquid kind (a schema change after CP2), lakes across a face edge (no region crosses one).

Requirement ID: REND-058 (reserved, design section 15 decision 1). Source: architect 2026-11-11 (PR 4 decisions), lead 2026-11-12 (E1 order).
