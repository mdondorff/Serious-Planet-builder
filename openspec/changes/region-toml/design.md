## Design notes
- Coordinates: L tiles of the bake's layout, inclusive; the production layout is the G-RT fixture's (12,288 regional cells, 768-cell tiles, 128 a face edge, halo 64, 4 rounds, the 16,384-node lattice, margin 200 regional cells). A tile is 96 regional cells.
- `RegionLayout` carries the layout so the end-to-end test runs on a small one of the same shape (128 regional cells, 16 tiles); `RegionLayout::PRODUCTION` is what the command and the editor use.
- The allowed band: tiles `ceil(m / 96)` to `floor((12,288 - m) / 96) - 1` with `m = max(margin, FACE_EDGE_CELLS)`: 3 to 124. The test checks that the band is exactly `RegionSpec::check`'s (the tile just outside fails it too).
- State: the commit records `effective_inputs_id`, which hashes the box, the layout and the caller's inputs id; `region_inputs_id = class_inputs_key ^ definition hash` (the hash holds the items). No status is stored.
- The base: `BakeInputs::base` on the layout's lattice over the solved window plus two regional cells and three nodes, read C1; the base does not read items today (the class inputs are the seed's), so items enter the identity only. The commit opens over the same in-memory base (VIEW-069, VIEW-072); the test opens it.
- `--seed N` uses the viewer's single planet of radius 6,371 km, so `--commit` with the same seed opens over the same definition.
- A rebake writes into the existing folder: the tiles are keyed and the commit file is replaced atomically.
