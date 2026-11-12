## Decisions
1. **Where.** The cache crate (`bake_l` lives there and cache depends on generators). A library function with closures, an example as the command; an xtask wrapper later if wanted.
2. **Pipeline (from `generators/examples/k7_region.rs`, which stays as the measurement tool):**
   1. regional window = region plus margin (regional cells); `zb` = base at the regional cell centres (`FaceSpec::halo_direction`);
   2. `solve_rounds` with the production `CarveParams` (rounds f32, pre-breach profile, cell metric, window cut) into `zs`;
   3. runoff, evaporation and discharge weight from the climate closure on land cells; `route_rules`, `priority_flood_with`, `hydrology` (runoff in m/s: without it Q is an area);
   4. `lakes_from_hydro`, `ChannelGraph::from_hydro`, `channels_from_graph` with `to_local(c) = 8 (i0 + c % w) + 3.5` (a regional cell is 8 local cells);
   5. `LBake` with `MemoryFeed` (discharge), `Lakes`, `GraphInput` (`cell_xy`: the region's cell to `(face, i, j)` on the face's H1 grid), `h1_cell` answering only the face's own H1 cells (the region stays in one face), tiles = the land tiles of the region;
   6. `bake_l` writes tiles, basin tables, graph blocks, the commit.
3. **Sizes** are a spec (`lattice`, `regional`, `local` cells a face edge, tile, halo); the production values are 16,384 / 12,288 / 98,304 / 1,024 / 64; the fixture's are small with the same ratios where the code needs them (local = 8 regional).
4. **Identity.** `base_id` and `inputs_id` are given by the caller (the example derives them from `class_inputs_key`), as the writer's tests do; a commit is read back only with the same spec.
5. **No thread dependence.** Each stage is the one the tests already show thread independent; the new test asserts the bytes at one and eight threads.
6. **Cost.** R3a: regional solve and H1 minutes, K7 3 to 6 minutes a tile at low load; 16 tiles about 1 to 1.5 h on 4 threads (to be replaced by one measured run).
## Risks
- The graph's face mapping (`cell_xy`) and the regional-to-local mapping are the two places a one-cell error would hide: the test checks a river's cells against the known H1 cells of the fixture.
- A region at a face edge has the PR 7 limit (no cross-face channels): the spec refuses a region within 4 regional cells of a face edge.
