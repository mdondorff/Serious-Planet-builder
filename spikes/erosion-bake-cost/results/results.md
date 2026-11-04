# SPIKE-98: cost of the erosion bake's pass 2

Question: which settings bring pass 2 to at most 100 single-thread CPU-s per tile, with pass 1 at 12,288 cells, without a visible loss.
Machine state: untimed-rule period (until Saturday); numbers are single-thread CPU-s per tile or ratios inside one session. Projections are B x single-thread, labelled as such.
Inputs: seed 14, pass-1 faces 2 and 0 at 12,288 (MFD area, C1 read), tiles 2/12/10 (mountains) and 0/8/24 (hills), `cost_probe --threads 1`, raw logs in `raw/`.

## Cost drivers (baseline, halo 256)
| Stage | Mountains s | Hills s |
|---|---|---|
| base sampling | 334.4 | 323.8 |
| carve | 23.2 | 27.5 |
| thermal | 17.4 | 1.2 |
| deposition | 13.2 | 17.0 |
| wall | 389.3 | 370.6 |

Sampling is 86 % (base evaluation about 24 us a cell over a 3,584 x 3,584 window).

## Candidates (same session, one thread, two tiles in parallel)
| Id | Setting | Mountains s | Hills s | Ratio to A |
|---|---|---|---|---|
| A | as built | 389.3 | 370.6 | 1 |
| B | base every 4 cells (interpolated), halo 128 | 68.2 | 60.4 | 0.17 |
| C | 200 m pass-2 grid (half the cells), base every 2, halo 128 | 28.7 | 25.6 | 0.07 |
| D | C with thermal 300 passes, k 1/8 | 32.2 | 25.9 | 0.08 |

Both B and C meet the 100 s target; C leaves a 3.5x margin.

## Error against A (core heights)
| Id | Tile | rms m | p99 m | max m |
|---|---|---|---|---|
| B | mountains | 4.02 | 16.8 | 412 |
| B | hills | 0.99 | 4.0 | 68.9 |
| C (A subsampled by 2) | mountains | 17.9 | 76.8 | 422 |
| C | hills | 5.3 | 22.8 | 96.5 |

The maxima sit in carve-route changes (a channel moving by a cell), not in a smooth bias. C is coarser by design (the 200 m grid), so its error is the resolution step, not the stride.

## Thermal residual at the cap (largest excess of a pair over the talus, m)
| Setting | Mountains residual m | Pairs over 1 m | Cells still moving |
|---|---|---|---|
| A, 100 passes, k 1/16 | 72.8 | 356,200 | 195,185 |
| B | 72.8 | 335,459 | 183,727 |
| C, 100 passes | 67.3 | 44,122 | 36,669 |
| D, 300 passes, k 1/8 | 13.7 | 3,941 | 16,182 |
Hills converge (residual 0.07 m, 0 pairs over 1 m). Mountains: the 100-pass cap leaves 73 m "teeth"; D cuts them to 13.7 m for 3.3 s on the coarse grid.

## Earlier measurements (same spike, other sessions, ratios only)
- Halo 128 against 256: identical core except near the edges; the seam step is the same.
- Base every 2 cells: 0.40 of baseline; every 4: 0.24 (halo 256).
- Pass 1 at 12,288 with MFD: 17-23 min a face at B = 10; two faces at once at 5 threads: 1,434 and 1,404 s.

## Projection (B x single-thread, indicative)
Pass 2 time = tiles x CPU-s per tile / B. At 28 s (C) or 65 s (B) a tile the 100 s target holds for any B >= 1 up to the tile count of a planet bake; the planet tile count is the owner's number, not set here.

## Recommendation
1. Halo 128 (small gain, no seam change).
2. Base sampled every 4 cells with interpolation (B) keeps the 101.8 m grid at 0.17 of the cost; the 200 m grid (C) needs an owner decision, it changes the layer's resolution.
3. Thermal 300 passes at k 1/8 where the residual at the cap exceeds 1 m (mountains), cost 3-4 s.
