## MODIFIED Requirements
### Requirement: BUILD-055 The river gate prints the river numbers of a region
`cargo xtask river-gate <store dir | window>` SHALL print, in release and under the heavy lock, on one region at L: the share of river cells inside in closed basins (at least 0.5 m deep and 16 cells, `--min-cells`) beside SPIKE-114's 5 cm reference; the valley depth (surface 1 to 2 cells out above the river cell, the canyon a viewer sees) and the burn (surface above the river level, K7's cut), each a share over 50 m; ring 2-4 and the erosion depth; the share in lakes deeper than D (`--d`, default 20 m); p50 relief at 2 x 2 and 10 x 10 cells beside the start's; H1's edge crossings matched within 2 km. It SHALL exit 0.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #143, P3 E1 and E2), SPIKE-114, SPIKE-115, crusader 2026-11-14 (surfaces, canyon, N), TEST-136, lead 2026-11-14 (canonical window names), lead 2026-11-14 (DEC crusader-4: the canyon as valley depth and burn), spikes 2026-11-15 (SPIKE-119: windows M1 and F1)
Detail: The valley depth is the p50 of the surface at Chebyshev distance 1 to 2 (102-204 m) above the surface at the river cell; its bar is relative to Earth: at most the share of a GLO-30 tile with 1 to 2 x the window's relief at 10 x 10 cells; the gate prints that relief beside the share, and the GLO-30 share comes from the review table, not from the gate. The burn is the surface at the river cell above the river level, every basin breached and with the lakes of D kept. Ring 2-4 is the p50 of the surface 2 to 4 cells out above the river level (the valley depth there plus the burn). A window is `face,i0,i1,j0,j1,seed` in regional cells or a canonical name: R1h face 1, 8056-8675 x 760-1379, seed 14 (the E1 region stores' window, the default); R3a face 4, 368-987 x 4720-5339, seed 15; R3b face 2, 9968-10587 x 8304-8923, seed 15; M1 (mountain belt) face 4, 6600-7219 x 10200-10819, seed 15; F1 (low-gradient river landscape) face 4, 4800-5419 x 3300-3919, seed 15. The basins are found under a fill of 1e-7 m a cell. A window measures its start surface (the base's C1 read B plus the regional field) over the region's box. A store measures its stored ground B + L (an L tile is K7's ground minus B) over the commit's tiles, with the start surface around them, its tiles draining to their border, at the commit's sea level; a commit of another grid, radius, seed, base or (with the keys) inputs identity is refused; `--no-key-check` reads each tile under the key its commit names and the report says "keys not checked". An H1 crossing counts with Q of at least 1 m3/s. An unknown option, a missing value, a second window and `--no-key-check` without a store are refused.

#### Scenario: A window
- **WHEN** the gate runs on `r1h`, `r3a`, `r3b`, `m1` or `f1`
- **THEN** it measures that canonical window and prints one line each for rivers, basins, valley depth, burn, ring 2-4, erosion depth, lakes, relief and edge, and exits 0

#### Scenario: A spike store
- **WHEN** the gate runs on a spike branch's store with `--no-key-check`
- **THEN** it names the commit and prints "keys not checked: spike store"

## ADDED Requirements
### Requirement: BUILD-057 The river gate's measurement suite
`cargo xtask river-gate` (BUILD-055) SHALL print, after the gate lines and from the same input (a window's start surface or a store's baked ground), one line per metric, each beside the absolute count it is taken over: the gradient, the valley width, the cross-section asymmetry, the drainage density, the catchment ratio at H1 crossings, the seam at the region's edge, the narrowness ratio, the slope-area scaling and the relief power spectrum. The lines SHALL be the same bits for the same input.

Verify: A
Status: active
Source: Lead 2026-11-15 (explorer #151, P3 E1; the owner's rewrite of P3 after the external review), BUILD-055, TEST-137
Detail: One definition per metric; the bars come from the spikes' Earth benchmark (windows, transects and the GLO-30 reference, #121), not from this requirement.
- conventions: the quantile f of n sorted values is the value at index floor(n f) (the upper median for p50); 0 for none; a log is log10; a least-squares fit with fewer than two points is 0;
- gradient: a river step is a river cell inside to its receiver when that is a river cell; it is uphill when the receiver's surface stands more than 0.25 m above it; printed: uphill steps, steps, the largest rise;
- valley width: across the flow (perpendicular to the step to the receiver) the surface is walked in cell steps, bilinear, up to 64 cells to each side, to where it first stands more than the depth above the river cell's surface, the crossing interpolated; the width is both sides in metres; a side that reaches 64 cells or the grid's edge is capped and counted; the depth is 10 m, and half the cell's ring 2-4 depth, where a ring 2-4 depth under 2 m counts the cell as flat; printed: p50, p90, cells, capped sides, flat cells;
- asymmetry: |L - R| / (L + R), where L and R are the mean of the surface at the points 2, 3 and 4 cells out on each side (bilinear) minus the river cell's surface, a negative mean counting as 0; cells with L + R under 1 m or a point outside the grid are left out; printed: p50, p90, cells;
- drainage density: the river steps' length inside (a diagonal step is the cell width times the square root of 2) over the land area inside (cells above the sea level), km/km2;
- catchment ratio: each H1 crossing of Q at least 1 m3/s gives L's drainage area at the nearest L crossing of its direction within 2 km over its Q; the median of these is the implied runoff (m2 per m3/s); printed: the pairs, the implied runoff, p10 and p90 of each value over the median (1 when L carries H1's catchment at every crossing); no pair without an H1 crossing;
- seam: |dz| of the surface between 4-neighbours with one cell inside the region's box and one outside (count, p50, p99, maximum), against p50 and p99 between 4-neighbours both inside;
- narrowness: the valley depth (BUILD-055) over the ring 2-4 depth above the river cell, over river cells whose ring 2-4 depth is at least 1 m (p50, p90, cells); and the p50 valley depth over the p50 ring 2-4 depth, both over every river cell inside;
- slope-area: land cells inside with a receiver, a positive slope (the surface drop over the step length) and an area of at least 4 cells; bin b holds areas from 4 x 10^(b/2) cells; per bin the p50 slope and the cells; theta is minus the fitted slope of log p50 slope against log (the bin's lower area x 10^0.25), over the bins of at least 30 cells;
- spectrum: the largest power-of-two square of at least 8 and at most 1024 cells in the bounding box of the cells inside, centred in it (none below 8); the mean removed; a Hann window w(i) = 0.5 - 0.5 cos(2 pi (i + 0.5) / n) in each axis; the 2-D FFT; the power |F|^2 over the sum of the window's squares, averaged per octave o of the radial wavenumber k (cycles per square, floor(log2 k) = o, 1 <= k < n); beta is the fitted slope of log power against log (1.5 x 2^o) over the octaves from o = 1; printed for the surface and for the start.


#### Scenario: A store
- **WHEN** the gate runs on a store
- **THEN** the nine suite lines are measured on the stored ground B + L over the commit's tiles, with the same definitions as on a window

#### Scenario: Metrics of a valley
- **WHEN** the surface is a V valley with a river on its floor
- **THEN** the width at 10 m, the asymmetry, the narrowness and the density have the values of its geometry (TEST-137)
