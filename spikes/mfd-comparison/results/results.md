# SPIKE-107: MFD comparison (unclipped, clipped, boundary flux)

Opened: 2026-11-11
Question: does the catchment-clipped multiple-flow-direction (MFD) area of GEN-141 (`MfdClip::Local`) change the terrain's geomorphology against unclipped region-wide MFD (`MfdClip::Off`) and against a boundary-flux exchange between catchments?
Why it matters: A3 §12.4 (MFD, P0) and ADR 0112 condition 1: the clip stays provisional until compared; G-LOCAL shows computational consistency, not plausibility. A3 §13 open item 1 ("whether clipped MFD changes the look").
Time box: plan today; the measurements about 1 h of machine time (single runs, one variant at a time); ask the bulldozer before it exceeds 1 h without a result.
Success criteria: per fixture and per variant, the measures of §3 reported against the unclipped reference with numbers and renders; a plain statement of which measures differ materially (the architect judges).
Fallback if it fails: if the boundary-flux variant cannot be built in the probe within the time box, report the two existing variants and the unbuilt variant's definition.
Code location: probe on a spike branch (`spikes/mfd-comparison/`, never merged); no change to production code.

Status: plan (2026-11-11, no run yet). Machine: heavy runs only on the bulldozer's go.

## 1. The variants

| Variant | What it is | Where it exists |
|---|---|---|
| U: unclipped | `MfdClip::Off`: a cell shares its area among all its lower neighbours, also in another D8 catchment; the area is a region-wide quantity | `crates/generators/src/bake/route.rs` (`mfd_area`); whole-region `carve` and `solve_rounds` accept it |
| C: clipped | `MfdClip::Local` (GEN-141): lower neighbours in another catchment are dropped from the weights and their sum; it also changes the breach (`exit_base` in `round.rs`, `carve.rs`) | same |
| F: boundary flux | the area is clipped inside a catchment as in C; the share a cell would give to a lower neighbour in another catchment is not dropped but passed across the border and added to that cell's `A_d` | **not in the code**: built in the probe only (no production change) |
| ToReceiver | the foreign share goes to the cell's own receiver (`MfdClip::ToReceiver`) | exists; reported as a reference (A3 §6.3: not closed) |

Boundary flux, definition for the probe: in round `r` the region routing under U gives each cell's held area on the surface at the round's start. For a border pair (p in catchment A, q a lower neighbour in catchment B) the flux is `held(p) * w_pq / sum(p)` as U computes it. F adds that flux to the area input at q of q's catchment. C's own clipped weights stay unchanged. The exchange is evaluated once per round from the surface at the round's start (a lagged exchange). Two exchange counts are measured: one pass and two passes (the second pass uses the first pass's areas). The gap between F's area and U's area is reported; it is the cost of the lag.

## 2. Fixtures

| Fixture | What | Why |
|---|---|---|
| R1 | prototype region 1 (face 1, i 2014-2168, j 190-344 of the 3,072 grid, seed 14), margin 200 cells, the window of `rounds_region.rs`; 4 rounds of 5 steps, `cell_metric` | mountains, plains, rivers, a closed basin |
| R3a, R3b | the second region (hills and plains) and the second seed of SPIKE-106 item 13 (`spikes/prototype-gates/results/`) | a second relief class and seed |
| S-smooth, S-terraced | the two synthetic lands of GEN-141 (`round_probe.rs`, four 96 x 96 each) | flats: where the flood and the clip interact |
| Divides | the cells within 8 cells of a catchment border (D8 catchments of round 1), taken from R1 | the part the question is about |
| Low-relief, coastal mountains | one window each from `spikes/prototype-region/results/region-search-output.txt` if it lists them; else dropped and reported | A3 §12.4 plausibility fixtures |

## 3. Measures (per fixture and variant, against U unless stated)

| Question (ADR 0112 condition 1) | Measure |
|---|---|
| Effective area near divides | `A_d` of C and F against U on the cells within 0, 1, 2, 4 and 8 cells of a catchment border: share of cells with ratio outside 0.9-1.1, p50 / p95 / max ratio; the same away from borders (control) |
| Flow divergence and convergence | share of land cells that give to 1, 2, 3 or more lower neighbours (U, C, F); share of border cells whose U area is more than half foreign |
| Ridge and valley statistics | after the rounds: plan and profile curvature histograms, the ridge-cell and valley-cell shares, slope p5 / p50 / p95, hypsometry |
| Channel initiation | channel cells = `A_d` at least H1's initiation threshold `T` (read from the code): count and the membership difference as a share of channel cells; heads moved by more than 2 cells |
| Outlet consistency | at every outlet of the round's catchments: `A_d` against the sum of `a w` over the members (A3 §5.2); the gap for U and F |
| Erosion displacement | `z_V - z_U` after the 4 rounds on the land of R1's region box: p5 / p50 / p95 / max, and its sign by catchment; relief (p95 - p5 of height in 5 km blocks) ratio; every row against the noise floor of the carve (the same variant under two jitter seeds; the carve is chaotic, ADR 0112 context) |
| Visual fixtures | PNGs of `z_V - z_U` (diverging colour), of the area ratio near borders, and hillshades of U, C and F side by side for R1's region box and a crop of a divide; each under 1 MB, in `images/` |

A displacement is a finding only where it exceeds the noise floor, so each displacement row carries the floor measured the same way.

## 4. Method and cost

- Whole-region reference per variant: `carve` with the rules (U: `Off` with the canonical flood on; C: `RoundRules::ROUND`; F: the probe). The round solver (`solve_rounds`) is run for U and C as well, since the shipped path is the solver; under U the solver takes the region areas as key inputs.
- R1's window with margin 200 is about 555 x 555 cells; `rounds_region` solves it in seconds to about a minute. Estimate for R1 (three variants, two seeds for the floor): 10-15 min; R3a and R3b about the same each; the synthetic lands seconds. One variant at a time, BelowNormal, 8 threads, no run before the bulldozer's go. A K7 level (local 102 m) pass is out of scope; if a measure needs it, ask first.
- Probe: `crates/generators/examples/mfd_compare.rs` on the spike branch `spike/spike-107-mfd`, written from `rounds_region.rs` and `round_probe.rs`; it reads library APIs only and builds F from `route_rules` outputs. No library change.
- Sanity check of the probe: C against the region routing reproduces the GEN-141 figures of A3 §6.4 (member areas that differ on smooth land).

## 5. Open points for the architect

- Whether F as defined (one lagged exchange per round) is the boundary-flux form ADR 0112 means, or an iterated exchange within a round; the probe measures one and two passes and reports the gap.
- The bar for "unclipped is acceptable" (channel-cell membership change at most 1 %, A3 §12.1 G-EQ, or a visual criterion): the spike reports numbers; the verdict is the architect's.
