## MODIFIED Requirements
### Requirement: BUILD-060 The river gate's G2 columns for river graphs
`cargo xtask river-gate` (BUILD-055) SHALL read RG and B through one build: given a canonical river graph (GEN-189), the per-stage budget fields or a partition pair, it SHALL print the G2 columns beside the raster numbers, each with its gate class (a `RIVER-GATE classes:` line, and a `class` row in the table); a row without them (B) prints none of them.

Verify: A
Status: active
Source: Lead 2026-11-17 (explorer #165; explorer #166a: the surface input; explorer #167a, owner 2026-11-17: the chain sheet as a kept tool; the river-graph plan's G2, its addendum's section 4 graph reports, the G0 contracts' sections 5, 6, 9 and reading 17), BUILD-046, BUILD-055, BUILD-057, BUILD-059, GEN-189, TEST-137
Detail:
- inputs (read by `river_gate_graph::read_graph`, `read_stages` and `read_surface`): `--surface FILE` (a candidate's surface on the window's L grid, little-endian f64, row-major, every height finite: the window's raster columns measure it against the window's start surface; not with `--store` or `--grid`), `--graph FILE` (canonical form), `--stages DIR` (`<stage>-lower.u16` and `<stage>-fill.u16` for ks, kv and kc: little-endian u16 of 0.1 m on the window's L grid, row-major; a missing file is a stage that did not run or does not fill; `residuals.csv` a header and one line per residual record), `--partition ONE,TWO` (the canonical graphs of one and of two regions); in `--table` a row takes them as suffixes `#graph=`, `#stages=`, `#partition=`, `#surface=`;
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
| Budget use | per stage over land inside, against `S0`: cells lowered, their p95 and largest lowering, cells filled and the largest fill, fills over the budget; cells whose summed lowering exceeds the budget (the fields floor each stage to 0.1 m: a count is a true overrun, an overrun under the step is not seen) and cells at 95 % of it or more; all diagnostic (the G0 contracts' section 6: the fields are never the gate; GEN-189's f64 budget validator is). Residual records and reaches flagged `budget_residual` | preservation; residual counts correctness |
| Partition | the changes between the two canonical graphs (GEN-189's diff), the topology changes among them, and the first change (`identical` for none) | partition |

- budget field: GEN-189's, from the gate's start surface, on the window's H1 blocks (`--stages` refuses an origin off an H1 corner); the centre L cell of a block is its index `block / 2`; the p50 follows BUILD-057's quantile convention; the 25 km neighbourhood is cut at the window's edge (a gate approximation of the landmass field);
- chain sheet: `cargo xtask stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]` renders a chain directory (`planet_cache::stage_sheet`, the `stage_sheet` example; no spike code) as one PNG, by default the whole window at 12 L cells per pixel to `target/review/stages/chain/<label>.png`; the layout: `chain.txt` (`stage-chain 1`; `window X0 Y0 W H` in global L cells; `coarse C`, L cells per H1 cell; `cell_m`; `sea_m`; `label`), little-endian f32 row-major on the L grid `base`, `s0`, `kh`, `ks`, `kv`, `final` (`.f32`), on the H1 grid `zs.f32` and `h1-rivers.u8` (1 on a river cell), `graph.txt` (GEN-189 canonical form) and `stages/` (the stage fields above); every file but `chain.txt` optional (its panel is grey and says `not dumped`), a wrong size refused naming the file and both sizes;
- the sheet's twelve panels, in rows of four under a title band (label, sun, km per pixel, a 50 km bar): base, `z_s`, H1 rivers on `z_s`, `S0`, KH, KS (its lowering against KH tinted), the trunk graph (H1 reaches and L reaches with a parent) on KS, tributaries (drawn when their drained area is at least 25 pixels) and basin floors (kept magenta) on KS, KV and KC (each with its stage fields tinted: lowering red, fill blue), the final surface with the trunks and the drawn tributaries, and final minus `S0` (the sea in its sea colour); one fixed scale for all panels and runs: the sun at azimuth 315 and altitude 10 degrees, grey = 0.5 + 2 (shade - sin 10 degrees) per L cell averaged over the pixel, tints full at 20 m (stages) and 50 m (change);
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
