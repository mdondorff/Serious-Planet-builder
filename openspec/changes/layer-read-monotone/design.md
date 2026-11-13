# Design: layer-read-monotone (GEN-188)

## The defect
- The read was `clamp(CR_y(CR_x(rows)), min4, max4)`: Catmull-Rom along each row, then across the rows, clamped to the four cells around the point.
- The four cells are a function of `(floor(x), floor(y))`. At `y = j` (a cell-centre row) the lower quad uses rows `j - 1, j` and the upper quad rows `j, j + 1`. The unclamped value is the same on both sides (C-R along row `j` at `fx`), but where it overshoots the pair `cell(i, j), cell(i + 1, j)` it can lie inside one quad's range and outside the other's: the clamp cuts only one side, a step of up to the overshoot (7.4 % of a step for C-R; 12.5 m measured on R3a).

## The read
- Per axis: the Hermite cubic through `p1` (t = 0) and `p2` (t = 1) with the slopes `m1 = hm(p1 - p0, p2 - p1)`, `m2 = hm(p2 - p1, p3 - p2)`, `hm(a, b) = 2ab / (a + b)` when `ab > 0`, else 0.
- Monotone: `0 <= m / d <= 2` for both slopes against the segment's difference `d` (inside Fritsch-Carlson's region, `<= 3`); `d = 0` gives zero slopes and a flat segment. The 1D value stays in `[min(p1, p2), max(p1, p2)]`; the row values stay in their pairs' ranges, the cross pass stays between two of them: the 2D value stays in the four cells' range without a clamp, so GEN-102's pyramid ranges still hold.
- Continuity: the slope at a sample is a function of the three samples around it only, so the two segments meeting at a sample have the same value and slope there (C1). `hm` is continuous in `(a, b)`, also where the sign changes (it tends to 0), so the cross pass is continuous across the column lines too.
- Exact samples: the basis form `h00 p1 + h10 m1 + h01 p2 + h11 m2` is exactly `p1` at `t = 0` and `p2` at `t = 1` in floating point (each basis polynomial is exactly 0 or 1 there). A cell coordinate within `CENTRE_SNAP_CELLS` (1e-9 cells, 0.1 µm at 101.8 m) of an integer is moved onto it, so a direction made from a cell's indices (whose round trip leaves about 1e-13 cells) reads the sample bit for bit; the snap moves the read by at most `1e-9` cells times the slope.
- libm only (`round`); no new operation depends on the thread count (CON-14).

## Cost
- Two harmonic means per 1D pass (5 passes a read) replace the C-R polynomial and the clamp's four extra cell reads: about the same work.

## Rejected
- Per-axis clamp of each 1D C-R pass to its pair's range (the lead's fallback): continuous, but C0 where clipped (a kink along the clipped places).
- Keeping C-R and blending the clamp over a band near the lines: still C-R's overshoot, more parameters.
