## MODIFIED Requirements
### Requirement: BUILD-060 The river gate's G2 columns for river graphs
`cargo xtask river-gate` (BUILD-055) SHALL read RG and B through one build: given a canonical river graph (GEN-189), the per-stage budget fields or a partition pair, it SHALL print the G2 columns beside the raster numbers, each with its gate class (a `RIVER-GATE classes:` line, and a `class` row in the table); a row without them (B) prints none of them.

Verify: A
Status: active
Source: Lead 2026-11-17 (explorer #165; explorer #166a: the surface input; explorer #167a, owner 2026-11-17: the chain sheet as a kept tool; explorer #168, owner decisions 1 and 3 2026-11-17: the bars and the dual reference, reading 21; explorer #169 and #170, 2026-11-18: the chain per row, reading 28; explorer #171, owner 2026-11-18: the run's own start; explorer #172, 2026-11-18: step C's first measures; explorer #173, 2026-11-19: the base noise, reading 30; lead #166, owner 2026-11-18: kept-basin interiors and cells above a graph head counted apart; the river-graph plan's G2, its addendum's section 4 graph reports, the G0 contracts' sections 5, 6, 9 and reading 17), BUILD-046, BUILD-055, BUILD-057, BUILD-059, GEN-189, TEST-137
Detail:
- inputs (read by `river_gate_graph::read_graph`, `read_stages` and `read_surface`): `--surface FILE` (a candidate's surface on the window's L grid, little-endian f64, row-major, every height finite: the window's raster columns measure it against the window's start surface; not with `--store` or `--grid`), `--noise axis | turned` (the base's band and crease noise, GEN-140: `turned` with the per-octave lattice turn, the default (main's bakes since GEN-140's default, owner 2026-11-19), `axis` without it; the window's start surface and a store's base follow it, so a run on the turned base is measured on a turned start), `--start FILE` (the run's own start surface on the window's L grid, f64 as `--surface`: a chain that conditions its base has another `S0` than main's; every raster delta, the budget field and the corridor are measured against it, and the corridor's trunks are the H1 reaches of the row's `--graph` at their H1 cell centres, cut at the window's edge; without `--graph` no trunk is measured; the H1 edge-agreement column still reads main's H1; not with `--grid` or `--store`; default: the window's start surface and main's H1 trunks), `--graph FILE` (canonical form), `--stages DIR` (`<stage>-lower.u16` and `<stage>-fill.u16` for kh, ks, kv and kc: little-endian u16 of 0.1 m on the window's L grid, row-major; a missing file is a stage that did not run or does not fill; `residuals.csv` a header and one line per residual record), `--partition ONE,TWO` (the canonical graphs of one and of two regions); `--corridor-bar PCT` (the corridor bar; default 97.5, 95 on M1); in `--table` a row takes them as suffixes `#graph=`, `#stages=`, `#partition=`, `#surface=`, `#start=`, `#noise=`, `#timing=FILE` (a run's timing file, read by the table only: a header, then `stage,seconds,peak_mib` per line, the numbers read from the line's right end, so a stage name may hold commas; the row gets `cost_s`, the seconds summed, and `cost_peak_mib`, the largest peak, `-` when none is given), and `#chain=FILE` (a run's chain file, read by the table only: a start-surface row takes the chain's label, `<label> @ <window>`, and the markdown table ends in a `Chains` section listing each such row's `commit`, `switch` and `stage` lines);
- ids: an L cell of the window `(x, y)` is the global cell `(x0 + x, y0 + y)`, id `(gy << 32) | gx`; 8-neighbours by those coordinates;
- classes:

| Part | Columns | Class |
|---|---|---|
| Graph validity | nodes and reaches per level; GEN-189's validators on an open slice, counted by kind, without the tile-core rule (the gate has no tile map); endpoints owned elsewhere | correctness |
| Trunk continuity, outlet connectivity | sources whose chain (followed through a marker along its reach) ends at an outlet, ends open, or neither; trunk reaches (H1, or L with a parent) whose held `down` node is neither an outlet nor a marker nor left by a reach | correctness |
| Junction conservation | the validators' conservation breaks | correctness |
| Bed profile | the validators' rises outside exceptions; reaches whose bed rises above its running minimum anywhere, and the largest such rise (m) | correctness |
| Exceptions | reaches per reason (`rise_over_budget`, `unresolved_endpoint`, `budget_residual`) | correctness |
| Tributary attachment | L reaches without a parent, and those ending at a held `trib_junction`, confluence, outlet or tile crossing | correctness |
| Hierarchy | on GEN-189's rasterisation over land inside: the length from each owned cell to its reach's next vertex (diagonals sqrt 2 cells), the density over the land area, Strahler streams by order (a stream of order k starts at an owned cell of order k that no donor of order k enters: the previous vertex, or at a reach's first vertex the reaches ending at its `up` node), `R_b` by BUILD-059's fit; read against BUILD-059's bar | geomorphology |
| Basins | kept and endorheic counts; a parent or a spill node the graph does not hold (a basin whose floor cell lies outside the counted cells apart, `g_basin_spill_missing_edge`: its spill path may leave the window); a same-level parent whose spill lies below the child's or whose floor lies above it | correctness |
| Graph/raster agreement | at each BUILD-059 threshold: routed river cells (land inside, drained area at least the threshold) with a rasterised graph cell within 1 cell (Chebyshev), and rasterised cells inside with a routed river cell within 1 cell; both shares; counted apart and outside both shares: routed and rasterised cells in a kept-basin interior (`agree<i>_routed_kept`, `agree<i>_graph_kept`), and routed cells above a graph head (`agree<i>_routed_above_head`: a cell with no graph cell within 1 cell whose downstream path on the gate's routing first comes within 1 cell of a graph cell at a cell within 1 cell of an L `source` node's anchor); each threshold's line names the three counts | correctness |
| Kept basins | a kept-basin interior: a depression of the measured surface (BUILD-057's basin flood: the 8-connected cells it raises by more than 1 mm) that holds the floor cell of a basin `protected_surface`; `kept_basins` the kept basins whose floor lies in a depression; `kept_cells` (inside), `kept_steps` (BUILD-057's river steps with either end in an interior) and `kept_uphill_steps` (those rising more than 0.25 m); a `RIVER-GATE kept basins` line with the four counts | correctness |
| Budget use | per stage over land inside (kh: `S0` against KH; ks, kv, kc: against `S_KH` when the fields are, reading 21): cells lowered, their p95 and largest lowering, cells filled and the largest fill, fills over the budget; cells whose summed lowering of ks, kv and kc exceeds the budget and cells at 95 % of it or more (`b_overrun`, `b_at_95`), and both with kh's lowering added, against `S0` (`b_overrun_s0`, `b_at_95_s0`); the fields floor each stage to 0.1 m: a count is a true overrun, an overrun under the step is not seen; all diagnostic (the G0 contracts' section 6: the fields are never the gate; GEN-189's f64 budget validator is). Residual records and reaches flagged `budget_residual` | preservation; residual counts correctness |
| Partition | the changes between the two canonical graphs (GEN-189's diff), the topology changes among them, and the first change (`identical` for none) | partition |
| G2 bars (owner decision 1, lead #168) | on a row with a graph, each `PASS:<value>` or `FAIL:<value>`: `bar_corridor` the snapped k_c 2 trunk reaches passing (BUILD-057) >= the corridor bar; `bar_uphill` river steps rising over 0.25 m (BUILD-057) <= 1.0 %, over the steps outside kept-basin interiors (uphill steps less `kept_uphill_steps`, over steps less `kept_steps`); `bar_agree_<A>km2` the routed share of the graph/raster agreement >= 90 % at each threshold (without the cells counted apart); `bar_violations` = 0; `bar_spill_missing` = 0 (basins whose floor cell lies inside); a `RIVER-GATE bars` line with the count passing | corridor preservation; the rest correctness |
| Step C, first measures (lead #172) | per stage, cells lowered by more than 50 m (`b_<stage>_over50`); KC's patches, count and volume (`b_kc_patches`, `b_kc_patch_km3`) from `kc-patches.csv` in the stage directory (a header `cells,volume_m3`, one line per patch); `relief2_lost_m` and `relief10_lost_m` (the start's relief minus the surface's, positive is lost); `beta_change` (the spectrum slope minus the start's); the bars `bar_spectrum_steep` (beta >= -4.13) and `bar_spectrum_rough` (beta <= -2.05), the range of SPIKE-119's 25 GLO-30 and 3DEP tiles; in `--table`, each row with a graph against the store row of the same window (B; a row's window is its label's part after ` @ `; a start-surface row gets none): `bar_relief_lost` (`relief10_lost_m` at most `max(B's, 0)`: a gain of relief is no loss), `bar_beta_change` (`abs(beta_change)` at most B's) and `bar_cost` (`cost_s` at most B's, when both rows carry a timing) | over-50 and patches preservation; spectrum bars geomorphology; `bar_relief_lost` preservation, `bar_beta_change` geomorphology, `bar_cost` product |
| Step C, ridges (lead #172) | on every row, one ridge mask on the start: a cell inside, on land, at least 2 m above both cells 2 L cells away along a face axis or a diagonal (the G0 contracts' section 9); against the reference `S_KH` (the start less the stage directory's `kh-lower.u16`, plus `kh-fill.u16`, in 0.1 m steps; reading 21) and against `S0` (the start; `ridge_ref` `s_kh`, or `s0` without a KH field, when both are the start): ridge cells lowered more than 20 m, the p95 and the largest lowering over the ridge cells (a raise counts as 0); per channel threshold 1 and 5 km2, cut-through cells (a lowered ridge cell with a cell of the surface's routing draining at least the threshold in its 3 x 3 and none of the reference's routing there, each reference routed as the surface is) and saddles (their 8-connected groups); columns `ridge_cells`, `ridge_lowered`, `ridge_lowered_pct`, `ridge_p95_m`, `ridge_max_m`, `cut1_cells`, `saddles1`, `cut5_cells`, `saddles5` against the reference, `ridge_s0_lowered_pct`, `ridge_s0_p95_m`, `saddles1_s0`, `saddles5_s0` against `S0`; a `RIVER-GATE ridges` line; on a row with a graph the bar `bar_crest` (`ridge_p95_m` at most 15 m, 35 m on M1); in `--table`, each row with a graph against B's row of the same window: `bar_saddles1` and `bar_saddles5` (`saddles1`, `saddles5` at most B's) | preservation |

- budget field: GEN-189's, from the gate's start surface (reading 28: the relief that sizes the budget is `S0`'s; the stage fields carry the lowering against `S_KH`), on the window's H1 blocks (`--stages` refuses an origin off an H1 corner); the centre L cell of a block is its index `block / 2`; the p50 follows BUILD-057's quantile convention; the 25 km neighbourhood is cut at the window's edge (a gate approximation of the landmass field);
- chain sheet: `cargo xtask stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]` renders a chain directory as one PNG (`planet_cache::stage_sheet`, the `stage_sheet` example; no spike code);
  - default: the whole window at 12 L cells per pixel, written to `target/review/stages/chain/<label>.png` (every character of the label but a letter, a digit or `-` written as `-`; `chain.png` without a label);
  - `chain.txt`: `stage-chain 1`, then `window X0 Y0 W H` (global L cells), `coarse C` (L cells per H1 cell), `cell_m M`, and optionally `sea_m S` (default 0), `label TEXT` and the run's chain: `commit C`, `switch NAME VALUE` and `stage STEP NUMBER NAME FORM` lines, kept in order (the commit printed in the title band);
  - L grid, little-endian f32, row-major: `base.f32`, `s0.f32`, `kh.f32`, `ks.f32`, `kv.f32`, `final.f32`;
  - H1 grid: `zs.f32` (f32) and `h1-rivers.u8` (1 on a river cell);
  - `graph.txt` (GEN-189 canonical form) and `stages/` (the stage fields above);
  - every file but `chain.txt` is optional: its panel is grey and its label ends `not dumped`; a file of the wrong size is refused, naming the file and both sizes;
- the sheet: a title band (label, sun, km per pixel, a 50 km bar), then the panels in rows of four, each under its label (cut at the panel's width):

| Panel | Content |
|---|---|
| 1 | the base (the H1 base read at L) |
| 2 | `z_s`, held per H1 cell |
| 3 | H1 rivers on `z_s` |
| 4 | `S0` |
| 5 | KH |
| 6 | KS, its change against KH tinted (lowering red, raise blue) |
| 7 | the trunk graph (H1 reaches and L reaches with a parent) on KS |
| 8 | tributaries whose drained area is at least 25 pixels, and basin floors (kept magenta), on KS |
| 9 | KV, its stage fields tinted (lowering red, fill blue) |
| 10 | KC (the final surface), its stage fields tinted the same way |
| 11 | the final surface with the trunks and the drawn tributaries |
| 12 | final minus `S0` (lowering red, raise blue; the sea in its sea colour) |

- one fixed scale for every panel and run:

| Quantity | Value |
|---|---|
| sun | azimuth 315, altitude 10 degrees |
| grey | 0.5 + 2 (shade - sin 10 degrees) per L cell, averaged over the pixel |
| stage tints (panels 6, 9, 10) | full at 20 m |
| change tint (panel 12) | full at 50 m |

- not here: incremental correctness (fresh against incremental) and the owner's flight; the bar for each geomorphology column is BUILD-059's or the plan's.

#### Scenario: A hand graph
- **WHEN** two sources meet at a confluence that runs to the sea and a tributary ends at a junction marker on it
- **THEN** every source reaches the outlet, Strahler counts 3 / 1, the density is the owned steps over the land area, and the validators report nothing (TEST-137)

#### Scenario: B's row
- **WHEN** the gate runs without a graph, stages or partition pair
- **THEN** it prints no G2 column and no classes line

#### Scenario: The chain sheet
- **WHEN** a 32 x 32 chain directory holds every file, its final surface 50 m below `S0` on one half, a trunk, a tributary and an H1 river cell
- **THEN** the sheet has twelve panels; panel 3 marks the H1 river cell, panel 7 the trunk and not the tributary, panel 8 the tributary, panel 12 the 50 m lowering full red and the unchanged half as shade only; a missing file leaves only its panel empty and a file of the wrong size is refused naming it

#### Scenario: The G2 bars and the dual reference
- **WHEN** a row has a graph, an uphill share of 1.0 %, agreement of 90 % and 89 % at its two thresholds, a corridor share of 97.4 % and three missing spill nodes; and stage fields with kh
- **THEN** uphill, the first agreement and the violations pass, the second agreement, the corridor (97.5 %) and the spill nodes fail, the corridor passes at M1's 95 %; the budget use counts ks, kv and kc against the budget and adds kh only in the `_s0` columns (TEST-137)

#### Scenario: A row's chain and the budget's relief
- **WHEN** a `--table` row ends in `#stages=st#chain=c.txt`, `c.txt` naming the label `r1h-proto`, a commit, a switch and a stage
- **THEN** the gate gets `--stages st` and no chain argument, the row is labelled `r1h-proto @ r1h`, the markdown table ends in a `Chains` section listing the three lines, `#reference=` is refused, and no `b_reference` column is printed (reading 28: the budget's relief is the start surface's)

#### Scenario: The run's own start
- **WHEN** a row gives `--start` and a graph with two H1 reaches and one L reach on a window starting at L (16, 8), H1 cells of 8 L cells, one H1 reach leaving the window
- **THEN** the trunks are the two H1 reaches at their H1 cell centres (H1 (3, 2) at window (11.5, 11.5)), the leaving one cut at its first vertex outside, the L reach no trunk (TEST-137)

#### Scenario: Kept interiors and cells above a head are counted apart
- **WHEN** a routed channel runs down to within one cell of a source's first vertex, a side channel joins a reach away from every source, a routed cell and a graph cell lie in a kept interior
- **THEN** the cells above the source are `routed_above_head`, the side channel's cells off the graph stay misses, and the kept cells leave both shares (TEST-137)

#### Scenario: A kept basin's interior and its river steps
- **WHEN** a kept basin's floor lies in a depression of nine cells (a 3 x 3 bowl), a drained basin's floor in another depression and a kept basin's floor on dry ground
- **THEN** the interior is the bowl's nine cells and only the first basin counts; the four river steps touching the bowl are `kept_steps`, the two rising over its rim `kept_uphill_steps`, and they leave the uphill bar (TEST-137)

#### Scenario: Missing spill nodes in the margin are counted apart
- **WHEN** two basins name a spill node the graph does not hold, one with its floor inside the counted cells and one outside
- **THEN** `g_basin_spill_missing` is 1 and `g_basin_spill_missing_edge` 1, and only the first fails `bar_spill_missing` (TEST-137)

#### Scenario: Step C's first measures
- **WHEN** a stage lowers one cell by 50.1 m and one by exactly 50 m, the stage directory holds two KC patches of 1,000.5 and 20 m3, and a table holds B and a prototype row on R1h where B gains 8.9 m of 1 km relief, the prototype gains 0.55 m, and the prototype runs 900 s against B's 435 s
- **THEN** the stage counts one cell over 50 m, KC's patches are two of 1,020.5 m3, the prototype passes `bar_relief_lost` (a gain is no loss) and fails `bar_cost`; a spectrum slope of -3.5 passes both spectrum bars and the range's ends pass (TEST-137)
