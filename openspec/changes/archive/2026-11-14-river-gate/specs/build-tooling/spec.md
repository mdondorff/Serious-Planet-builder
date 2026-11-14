## ADDED Requirements
### Requirement: BUILD-055 The river gate prints the river numbers of a region
`cargo xtask river-gate <store dir | face,i0,i1,j0,j1,seed | r3a>` SHALL print, in release and under the heavy lock, on one region at L: the share of river cells inside in closed basins (at least 0.5 m deep and 16 cells, `--min-cells`) beside SPIKE-114's 5 cm reference; the share over 50 m below the p50 of their valley 2 to 4 cells out (the canyon), with the lakes of D kept and every basin breached; the erosion depth under the start surface; the share in lakes deeper than D (`--d`, default 20 m); the p50 relief over 2 x 2 and 10 x 10 cells beside the start's; and H1's edge crossings matched within 2 km. It SHALL exit 0.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #143, P3 E1 and E2), SPIKE-114, SPIKE-115, crusader 2026-11-14 (surfaces, canyon, N), TEST-136
Detail: The basins are found under a fill of 1e-7 m a cell. A window measures its start surface (the base's C1 read B plus the regional field) over the region's box. A store measures its stored ground B + L (an L tile is K7's ground minus B) over the commit's tiles, with the start surface around them, its tiles draining to their border, at the commit's sea level; a commit of another grid, radius, seed, base or (with the keys) inputs identity is refused; `--no-key-check` reads each tile under the key its commit names and the report says "keys not checked". An H1 crossing counts with Q of at least 1 m3/s. An unknown option, a missing value, a second window and `--no-key-check` without a store are refused.

#### Scenario: A window
- **WHEN** the gate runs on `r3a`
- **THEN** it prints one line each for rivers, basins, canyon, erosion depth, lakes, relief and edge, and exits 0

#### Scenario: A spike store
- **WHEN** the gate runs on a spike branch's store with `--no-key-check`
- **THEN** it names the commit and prints "keys not checked: spike store"
