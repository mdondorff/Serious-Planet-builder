## ADDED Requirements
### Requirement: TEST-151 TEST-139's partition layouts as data
TEST-139's fixture SHALL carry its partition layouts as a data file (`crates/generators/tests/support/g0_layouts.txt`): each a name, a region count, a tile size, a halo and the region of every tile of the six faces, regions of whole tiles of GEN-182's grid on the fixture's L lattice; every layout SHALL tile the cube with no gap and no overlap, each tile a `CubePatch` (COORD-012).

Verify: A
Status: active
Source: Lead 2026-11-24 (#195, owner O2; the G3 port order's C1 and C2, reconciliation rows 1 and 2; DEC on the halo: 24, lead 2026-11-24), TEST-139, GEN-182, COORD-012
Detail:
| Layout | Regions | Seam |
|---|---|---|
| `one-region` | 1 | none |
| `two-regions` | 2 | face 0's tile line at longitude -15 (TEST-139's layout) |
| `four-regions` | 4 | `two-regions` split by the sign of each tile centre's latitude |
| `shifted-seam` | 2 | `two-regions` with its cut one tile column east, face 0's tile line at +15 |
| `face-edge-seam` | 2 | face 0 against the other faces: face 0's four cube edges |
- tiles of 64 L cells (3 x 3 a face), halo 24: GEN-182's bounds, `DISC_RADIUS_MAX + PROFILE_CARRY_MIN` (16 + 8) at least, `n / 8` (192 / 8) at most, tile more than twice the halo.

#### Scenario: Whole tiles, no gap, the declared seams
- **WHEN** the layouts are read and each tile is built as a `CubePatch` with the layout's halo
- **THEN** every L cell lies in exactly one tile's core, each region holds a tile, `two-regions` equals TEST-139's layout, face 0's seam is its tile line 64 in `two-regions` and 128 in `shifted-seam`, every pair across `face-edge-seam` lies on two faces (4 n pairs), and `four-regions` cuts a face in both directions
