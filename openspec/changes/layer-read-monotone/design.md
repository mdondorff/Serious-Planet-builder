# Design: layer-read-monotone (GEN-188)

## The defect
- The read was `clamp(CR_y(CR_x(rows)), min4, max4)`: Catmull-Rom along each row, then across the rows, clamped to the four cells around the point.
- The four cells are a function of `(floor(x), floor(y))`. At `y = j` (a cell-centre row) the lower quad uses rows `j - 1, j` and the upper quad rows `j, j + 1`. The unclamped value is the same on both sides (C-R along row `j` at `fx`), but where it overshoots the pair `cell(i, j), cell(i + 1, j)` it can lie inside one quad's range and outside the other's: the clamp cuts only one side, a step of up to the overshoot (12.5 m measured on R3a).

## The read
- Per axis: the Hermite cubic through `p1` (t = 0) and `p2` (t = 1) with the slopes `m1 = L(p1 - p0, p2 - p1)`, `m2 = L(p2 - p1, p3 - p2)`.
- The limiter `L(a, b) = 2ab / (a + b) * 2ab / (a² + b²)` when `ab > 0`, else 0:
  - `L(a, a) = a`: a uniform slope is kept at any scale (no terraces on gentle ground).
  - `0 <= L / d <= 2` against either neighbouring difference `d` (the second factor is at most 1, the harmonic mean at most twice the smaller): inside Fritsch-Carlson's monotone region (`<= 3`); `d = 0` gives zero slopes and a flat segment.
  - C1 in `(a, b)` where one difference changes sign (it vanishes as `b²`), except at `a = b = 0` (degree 1). The harmonic mean alone (Fritsch-Butland) is only C0 there.
- The 1D value stays in `[min(p1, p2), max(p1, p2)]`; the row values stay in their pairs' ranges and the cross pass stays between two of them: the 2D value stays in the four cells' range without a clamp, so GEN-102's pyramid ranges still hold. In floating point the containment holds to about 2 ulp (numerics review: at most 1.8e-12 m over 5M cases); the consumers pad their bounds by `1e-6 + 1.2e-7 |x|` (`erosion.rs`).
- Continuity across cell lines: the slope at a sample depends on the three samples around it only, so the two segments meeting there have the same value and slope (C1). Inside a cell the cross pass's slopes follow the row values with `x`; `L` being C1 there leaves no crease.
- Exact samples: the basis form `h00 p1 + h10 m1 + h01 p2 + h11 m2` is exactly `p1` at `t = 0` and `p2` at `t = 1`. A cell coordinate within `CENTRE_SNAP_CELLS` (1e-9 cells) of an integer is moved onto it: a direction made from a cell's indices round-trips to 2.8e-14 cells (256 cells), 1.5e-11 (98,304) and 2.3e-10 (1,572,864, still inside the margin); the snap moves the read by at most 1e-9 cells times the slope (1e-6 m at 500 m per cell). A sample of -0.0 reads +0.0 (equal, not bit-equal; the store's quantised change does not hold -0.0).
- libm only (`round`); no new operation depends on the thread count (CON-14).

## Measured (tier A, the noisy synthetic layer: swells of 40 m and cell noise of 25 m)
| Read | Step across a cell-centre line | Slope jump over 1e-4 cells inside a cell | Slope change over 0.01 cells (about 1 m) |
|---|---|---|---|
| C-R clamped to the four cells (before) | 1.93 m | - | - |
| monotone, harmonic-mean slope | about 2e-5 m | 15-20 m per cell (a kink) | 16.4 m per cell |
| monotone, this limiter | about 2e-5 m | 1.8 m per cell (curvature) | 12.6 m per cell |
| C-R without a clamp (overshoots) | 0 | - | 1.2 m per cell |

- A monotone read bends more sharply than C-R where the samples have extrema (its slope is 0 at an extremum: flat-topped cells on a noisy layer). That is the price of no overshoot; the layer's real change is far smoother than this test layer.

## Cost
- Two limiter evaluations per 1D pass (5 passes a read) replace the C-R polynomial and the clamp's four extra cell reads: about the same work.

## Rejected
- Per-axis clamp of each 1D C-R pass to its pair's range (the lead's fallback): continuous, but C0 where clipped.
- A softened limiter `2ab / (a² + b² + s²)`: smooth at the origin, but it flattens uniform slopes smaller than `s` (terraces on plains).
