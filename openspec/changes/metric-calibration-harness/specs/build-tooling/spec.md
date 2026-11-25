## ADDED Requirements
### Requirement: BUILD-069 A geometric gate column is registered only with a passing calibration
A geometric gate column SHALL be registered (added as a bar or a target) only with a calibration entry in `planet_cache::metric_calibration` that passes TEST-155. Each metric's band SHALL be stated in the Detail of its own requirement. A column registered before this requirement SHALL carry a calibration report: a failing report is an owner item, not a removal.

Verify: A
Status: active
Source: Lead 2026-11-25 (the lead's rule after the owner's caution and the external process review), TEST-155, CON-31
Detail:
- `metric_calibration::registered()` lists the entries registered under this rule; the tier-A test fails if one of them fails its calibration; an entry without a band (report only) cannot be registered;
- `metric_calibration::reported()` lists the entries that predate it; their calibration runs and writes its table, and its verdict is printed, not asserted;
- the first reported entries and their bands (a band holds a defect-free valley's reading):

| entry | reading | band |
|---|---|---|
| `corner-links-added` | the counted 2 x 2 blocks that are corner links (`river_suite::corner_link`, the predicate `corner_links` counts) in the surface and not in the reference | 0 to 0 |
| `hump-cuts` | `river_suite::hump_cuts` | none (report only: the count depends on the background's humps) |
| `spectrum-slope-change` | `river_suite::spectrum`'s beta, the surface's minus the reference's | none (report only: a valley on a flat background has its own spectrum) |
| `xdiag-in-zone` | SPIKE-143's M1 X_diag over the zone's counted cells (the grid's border cells left out): d1 = (z(x+1, y+1) - 2 z + z(x-1, y-1)) / 2 binned by (x - y) mod 8, d2 = (z(x-1, y+1) - 2 z + z(x+1, y-1)) / 2 binned by (x + y) mod 8; per diagonal the largest share of its squared energy in two adjacent bins (cyclic); X_diag = their mean / 0.25 - 1 | -1 to 0.010 (the chainsaw target) |
| `xdiag-in-zone-weighted` | the same per-diagonal shares, weighted by each diagonal's energy: `(e1 l1 + e2 l2) / (e1 + e2) / 0.25 - 1` | -1 to 0.010 |
| `m4-along-added` | M4 along the channel path (zone cells within 4 cells of its segments; k = z(p + t) - 2 z(p) + z(p - t), bilinear, t the nearest segment's unit direction; RMS), the surface's minus the reference's (m) | -0.5 to 0.5 |
| `d8-sinuosity` | the channel path's length over its chord, at least 1 (a rounding below 1 reads 1) | 1.00 to 1.02 |
| `straight-run-share` | the share of the channel path's length in runs of one repeated step longer than 2 km | 0 to 0.5 |

- a reported entry that fails is not changed or removed by the calibration (CON-31); its report goes to the owner.

#### Scenario: A new metric without a passing calibration
- **WHEN** an entry is added to `registered()` and its calibration fails
- **THEN** the tier-A test fails, naming the metric and the first case that breaks TEST-155's rule

#### Scenario: A column that predates the rule
- **WHEN** a `reported()` entry's calibration fails
- **THEN** the test passes, the table and verdict are written for the owner, and the gate's column is unchanged
