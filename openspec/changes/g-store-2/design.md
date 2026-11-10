## Decisions
- The field bake is the production sample (`global::class_sample`, seed 14) on the vertices of a tile of 208 km (blocks of (T+1)^2 vertices, T = cells a face edge / 48); the elevation is the K3 base field, which the new chain reads and erodes (A3 section 3).
- The narrow variant is a measurement encoding with stated steps: continent i16 at 1/4096, elevation i16 at 0.25 m, temperature u8 at 0.5 C, precipitation u16 at 1 mm, byte planes, brotli quality 5.
- The version 1 of the space study is the pack of `store_measure` (the new chain's items). An edit's footprint is the tile of a cell, its eight neighbour tiles and the tiles of its downstream path (the receiver chain of the final routing). Replacement records copy the size of the records they replace.
- Peak during a compaction is the directory's size sampled every 5 ms; it is at least the size beside.

## Measurements (prototype landmass, seed 14; examples, not gates)
Field bake, bytes a vertex, tiles sampled every 16th (k = 1 every 8th), by the grid the field is sampled on:
| Grid (cells a face edge) | Production block (4 x f32, byte planes, LZ4 as STRM-009) | Narrow variant | Tiles |
|---|---|---|---|
| 3,072 | 11.75 | 3.30 | 336 |
| 6,144 | 11.02 | 2.78 | 168 |
| 12,288 | 10.30 | 2.04 | 168 |

- At 12,288 cells (the Earth grid of the regional field): land tiles 10.42 and 2.14, coast 10.21 and 1.59, sea tiles 9.39 and 1.71 (17 coastal sea tiles; deep ocean was not sampled).
- Earth, 9.06e8 vertices with land at the land-tile rate and the rest at the sea-tile rate: 8.74 GB production block, 1.64 GB narrow (A3 table target 5 to 8 GB). Narrow errors at most 0.125 m, 0.25 C, 0.5 mm, 1.2e-4; no value clamped.
- The rate falls with the grid (smoother neighbours); 12,288 is the measured case, no extrapolation in the table above. Deep-ocean tiles are expected below the coastal sea rate (not measured).

Space of a replacement, bytes on the real packs (k = 2 landmass; the k = 1 run gives the same ratios):
| Scenario | Tiles | Steady | Beside | Peak in compaction | After | Added by the version |
|---|---|---|---|---|---|---|
| Median edit | 12 | 646,976,756 | 649,901,866 | 1,296,878,670 | 646,976,756 | 2,925,110 |
| Largest sampled edit | 34 | 646,976,756 | 655,920,252 | 1,302,897,056 | 646,976,756 | 8,943,496 |
| Full re-bake | 2,685 | 646,976,756 | 1,293,953,448 | 1,940,930,252 | 646,976,756 | 646,976,692 |

- Footprint of 400 sampled land cells: tiles p50 11, p90 19, max 34; record bytes p50 2.9 MB, p90 5.2 MB, max 8.9 MB at k = 2 (they scale with the cells a tile, about 4 times at 12,288).
- `pack::compact` writes the new packs before it removes the old ones: the peak is steady plus the live set (about 2 times steady for any edit, 3 times for a full re-bake with both versions kept). At a steady store of the sum below, the peak is about 37 GB (narrow) to 51 GB (production block).
- A version retained for undo costs the records it superseded: the added bytes of its footprint (median 2.9 MB of the new chain's items at k = 2, about 12 MB at 12,288, plus the field and `L` records of the same tiles: `L` alone is about 46 MB for 12 tiles at 3.8 MB a land tile (13.1 GB over 3,444 land tiles of 208 km), the field 1 to 8 MB, so an undo step of a median edit is estimated at 55 to 65 MB).

Edit items (canonical file text, brush items of 8-point strokes): 1 stroke 870 B (293 B brotli), 10 strokes 5,950 B (713 B), 64 strokes (the cap) 36,443 B (2,936 B). A thousand items of 10 strokes is 6 MB (0.7 MB brotli); the authored items are part of the definition, not the store.

## What the numbers give (steady state at Earth, GB)
| Item | Narrow field | Production block |
|---|---|---|
| Field bake (12,288) | 1.6 | 8.7 |
| Local change `L` (kind 13, l-store, one mountain-and-coast region; highest tile 16.7) | 13.1 | 13.1 |
| New chain's items (g-store; upper bound 4.7) | 3.9 | 3.9 |
| Sum | 18.6 | 25.7 |

- Both sums are under the 35 GB target; the upper bounds (L at its highest tile, items at 4.7) give 23 and 30 GB.
- `L` and the sea share of the field are the open caveats: `L` was measured on one region, the field's deep ocean was not sampled.
- The peak temporary space (compaction) is a separate figure: 2 times steady, 37 to 51 GB.

## Not done
- A pack-wise compaction, which would bound the peak by the packs rewritten; not designed, no departure from ADR 0019 or ADR 0024 made.
- Deep-ocean field tiles; `L` over more than one region; edit overrides as a stored record (the measured form is the item file; no per-tile override record exists).
- A planet build's temporary space on disk (the spill is 1.2 GB at the landmass, measured in `phase-plan`).
