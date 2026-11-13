# e1-start-pose

Why: the owner flew `editor --seed 14 --commit target/stores/r3a-e1` (lead, 2026-11-13) and saw coarse blocks and no lakes: the editor dived over the default site (20 N 30 E), 17,800 km from the R3a region, over the 1024-cell in-memory bake (9.8 km cells; the region's layer is 102 m).

What: with `--commit` the viewer starts held still at a single camera over the commit's region: the eye 25 km over the ground, 20 km south of `--site` or else the centre of the commit's tiles, looking north 50 degrees down (the E1 cluster still's pose). The title says "inside the region" or "outside the region: coarse base" by the ground under the camera; the log prints the start line. `WorldTerrain::commit_region` (the commit's L tiles; `contains`, `centre`).

Requirements: VIEW-069 (MODIFIED: the start pose and the title's region status). Source: lead 2026-11-13 (E1 reopened).
