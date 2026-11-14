## ADDED Requirements
### Requirement: STRM-077 The fill depth is written by a post-pass over the tiles' final heights
Pass 2 SHALL leave each tile's final core heights (f32 metres, `tile x tile`) as record kind 18 (`LayerHeights`) in a scratch store of its own, keyed like the channels; the post-pass `layer_fillpass::run_fill_pass` SHALL read only those and write kind 17 (STRM-076):
- stage 1: the labelled flood of each core (GEN-121; the outlets are the cells at or below the sea level that the field bake's ocean connectivity (GEN-022, ADR 0041 point 2) calls ocean, read at the cell's direction: ocean when any of the four bake vertices of the lattice cell holding the direction is ocean (the water field's wet rule without the wet-floor term: a cell below the sea level on a dry vertex quad is an inland basin even where the water field draws a lake); a cell at or below the sea level that it does not call ocean is an inland basin, filled like any pit and counted in the report as `inland_low_cells` (of the tiles written); without a field store every such cell is an outlet) gives its territory edges and, for each border cell, its seed and height; then the graph: the tiles' edges and every pair of neighbouring border cells of two tiles of the run (`fillgeom::border_neighbours`: inside a face and across a cube edge or corner by the direction mapping the readers use), `w = max(z, z)`; a border cell whose neighbour is in a tile that is not in the run drains to the sea there, at its own height; then stage 3, per tile: the flood again, the level `max(level, G(label))` and the depth `level - z` as 16-bit codes; nothing but the heights is kept between the stages;
- the depth SHALL equal the depth below the planet-wide spill level of the assembled surface (GEN-121), the tiles a run does not hold being sea (a run over a region needs a margin that holds whole basins);
- the post-pass SHALL write the same bytes on any thread count, skip a tile that holds a fill record under the current key, and refuse a tile whose scratch heights are missing, naming the tile; a cell whose level no path from the sea reaches is written as 0 and counted;
- the surface that is flooded is the pass-2 surface (the window's interpolated base plus the change); the depth written is taken against the exact base plus the stored change at the cells that are pits of that surface (the post-pass takes the base closure and reads the tiles' stored change), clamped at 0 (planner and owner path 2026-11-05: the interpolation ripple of the base, p99 26 m on a mountain tile, is taken out of the filled floor; the cells above the level stay as outcrops), and the report counts the clamped cells;
- the post-pass SHALL refuse a tile directory that was filled for another set of tiles (the depth depends on which tiles a run holds) or another outlet mask (a sidecar file `fill-pass-tiles.txt` in the directory holds the tile-set hash and the outlet identity, `all` without a field store, else the lattice resolution and the store directory's name, so two stores of one name are not told apart; a store copied without it bypasses the guard), do nothing when every tile of the run holds its fill, and refuse a run whose scratch store lacks a tile before it floods any; pass 2 SHALL NOT recompute a tile that holds its fill record only for its scratch heights; a neighbour pair that is dropped because the neighbouring cell is not on its tile's border is counted in the report;
- the keys of kinds 17 and 18 carry the post-pass kernels' version (`FILL_KERNEL_VERSION`); a bump re-keys the scratch heights too (a leftover scratch is refused, never reused);
- the depth is taken against the base pass 2 was run with (a caller's closure, not part of the key: the layer's `inputs_id` identifies the field); the filled change changes the erosion depth the channel reader derives in a basin by the removed ripple (STRM-075);
- the scratch store SHALL be deleted once the post-pass has run (`bake layer` does so): it holds 38 MB a tile before compression and is not part of the layer.

Verify: A
Status: active
Source: planner inbox #67 (seam fix, option (a)); GEN-121, STRM-076

#### Scenario: Bytes, resume and refusals
- **WHEN** the post-pass runs on 1 and on 8 threads over the 16 tiles of a face, runs again, is asked for a depth under another layer's key, and is given a tile the scratch store lacks
- **THEN** the packs are the same bytes, the second run writes nothing, the other layer's key is refused naming both keys, and the missing tile is refused naming it

#### Scenario: A region
- **WHEN** the post-pass runs over two tiles of a face whose basin spills into a tile it does not hold
- **THEN** the basin at the run's border is not filled: the tiles not held are sea; the same directory refuses a run over all the tiles of the face (another set of tiles)

#### Scenario: A rerun without scratch
- **WHEN** the post-pass has run, the scratch store is gone, and pass 2 and the post-pass are run again with a fresh scratch store
- **THEN** pass 2 recomputes no tile and the post-pass writes nothing and needs no heights

#### Scenario: The neighbours of a border cell
- **WHEN** the neighbouring cells of other tiles are named for the border cells of the tiles of a face and across a cube edge and corner
- **THEN** inside a face an edge cell touches three cells of the next tile, a corner cell five cells of three tiles, an inner cell none, and the relation is symmetric; across a cube edge the neighbours are border cells of the next face and mostly mutual, and a corner reaches two other faces
