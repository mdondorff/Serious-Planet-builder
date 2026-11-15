## MODIFIED Requirements
### Requirement: BUILD-055 The river gate prints the river numbers of a region
`cargo xtask river-gate <store dir | window>` SHALL print, in release and under the heavy lock, on one region at L: the share of river cells inside in closed basins (at least 0.5 m deep and 16 cells, `--min-cells`) beside SPIKE-114's 5 cm reference; the valley depth (surface 1 to 2 cells out above the river cell, the canyon a viewer sees) and the burn (surface above the river level, K7's cut), each a share over 50 m; ring 2-4 and the erosion depth; the share in lakes deeper than D (`--d`, default 20 m); p50 relief at 2 x 2 and 10 x 10 cells beside the start's; H1's edge crossings matched within 2 km. It SHALL exit 0.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #143, P3 E1 and E2), SPIKE-114, SPIKE-115, crusader 2026-11-14 (surfaces, canyon, N), TEST-136, lead 2026-11-14 (canonical window names), lead 2026-11-14 (DEC crusader-4: the canyon as valley depth and burn)
Detail: The valley depth is the p50 of the surface at Chebyshev distance 1 to 2 (102-204 m) above the surface at the river cell; its bar is relative to Earth: at most the share of a GLO-30 tile with 1 to 2 x the window's relief at 10 x 10 cells (printed beside it; the review table gives the reference). The burn is the surface at the river cell above the river level, every basin breached and with the lakes of D kept. Ring 2-4 is the p50 of the surface 2 to 4 cells out above the river level (the valley depth there plus the burn). A window is `face,i0,i1,j0,j1,seed` in regional cells or a canonical name: R1h face 1, 8056-8675 x 760-1379, seed 14 (the E1 region stores' window, the default); R3a face 4, 368-987 x 4720-5339, seed 15; R3b face 2, 9968-10587 x 8304-8923, seed 15. The basins are found under a fill of 1e-7 m a cell. A window measures its start surface (the base's C1 read B plus the regional field) over the region's box. A store measures its stored ground B + L (an L tile is K7's ground minus B) over the commit's tiles, with the start surface around them, its tiles draining to their border, at the commit's sea level; a commit of another grid, radius, seed, base or (with the keys) inputs identity is refused; `--no-key-check` reads each tile under the key its commit names and the report says "keys not checked". An H1 crossing counts with Q of at least 1 m3/s. An unknown option, a missing value, a second window and `--no-key-check` without a store are refused.

#### Scenario: A window
- **WHEN** the gate runs on `r1h`, `r3a` or `r3b`
- **THEN** it measures that canonical window and prints one line each for rivers, basins, valley depth, burn, ring 2-4, erosion depth, lakes, relief and edge, and exits 0

#### Scenario: A spike store
- **WHEN** the gate runs on a spike branch's store with `--no-key-check`
- **THEN** it names the commit and prints "keys not checked: spike store"
