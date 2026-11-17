## ADDED Requirements
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
- the raw grid: little-endian f64, row-major, cells of 101.8 m (SPIKE-119's preparation); the byte count must be `8 w h`; outlets are the grid's edge and every cell at or below the sea level; counted inside a 10-cell border; routing seed 7; no H1 (the input line says so, and the edge, trunk and corridor lines report none);
- fields per threshold `net1_*` and `net2_*`: the threshold, the channel length, the land area, the density, the stream counts by order (`N_1/N_2/...`), the orders fitted and `R_b`;
- the bar rule (#163): for each window, the range over the GLO-30 tiles whose 1 km relief (10 x 10 cells, p50) lies within 1 to 2 times the window's start relief; the numbers are in the river-graph pre-work results, not in this requirement.

#### Scenario: A hand-built network
- **WHEN** four heads pair into two order-2 streams meeting in an order-3 trunk, and a fifth head joins the trunk
- **THEN** the counts are 5 / 2 / 1, `R_b` is 2.5 over orders 1 and 2, and the density is the step length over the land area (TEST-137)
