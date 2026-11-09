## Decisions
- A compact cube grid is laid out as its six faces stacked, `n` wide and `6 n` high (`Grid::layout`); `xy` and `at_xy` map a cell to its (x, y) and back (`None` off the landmass).
- A tile is a square of that layout; the tile flood, border offers, area sweep and memo key work on the slots of a tile, skip a slot off the landmass, and hash a skipped slot as a fixed word.
- The memoised path keeps a flood memo per tile, the previous result and a catchment memo per round. Those caches are the edit path's (GEN-143, G-INC) and cost about 340 bytes a cell at 37e6 cells (11.7 GiB; `landmass_scale part` before `without_memory`). A solve that is not edited again uses `without_memory` caches.

## Measurements (`examples/landmass_scale part`, 8 threads, tiles of 256 cells, 4 rounds; measurements, not gates)
| Cells a face edge | Cells | Peak (MiB) | Bytes a cell | Time (s) | Resident route, bytes a cell |
|---|---|---|---|---|---|
| 3,072 | 9.4e6 | 627 | 70 | 177 (resident 73) | 85 |
| 6,144 | 37e6 | 2,255 | 63 | 724 | 62 |
| 12,288 | 149.3e6 | 8,854 | 62 | 2,974 | 55 |

- With the memo caches kept the partitioned rounds peaked at about 340 bytes a cell (12.1 GiB at 37e6 cells), over the budget.
- Without them the peak is 62 to 70 bytes a cell, 8.65 GiB at 149.3e6 cells: under 12 GB (ADR 0115 point 7) for the rounds alone; the H1 pass (12.1e9 bytes extrapolated) and the whole build's peak are not measured.
- The partitioned rounds are bit-equal to the resident rounds (3,072: 0 cells differ; tier A: tiles 4 to 24).
- Time is about 2.4 times the resident rounds at 3,072.

## Not done
- Item 9 stays open: the arrays (filled, receivers, area, order, catchment results) are resident. Tiles bound what each step touches, not the memory. A tile-wise on-disk form is not built.
- The H1 pass at 12,288 cells and the whole build's peak are not measured.
- Cache eviction for the edit path is not added.
- Earth-specific values (ADR 0108): the cell counts above are the prototype landmass at 12,288 cells a face edge; they are examples, not rules.
