## MODIFIED Requirements
### Requirement: BUILD-060 The river gate's G2 columns for river graphs
`cargo xtask river-gate` (BUILD-055) SHALL read RG and B through one build: given a canonical river graph (GEN-189), the per-stage budget fields or a partition pair, it SHALL print the G2 columns beside the raster numbers, each with its gate class (a `RIVER-GATE classes:` line, and a `class` row in the table); a row without them (B) prints none of them.

Verify: A
Status: active
Source: Lead 2026-11-17 (explorer #165; explorer #166a: the surface input; explorer #167a, owner 2026-11-17: the chain sheet as a kept tool; explorer #168, owner decisions 1 and 3 2026-11-17: the bars and the dual reference, reading 21; explorer #169 and #170, 2026-11-18: the chain per row, reading 28; explorer #171, owner 2026-11-18: the run's own start; explorer #172, 2026-11-18: step C's first measures; explorer #173, 2026-11-19: the base noise, reading 30; lead #166, owner 2026-11-18: kept-basin interiors and cells above a graph head counted apart; the river-graph plan's G2, its addendum's section 4 graph reports, the G0 contracts' sections 5, 6, 9 and reading 17), BUILD-046, BUILD-055, BUILD-057, BUILD-059, GEN-189, TEST-137; owner 2026-11-20 (option C, reading 37, and its three conditions: kept lakes and ponds with their own bars, the raw closed-basin share untouched, a spill mismatch a violation; lead 2026-11-21: test (a) at the rim beside test (b), and the f64 validator's records in bar_violations; lead 2026-11-22: the excused down-node rises in their own columns, reading 41; lead 2026-11-23: the lake inlets by origin and the line-end ones in no kept basin; lead #186, #186b, 2026-11-23: G3-1's gate integrity and A4-A6 of the review response, Carbon M4.7, Fable F2 and F4)
Detail:
- inputs (read by `river_gate_graph::read_graph`, `read_stages` and `read_surface`): `--surface FILE` (a candidate's surface on the window's L grid, little-endian f64, row-major, every height finite: the window's raster columns measure it against the window's start surface; not with `--store` or `--grid`), `--noise axis | turned` (the base's band and crease noise, GEN-140: `turned` with the per-octave lattice turn, the default (main's bakes since GEN-140's default, owner 2026-11-19), `axis` without it; the window's start surface and a store's base follow it, so a run on the turned base is measured on a turned start), `--start FILE` (the run's own start surface on the window's L grid, f64 as `--surface`: a chain that conditions its base has another `S0` than main's; every raster delta, the budget field and the corridor are measured against it, and the corridor's trunks are the H1 reaches of the row's `--graph` at their H1 cell centres, cut at the window's edge; without `--graph` no trunk is measured; the H1 edge-agreement column still reads main's H1; not with `--grid` or `--store`; default: the window's start surface and main's H1 trunks), `--graph FILE` (canonical form), `--stages DIR` (`<stage>-lower.u16` and `<stage>-fill.u16` for kh, ks, kv and kc: little-endian u16 of 0.1 m on the window's L grid, row-major; a missing file is a stage that did not run or does not fill; `residuals.csv` a header and one line per residual record; `budget-violations.csv` the chain's f64 budget validator records, the header `cell,stage,used_m,budget_m` and one line each, a file with another header refused), `--partition ONE,TWO` (the canonical graphs of one and of two regions); `--corridor-bar PCT` (the corridor bar; default 97.5, 95 on M1); in `--table` a row takes them as suffixes `#graph=`, `#stages=`, `#partition=`, `#surface=`, `#start=`, `#noise=`, `#timing=FILE` (a run's timing file, read by the table only: a header, then `stage,seconds,peak_mib` per line, the numbers read from the line's right end, so a stage name may hold commas; the row gets `cost_s`, the seconds summed, and `cost_peak_mib`, the largest peak, `-` when none is given), and `#chain=FILE` (a run's chain file, read by the table only: a start-surface row takes the chain's label, `<label> @ <window>`, and the markdown table ends in a `Chains` section listing each such row's `commit`, `switch` and `stage` lines);
- ids: an L cell of the window `(x, y)` is the global cell `(x0 + x, y0 + y)`, id `(gy << 32) | gx`; 8-neighbours by those coordinates;
- classes:

| Part | Columns | Class |
|---|---|---|
| Graph validity | nodes and reaches per level; GEN-189's validators on an open slice, counted by kind, without the tile-core rule (the gate has no tile map); endpoints owned elsewhere; the L `lake_inlet` nodes inside by origin (`g_lake_inlets`; `g_inlet_line_ends` some reach's down node, `g_inlet_spill_targets` only a basin's spill node, `g_inlet_orphans` neither) and `g_inlet_no_basin` the line-end inlets whose cell lies in no kept-basin interior (the row above), `g_inlet_no_basin_far` those not within one cell of one (lead 2026-11-23; a spill target between reported basins is a hierarchy record and is not counted; an inlet both a line end and a spill target counts as a line end; columns only: they count every window cell, so an inlet beside a kept lake whose floor lies outside the window, or a slice's inlet whose reach another slice owns, is counted too); `g_excused_rises` and `g_excused_rise_m` the bed rises into a reach's down node GEN-189 excuses by its `budget_residual` (reading 41), count and metres, on the graph line beside the violations | correctness |
| Trunk continuity, outlet connectivity | sources whose chain (followed through a marker along its reach) ends at an outlet, ends open, or neither; trunk reaches (H1, or L with a parent) whose held `down` node is neither an outlet nor a marker nor left by a reach | correctness |
| Junction conservation | the validators' conservation breaks | correctness |
| Bed profile | the validators' rises outside exceptions; reaches whose bed rises above its running minimum anywhere, and the largest such rise (m) | correctness |
| Exceptions | reaches per reason (`rise_over_budget`, `unresolved_endpoint`, `budget_residual`) | correctness |
| Tributary attachment | L reaches without a parent, and those ending at a held `trib_junction`, confluence, outlet or tile crossing | correctness |
| Hierarchy | on GEN-189's rasterisation over land inside: the length from each owned cell to its reach's next vertex (diagonals sqrt 2 cells), the density over the land area, Strahler streams by order (a stream of order k starts at an owned cell of order k that no donor of order k enters: the previous vertex, or at a reach's first vertex the reaches ending at its `up` node), `R_b` by BUILD-059's fit; read against BUILD-059's bar | geomorphology |
| Basins | kept and endorheic counts; a parent or a spill node the graph does not hold (a basin whose floor cell lies outside the counted cells apart, `g_basin_spill_missing_edge`: its spill path may leave the window); a same-level parent whose spill lies below the child's or whose floor lies above it | correctness |
| Graph/raster agreement | at each BUILD-059 threshold: routed river cells (land inside, drained area at least the threshold) with a rasterised graph cell within 1 cell (Chebyshev), and rasterised cells inside whose reach drains at least the threshold there (`a_geo` at the cell's vertex; G3-1) with a routed river cell within 1 cell; both shares; counted apart and outside both shares: routed and rasterised cells in a kept-basin interior (`agree<i>_routed_kept`, `agree<i>_graph_kept`), and routed cells above a graph head (`agree<i>_routed_above_head`: a cell with no graph cell within 1 cell whose downstream path on the gate's routing first comes within 1 cell of a graph cell at a cell within 1 cell of an L `source` node's anchor); each threshold's line names the three counts | correctness |
| Kept basins | a kept-basin interior (G3-1, lead #186): a basin `protected_surface`'s lake, the cells of the measured surface under its recorded spill (`spill_m`) joined (8-connected) to its floor cell inside the depression holding the floor (BUILD-057's basin flood: a rim cut under the spill or a drained cell does not let it run on); a floor at or above its spill, or in no depression, holds none; `kept_basins` the kept basins whose floor lies in the window under its spill; `kept_cells` (inside), `kept_steps` (BUILD-057's river steps with either end in an interior) and `kept_uphill_steps` (those rising more than 0.25 m); a `RIVER-GATE kept basins` line with the four counts | correctness |
| Closed basins and kept lakes | on a row with a graph (a kept basin is a lake, not a closed basin): `ponds_cells` the river cells inside in a closed basin (BUILD-055) outside every kept-basin interior (the row above), `ponds_pct` their share of the river cells inside outside the interiors; `kept_river_cells` and `kept_river_pct` the river cells inside in an interior, over all river cells inside; `kept_lake_cells` and `kept_lake_km2` the interiors' cells inside; `g_spill_mismatch` the kept basins whose floor lies in the window and whose depression level there (BUILD-057's basin flood) lies more than 0.5 m under the recorded spill (test b); `g_spill_rim` the kept basins whose floor lies in the window and whose rim (the cells outside the floor's depression that touch it, 8-connected) holds an L vertex whose bed lies more than 0.5 m under the recorded spill (test a: a line leaving the lake below its spill); BUILD-055's `basin_cells` and `basin_pct` unchanged; on a row without a graph these columns are `-` | geomorphology |
| Budget use | per stage over land inside (kh: `S0` against KH; ks, kv, kc: against `S_KH` when the fields are, reading 21): cells lowered, their p95 and largest lowering, cells filled and the largest fill, fills over the budget; cells whose summed lowering of ks, kv and kc exceeds the budget and cells at 95 % of it or more (`b_overrun`, `b_at_95`), and both with kh's lowering added, against `S0` (`b_overrun_s0`, `b_at_95_s0`); the fields floor each stage to 0.1 m: a count is a true overrun, an overrun under the step is not seen; all diagnostic (the G0 contracts' section 6: the fields are never the gate; GEN-189's f64 budget validator is). Residual records and reaches flagged `budget_residual`; `b_validator_overrun` the f64 validator's overruns and raises in a lowering stage as the chain writes them beside the stage fields (`budget-violations.csv`, a header `cell,stage,used_m,budget_m` and one line each; `-` without the file) | preservation; residual counts and `b_validator_overrun` correctness |
| Partition | the changes between the two canonical graphs (GEN-189's diff), the topology changes among them, and the first change (`identical` for none) | partition |
| G2 bars (owner decision 1, lead #168) | on a row with a graph, each `PASS:<value>` or `FAIL:<value>`: `bar_corridor` the snapped k_c 2 trunk reaches passing (BUILD-057) >= the corridor bar; `bar_uphill` river steps rising over 0.25 m (BUILD-057) <= 1.0 %, over the steps outside kept-basin interiors (uphill steps less `kept_uphill_steps`, over steps less `kept_steps`); `bar_agree_<A>km2` the lower of the routed and the graph share of the graph/raster agreement >= 90 % at each threshold (without the cells counted apart; a side with no cells counts 100 %); `bar_above_head_<A>km2` `agree<i>_routed_above_head` over the routed cells with them <= 10 % (G3-1, lead #186; proposed for the owner, O10); `bar_trunk_exceptions` the trunk reaches (H1, or L with a parent) flagged `rise_over_budget` or `budget_residual` (`g_trunk_exceptions`) over the trunk reaches <= 1 % (A6; proposed, O10); `bar_violations` = 0, the validators' violations plus `g_spill_mismatch`, `g_spill_rim` and `b_validator_overrun`; `bar_spill_missing` = 0 (basins whose floor cell lies inside); `bar_ponds` `ponds_pct` <= 10 %; `bar_kept_river` `kept_river_pct` <= 5 %, relief-indexed to 10 % (the row below); `bar_uphill_1km2` and `bar_ponds_1km2` the uphill and ponds shares on the 1 km2 network (the row below) <= 1.0 % and <= 10 % (proposed, O10); `bar_hump_cuts` (the row below) per 10,000 km2 of counted land <= 1 (from B's row, 0.04 to 0.24 on the five windows) and `bar_corner_links` <= 10 (both proposed, O10); a `RIVER-GATE bars` line with the count passing | corridor preservation; the rest correctness |
| G3-1 measures (lead #186) | on every row: the 1 km2 network (river cells inside on land draining at least 1 km2): `fine_river_cells`, `fine_uphill_steps` and `fine_uphill_pct` (BUILD-057's river steps rising over 0.25 m, both ends outside the kept-basin interiors), `fine_ponds_cells` and `fine_ponds_pct` (river cells in a closed basin, BUILD-055, outside the interiors, over the river cells outside them); hump cuts: an 8-connected group of counted land cells lowered more than 0.25 m against a reference is a path, its two ends the ends of its longest walk (a breadth-first search from its first cell to the farthest cell, a second from there; ties by cell index), a hump cut a path along which the reference rises more than 10 m above its highest end and that is cut more than 10 m somewhere, `hump_cuts` against `S0` and `hump_cuts_s_kh` against `S_KH` (with KH's stage field; printed beside, not barred), `hump_cuts_per_10k_km2`; corner-only links: 2 x 2 blocks of counted land cells whose one diagonal pair stands more than 5 m above both cells of the other, `corner_links` on the surface and `corner_links_s0` on `S0`, `corner_links_added_per_10k_km2` their difference per 10,000 km2; the margin (Carbon M4.7): the window's cells outside the region's box, `m_cells`, `m_river_cells` (75 km2 network), `m_basin_cells` and `m_basin_pct`, `m_ponds_cells` and `m_ponds_pct`, `m_uphill_steps` and `m_uphill_pct` (outside the interiors) and `m_kept_lake_km2`, reported and not barred; relief-indexed limits (A5, owner 2026-11-21): the corridor, crest and kept-river bars take their lowland value (97.5 %, 15 m, 5 %) up to a start relief (`relief10_start_m`) of 60 m and their mountain value (95 %, 35 m, 10 %) from 300 m, linear in the logarithm of the relief between (both endpoints proposed by the lead for the owner, O10); `--corridor-bar` fixes the corridor bar by hand, and no bar is keyed on a window | uphill and trunk exceptions correctness; ponds, hump cuts and corner links geomorphology; margin columns diagnostic |
| Step C, first measures (lead #172) | per stage, cells lowered by more than 50 m (`b_<stage>_over50`); KC's patches, count and volume (`b_kc_patches`, `b_kc_patch_km3`) from `kc-patches.csv` in the stage directory (a header `cells,volume_m3`, one line per patch); `relief2_lost_m` and `relief10_lost_m` (the start's relief minus the surface's, positive is lost); `beta_change` (the spectrum slope minus the start's); the bars `bar_spectrum_steep` (beta >= -4.13) and `bar_spectrum_rough` (beta <= -2.05), the range of SPIKE-119's 25 GLO-30 and 3DEP tiles; in `--table`, each row with a graph against the store row of the same window (B; a row's window is its label's part after ` @ `; a start-surface row gets none): `bar_relief_lost` (`relief10_lost_m` at most `max(B's, 0)`: a gain of relief is no loss), `bar_beta_change` (`abs(beta_change)` at most B's) and `bar_cost` (`cost_s` at most B's, when both rows carry a timing) | over-50 and patches preservation; spectrum bars geomorphology; `bar_relief_lost` preservation, `bar_beta_change` geomorphology, `bar_cost` product |
| Step C, ridges (lead #172) | on every row, one ridge mask on the start: a cell inside, on land, at least 2 m above both cells 2 L cells away along a face axis or a diagonal (the G0 contracts' section 9); against the reference `S_KH` (the start less the stage directory's `kh-lower.u16`, plus `kh-fill.u16`, in 0.1 m steps; reading 21) and against `S0` (the start; `ridge_ref` `s_kh`, or `s0` without a KH field, when both are the start): ridge cells lowered more than 20 m, the p95 and the largest lowering over the ridge cells (a raise counts as 0); per channel threshold 1 and 5 km2, cut-through cells (a lowered ridge cell with a cell of the surface's routing draining at least the threshold in its 3 x 3 and none of the reference's routing there, each reference routed as the surface is) and saddles (their 8-connected groups); columns `ridge_cells`, `ridge_lowered`, `ridge_lowered_pct`, `ridge_p95_m`, `ridge_max_m`, `cut1_cells`, `saddles1`, `cut5_cells`, `saddles5` against the reference, `ridge_s0_lowered_pct`, `ridge_s0_p95_m`, `saddles1_s0`, `saddles5_s0` against `S0`; a `RIVER-GATE ridges` line; on a row with a graph the bar `bar_crest` (`ridge_p95_m` at most 15 m, relief-indexed to 35 m); in `--table`, each row with a graph against B's row of the same window: `bar_saddles1` and `bar_saddles5` (`saddles1`, `saddles5` at most B's) and `bar_saddles_s0` (`saddles1_s0` at most B's `saddles1`, B's reference being `S0`; owner decision 3 revisited, G3-1) | preservation |

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
- **THEN** uphill, the first agreement and the violations pass, the second agreement, the corridor (97.5 %) and the spill nodes fail, the corridor passes at the mountain value 95 %; the budget use counts ks, kv and kc against the budget and adds kh only in the `_s0` columns (TEST-137)

#### Scenario: G3-1's bars
- **WHEN** a row has a graph whose agreement's graph share is lower than its routed share, routed cells above a graph head of 9.9 % and 10.7 % at its two thresholds, 2 and then 3 of 200 trunk reaches flagged, a path lowered 15 m through a 15 m reference hump, a 2 x 2 block whose low pair meets only at a corner, a pit in its margin, and a start relief of 10 m and of 400 m
- **THEN** the agreement bar takes the graph share, the first above-head bar passes and the second fails, 2 trunk exceptions pass and 3 fail, one hump cut and one corner link are counted, the margin's pit is in `m_*` and in no bar, and the corridor, crest and kept-river limits are 97.5 %, 15 m and 5 % at 10 m and 95 %, 35 m and 10 % at 400 m (TEST-137)

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

#### Scenario: A kept lake on the river
- **WHEN** a river runs through a closed basin that holds a kept basin's floor, on a row with a graph
- **THEN** its river cells count in `kept_river_cells` and `bar_kept_river`, not in `ponds_cells` or `bar_ponds`, and BUILD-055's `basin_cells` still counts them

#### Scenario: A spill mismatch
- **WHEN** a kept basin's depression level at its floor lies more than 0.5 m under its recorded spill
- **THEN** `g_spill_mismatch` counts it and `bar_violations` fails

#### Scenario: A line leaves a kept lake below its spill
- **WHEN** an L vertex on a kept basin's rim has its bed more than 0.5 m under the basin's recorded spill
- **THEN** `g_spill_rim` counts the basin once and `bar_violations` fails; with the beds at the spill it counts none

#### Scenario: The f64 validator finds an overrun
- **WHEN** the stage directory's `budget-violations.csv` holds a line
- **THEN** `b_validator_overrun` counts it and `bar_violations` fails; a missing file prints `-` and adds nothing; a line not of four fields is refused naming the file

#### Scenario: A line ends at an inlet of no kept lake
- **WHEN** a reach ends at a `lake_inlet` whose cell lies in no kept-basin interior, while a reported basin spills to another inlet outside them
- **THEN** `g_inlet_no_basin` counts the first and not the second, and `g_inlet_spill_targets` counts the second

## ADDED Requirements
### Requirement: BUILD-066 G3-8's out-of-sample windows, a seeded stratified draw
`cargo xtask windows-draw [--out FILE] [--threads N]` SHALL draw 10 windows of the five windows' size over the seed-14 world's coarse field by fixed rules and a recorded seed, none overlapping one of the five windows or another drawn window, and write them as data (`crates/cache/data/g3-windows.toml`), so that G3-8's windows are fixed before anyone looks at their terrain and no RG or B run happens on them before G3-8.

Verify: A
Status: active
Source: Lead 2026-11-23 (#186a: the 10 out-of-sample windows as a seeded stratified draw, not hand-picked; the G3 gate's rows G3-1 and G3-8, risk R18), BUILD-055, BUILD-060, TEST-149
Detail:
- the candidates: on each face, boxes of 620 x 620 regional cells (of 12,288 a face edge) at `0, 620, 1240, ...` on each axis while they fit, and one more at `12,288 - 620`;
- the samples: 16 x 16 per box, at regular sub-box centres: the class inputs' base elevation (land above 0 m), the orography's precipitation and temperature (runoff by `runoff_mm`), and the latitude of the box's centre;
- a box's statistics: the land share, the p50 and p90 of the land samples' elevation, their mean runoff (mm a year), and the absolute latitude (degrees);
- a box lies in a face edge's band when it comes within 204 regional cells of that edge (the gate's 200-cell margin and the C1 read's 4);
- the strata, the first rule that holds, with their quota:

| Stratum | Rule | Quota |
|---|---|---|
| cube corner | in two edges' bands, land at least 20 % | 1 |
| cube edge | in one edge's band, land at least 30 % | 2 |
| high mountain | in no band, land at least 80 %, land p50 at least 1,500 m | 2 |
| arid or endorheic | in no band, land at least 80 %, land runoff under 50 mm a year | 2 |
| flat coast | in no band, land 20 to 80 %, land p90 under 300 m | 2 |
| mixed mid-latitude | in no band, land 40 to 90 %, latitude 30 to 60 degrees | 1 |

- the draw: per stratum in the table's order, its candidates in the order of a splitmix64 hash of the draw seed (`0x20261123`), the face and the box's corner (ties by face and corner), each taken unless it overlaps one of the five windows or an earlier drawn window, until the quota is met; a stratum short of candidates is refused, naming it and the count found;
- the data file: `world_seed`, `draw_seed`, `size`, and one `[[window]]` per drawn window with its draw `index`, `class`, `window` (`face,i0,i1,j0,j1,seed`) and statistics;
- the edge and corner windows reach the face's edge: the gate measures them once cube faces are defined (G3-5); until then they are data only.

#### Scenario: The draw
- **WHEN** candidates of every stratum are classed and drawn twice with the same seed and once with another
- **THEN** each stratum's quota is met, no drawn window overlaps one of the five windows or another, the same seed draws the same windows, another seed draws others, and a stratum with no candidate is refused naming it (TEST-149)
