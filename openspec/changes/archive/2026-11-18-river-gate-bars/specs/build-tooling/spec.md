## MODIFIED Requirements
### Requirement: BUILD-060 The river gate's G2 columns for river graphs
`cargo xtask river-gate` (BUILD-055) SHALL read RG and B through one build: given a canonical river graph (GEN-189), the per-stage budget fields or a partition pair, it SHALL print the G2 columns beside the raster numbers, each with its gate class (a `RIVER-GATE classes:` line, and a `class` row in the table); a row without them (B) prints none of them.

Verify: A
Status: active
Source: Lead 2026-11-17 (explorer #165; explorer #166a: the surface input; explorer #167a, owner 2026-11-17: the chain sheet as a kept tool; explorer #168, owner decisions 1 and 3 2026-11-17: the bars and the dual reference, reading 21; the river-graph plan's G2, its addendum's section 4 graph reports, the G0 contracts' sections 5, 6, 9 and reading 17), BUILD-046, BUILD-055, BUILD-057, BUILD-059, GEN-189, TEST-137
Detail:
- inputs (read by `river_gate_graph::read_graph`, `read_stages` and `read_surface`): `--surface FILE` (a candidate's surface on the window's L grid, little-endian f64, row-major, every height finite: the window's raster columns measure it against the window's start surface; not with `--store` or `--grid`), `--graph FILE` (canonical form), `--stages DIR` (`<stage>-lower.u16` and `<stage>-fill.u16` for kh, ks, kv and kc: little-endian u16 of 0.1 m on the window's L grid, row-major; a missing file is a stage that did not run or does not fill; `residuals.csv` a header and one line per residual record), `--partition ONE,TWO` (the canonical graphs of one and of two regions); `--reference FILE` (`S_KH` on the window's L grid, f64 as `--surface`; with `--stages` only); `--corridor-bar PCT` (the corridor bar; default 97.5, 95 on M1); in `--table` a row takes them as suffixes `#graph=`, `#stages=`, `#partition=`, `#surface=`, `#reference=`;
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
| Basins | kept and endorheic counts; a parent or a spill node the graph does not hold; a same-level parent whose spill lies below the child's or whose floor lies above it | correctness |
| Graph/raster agreement | at each BUILD-059 threshold: routed river cells (land inside, drained area at least the threshold) with a rasterised graph cell within 1 cell (Chebyshev), and rasterised cells inside with a routed river cell within 1 cell; both shares | correctness |
| Budget use | per stage over land inside (kh: `S0` against KH; ks, kv, kc: against `S_KH` when the fields are, reading 21): cells lowered, their p95 and largest lowering, cells filled and the largest fill, fills over the budget; cells whose summed lowering of ks, kv and kc exceeds the budget and cells at 95 % of it or more (`b_overrun`, `b_at_95`), and both with kh's lowering added, against `S0` (`b_overrun_s0`, `b_at_95_s0`); the fields floor each stage to 0.1 m: a count is a true overrun, an overrun under the step is not seen; all diagnostic (the G0 contracts' section 6: the fields are never the gate; GEN-189's f64 budget validator is). Residual records and reaches flagged `budget_residual` | preservation; residual counts correctness |
| Partition | the changes between the two canonical graphs (GEN-189's diff), the topology changes among them, and the first change (`identical` for none) | partition |
| G2 bars (owner decision 1, lead #168) | on a row with a graph, each `PASS:<value>` or `FAIL:<value>`: `bar_corridor` the snapped k_c 2 trunk reaches passing (BUILD-057) >= the corridor bar; `bar_uphill` river steps rising over 0.25 m (BUILD-057) <= 1.0 %; `bar_agree_<A>km2` the routed share of the graph/raster agreement >= 90 % at each threshold; `bar_violations` = 0; `bar_spill_missing` = 0; a `RIVER-GATE bars` line with the count passing | corridor preservation; the rest correctness |

- budget field: GEN-189's, from `--reference` (`S_KH`) when given, else from the gate's start surface (the `b_reference` column names which), on the window's H1 blocks (`--stages` refuses an origin off an H1 corner); the centre L cell of a block is its index `block / 2`; the p50 follows BUILD-057's quantile convention; the 25 km neighbourhood is cut at the window's edge (a gate approximation of the landmass field);
- chain sheet: `cargo xtask stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]` renders a chain directory as one PNG (`planet_cache::stage_sheet`, the `stage_sheet` example; no spike code);
  - default: the whole window at 12 L cells per pixel, written to `target/review/stages/chain/<label>.png` (every character of the label but a letter, a digit or `-` written as `-`; `chain.png` without a label);
  - `chain.txt`: `stage-chain 1`, then `window X0 Y0 W H` (global L cells), `coarse C` (L cells per H1 cell), `cell_m M`, and optionally `sea_m S` (default 0) and `label TEXT`;
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
