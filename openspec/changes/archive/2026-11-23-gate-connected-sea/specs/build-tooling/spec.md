## MODIFIED Requirements
### Requirement: BUILD-055 The river gate prints the river numbers of a region
`cargo xtask river-gate <store dir | window>` SHALL print, in release and under the heavy lock, on one region at L: the share of river cells inside in closed basins (at least 0.5 m deep and 16 cells, `--min-cells`) beside SPIKE-114's 5 cm reference; the valley depth (surface 1 to 2 cells out above the river cell, the canyon a viewer sees) and the burn (surface above the river level, K7's cut), each a share over 50 m; ring 2-4 and the erosion depth; the share in lakes deeper than D (`--d`, default 20 m); p50 relief at 2 x 2 and 10 x 10 cells beside the start's; H1's edge crossings matched within 2 km. It SHALL exit 0.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #143, P3 E1 and E2), SPIKE-114, SPIKE-115, crusader 2026-11-14 (surfaces, canyon, N), TEST-136, lead 2026-11-14 (canonical window names), lead 2026-11-14 (DEC crusader-4: the canyon as valley depth and burn), spikes 2026-11-15 (SPIKE-119: windows M1 and F1), lead 2026-11-23 (#185: the gate's outlets are the connected sea)
Detail: The gate's outlets, of its floods and its routing: the window's edge cells and the ocean cells still at or under the sea level on the measured surface; the ocean is the cells at or under the sea level on the window's start surface joined through such cells (8-connected) to an edge cell or to an L cell of an H1 sea cell inside the region (the chain's connected sea mask, readings 12 and 39; on a raw grid, the grid itself and its edge), so a coast the variant raises does not cut the sea off, and a start-sea cell the variant raises above the sea is land; an inland cell under the sea level is no outlet: its pit fills, while the land-only measures (cells above the sea level) still leave it out. The valley depth is the p50 of the surface at Chebyshev distance 1 to 2 (102-204 m) above the surface at the river cell; its bar is relative to Earth: at most the share of a GLO-30 tile with 1 to 2 x the window's relief at 10 x 10 cells; the gate prints that relief beside the share, and the GLO-30 share comes from the review table, not from the gate. The burn is the surface at the river cell above the river level, every basin breached and with the lakes of D kept. Ring 2-4 is the p50 of the surface 2 to 4 cells out above the river level (the valley depth there plus the burn). A window is `face,i0,i1,j0,j1,seed` in regional cells or a canonical name: R1h face 1, 8056-8675 x 760-1379, seed 14 (the E1 region stores' window, the default); R3a face 4, 368-987 x 4720-5339, seed 15; R3b face 2, 9968-10587 x 8304-8923, seed 15; M1 (mountain belt) face 4, 6600-7219 x 10200-10819, seed 15; F1 (low-gradient river landscape) face 4, 4800-5419 x 3300-3919, seed 15. The basins are found under a fill of 1e-7 m a cell. A window measures its start surface (the base's C1 read B plus the regional field) over the region's box. A store measures its stored ground B + L (an L tile is K7's ground minus B) over the commit's tiles, with the start surface around them, its tiles draining to their border, at the commit's sea level; a commit of another grid, radius, seed, base or (with the keys) inputs identity is refused; `--no-key-check` reads each tile under the key its commit names and the report says "keys not checked". An H1 crossing counts with Q of at least 1 m3/s. An unknown option, a missing value, a second window and `--no-key-check` without a store are refused.

#### Scenario: An inland pit below the sea level
- **WHEN** a window holds a pit under the sea level that no chain of cells at or under the sea joins to its edge
- **THEN** the gate's flood fills it as a closed basin; a pit joined to the sea through cells at or under it is sea

#### Scenario: A window
- **WHEN** the gate runs on `r1h`, `r3a`, `r3b`, `m1` or `f1`
- **THEN** it measures that canonical window and prints one line each for rivers, basins, valley depth, burn, ring 2-4, erosion depth, lakes, relief and edge, and exits 0

#### Scenario: A spike store
- **WHEN** the gate runs on a spike branch's store with `--no-key-check`
- **THEN** it names the commit and prints "keys not checked: spike store"

### Requirement: BUILD-059 The river gate's channel network at a threshold
`cargo xtask river-gate` (BUILD-055) SHALL print and tabulate, from the same routing as BUILD-057, the channel network at each of two channel thresholds (`--network-km2 A,B`, default 1 and 75 km2): the drainage density and the Strahler stream counts with the bifurcation ratio `R_b`; and it SHALL take a raw height grid as input (`grid=file,w,h,sea_level_m`), so that the GLO-30 benchmark tiles are measured by the same code as the windows.

Verify: A
Status: active
Source: Lead 2026-11-16 (explorer #163 item 2; river-graph plan, pre-work: the density bar fixed before any RG result), BUILD-055, BUILD-057, TEST-137
Detail:
- channel cell: a land cell (above the sea level) whose drainage area (the gate's routing, BUILD-055) is at least the threshold;
- density: the length of the steps from a channel cell inside to a channel receiver (diagonal steps sqrt 2 cells) over the land area inside, km/km2; a step into the sea is not counted;
- Strahler order on the whole grid from the channel donors: a cell without a channel donor is order 1; otherwise the donors' highest order `m`, raised to `m + 1` where two or more donors carry `m`; the result depends on the receivers only;
- a stream of order `k` starts at a cell of order `k` that no donor of order `k` enters (a head, or a meeting of streams of order `k - 1`); `N_k` counts the streams starting inside;
- `R_b = 10^-b`, `b` the least-squares slope of `log10 N_k` on `k` over the orders 1 to the highest less one with `N_k > 0` (the highest order is cut by the window); none (0) with fewer than 2 such orders;
- the raw grid: little-endian f64, row-major, cells of 101.8 m (SPIKE-119's preparation); the byte count must be `8 w h`; outlets are BUILD-055's: the grid's edge and the cells at or below the sea level joined to it; counted inside a 10-cell border; routing seed 7; no H1 (the input line says so, and the edge, trunk and corridor lines report none);
- fields per threshold `net1_*` and `net2_*`: the threshold, the channel length, the land area, the density, the stream counts by order (`N_1/N_2/...`), the orders fitted and `R_b`;
- the bar rule (#163): for each window, the range over the GLO-30 tiles whose 1 km relief (10 x 10 cells, p50) lies within 1 to 2 times the window's start relief; the numbers are in the river-graph pre-work results, not in this requirement.

#### Scenario: A hand-built network
- **WHEN** four heads pair into two order-2 streams meeting in an order-3 trunk, and a fifth head joins the trunk
- **THEN** the counts are 5 / 2 / 1, `R_b` is 2.5 over orders 1 and 2, and the density is the step length over the land area (TEST-137)
