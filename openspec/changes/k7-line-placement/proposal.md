# k7-line-placement (proposed, for the lead's answer)

Why: K7 rasterises each H1 step (814 m) as a straight line between H1 cell centres. At L (101.8 m) that line leaves the valley floor:
- F3, the bank: at frame-43680 the line runs 15-20 m up the valley side, the carved low lies 130-150 m beside it (SPIKE-113), and the fixed line stands as a dyke above it.
- The cuts: with the network burn levels of GEN-185 (change k7-bed-coherence), a line that crosses a ridge of the start surface is cut through it. In R3a's commit tiles, 6.75 % of the line cells are cut more than 50 m (p95 60 m, p99 159 m, max 739 m). The owner rated cuts through ridges a fault (2026-11-13).
- Both come from the same cause: the line at L is not where water at L flows.

What: between two H1 vertices, the line at L follows the lowest pass of the base `B` within a corridor around the straight step, instead of the straight raster.
1. The corridor is the cells within 4 L cells (half an H1 cell) of the straight step from vertex to vertex.
2. Each vertex is the lowest cell of `B` within 3 cells of its H1 cell's centre, ties by cell id. A tributary's last vertex is the same H1 cell as the main segment's first, so both pick the same cell.
3. The path minimises the highest `B` along it (the lowest pass), ties broken by the summed height, then the cell order. It is 8-connected, deterministic, and a function of `B` and the two vertices only.
4. The burn levels of GEN-185 run along the placed cells. The burn, the fixed cells and the final pass use the placed line.
5. The drawing places the line the same way at read time, from the bake's `B`: no format change, and the stored graph keeps H1 vertices. This needs the exact `B` under `--commit` (VIEW-072, explorer); a reader whose `B` differs refuses to draw the river rather than draw it elsewhere.
6. Placement on `B` alone, not `B + regional`: the regional change is not in the commit at read time (only `L` is). The regional change is smooth at 814 m, so within a 400 m corridor it shifts a floor by little. The test measures that shift on R3a.

Fallback: if read-time placement fails its test (the drawn line differs from the burnt line on a tile), the graph stores the L cells of each step (a format change; the owner's call).

Tests:
- Tier A: a valley offset 2 cells from the H1 line: the placed line runs on the floor, and after K7 no bank stands between it and the floor (GEN-186).
- Tier A: placement on a window and on another overlapping window gives the same cells (partition independence), and a tributary and its main segment meet in one cell.
- Tier A: on TEST-134's fixture, no rise, no burn cutoff, and the share of line cells cut over 50 m falls.
- R3a (k7_line_check on a re-bake):
  - the share of line cells cut over 50 m inside the commit's tiles, against 6.75 %;
  - the line's height above the cross-section's low at frame-43680, against 15-20 m;
  - the bake's placed cells against the reader's on every tile, which must be equal.
- Stills of the F2 and F3 spots, before and after, through the production path.

Requirements: GEN-186 (reserved: no bank beside a line; the line placed on the low of `B`). IDs needed from the lead: one for the read-time placement in the drawing (renderer or streaming-cache), and one test ID. Depends on VIEW-072. Merges with k7-bed-coherence (lead, 2026-11-13).

Open for the lead:
- the corridor half-width: 4 cells (an H1 cell) or wider;
- placement on `B` alone, against storing the regional change in the commit (a format change);
- whether the final pass keeps its bounded carry once the placed line follows the floor (expected: few binds).

## Result on R3a (2026-11-13, not accepted)
| Lines and levels | Line cells cut > 50 m inside the commit's tiles |
|---|---|
| straight raster, network levels (A'') | 6.75 % |
| placed (lowest pass of B, snap 3) | 13.28 % |
| placed, no snap | 10.30 % |
| placed, start surface drained (lakes over D = 5 / 20 / 50 m) | 10.17 / 10.20 / 10.37 % |
- 32-84 % of the line length lies in closed pits of the start surface (window-local flood, three tiles); the region-wide fill finds 27,636 pits, the widest 1,650 cells across.
- The drain costs 22 s and 1.5 GB for R3a's 66.6 M cells (113 s with the front).
- Conclusion: most deep cuts are H1 lines crossing L ridges that water at L does not cross; the source is the network's geometry (H1 receivers), not the levels or the placement within half an H1 cell. Sent to the lead for an ADR (rivers routed at L).
