## Context
- SPIKE-57 (`spikes/m9-schott-publications/results/results.md`, "What changes in the scorecard"): seven descriptors, chosen by the survey's cost factors (one thread, 1024^2, average slope 18 ms) and its correlation matrix (one descriptor stands for each correlated group).
- SPIKE-55 (`spikes/m9-glo30-targets/results/results.md`): nine GLO-30 tiles, one per landform; two generator sites per tile, matched on mean and standard deviation of height per 1 degree cell; slope and relief at 30 m; Python reader and metrics outside the repo.
- GLO-30 licence: `C:/DEV/SeriousPlanetData/dem/glo30-samples/LICENCE-NOTES.md` (Art. 6 notices for derived data). The reference code of the survey (MIT) is read for definitions only; no code is adopted.
- CON-14: generator code uses f64 and `libm`, with no thread-count-dependent results; the descriptors live in `generators` so M10 can reuse them as drivers.

## Decisions
### 1. Placement and dependencies
| Part | Where | Why |
|---|---|---|
| Descriptor kernels | `planet_generators::descriptors` | pure f64 functions on a raster; reusable as M10 drivers (the driver set of the removed chunk stage, ADR 0086) |
| GLO-30 reader | `planet_testkit::glo30` | test data input; `planet-testkit` is a dev-dependency only (its manifest says so), so the product binary does not link it |
| Run | `crates/app/examples/terrain_score.rs` | an example may use dev-dependencies and the app's world (store, `world.field()`); same pattern as other `cargo xtask heavy` examples |
| Command | `cargo xtask terrain-score` | runs the example in release under the heavy lock (BUILD-026) |
| Sites | `crates/app/data/terrain-score-sites.toml` | SPIKE-55's tiles and matched sites as data, editable without code |

Reader dependency: `miniz_oxide` 0.8.9 (pure Rust inflate; MIT OR Zlib OR Apache-2.0; about 6,900 lines of source; one dependency, `adler2`). It is already in `Cargo.lock` through `png`, so the build gains no new crate. It is added to `planet-testkit` only. TIFF tag parsing and the floating-point predictor (predictor 3: byte-plane unshuffle plus horizontal differencing) are written by hand (about 300 lines); no GeoTIFF crate.

### 2. Rasters
- Real: the central 0.5-degree square of each 1-degree tile (1,800 x 1,800 samples at 1 arc-second; longitude spacing scaled by `cos(latitude)`), resampled bilinear onto a 30 m east-north grid of 1,853 x 1,853 cells centred on the tile centre (the tile's own 0.5-degree window). This is a change from SPIKE-55, which used the whole tile; it puts both sides on equal footing and quarters the cost.
- Generator: `world.field()` of the store (no chunk stage), the same 30 m east-north grid of 1,853 x 1,853 cells around each site, directions built in f64 (SPIKE-55's probe), heights read with the field's threaded read (deterministic per sample).
- No-data cells (GLO-30's `-32767`, ocean voids) are marked. A descriptor at a cell whose window holds a no-data cell is not counted.
- Edge: a descriptor with radius r is counted only for cells at least r from the raster edge (no padding).

### 3. Descriptors (definitions; cell size h = 30 m)
| # | Descriptor | Definition | Cost (factor of slope, SPIKE-57) |
|---|---|---|---|
| 1 | Slope (degrees) | `atan(|grad z|)`, central differences | 1 |
| 2 | Profile and contour curvature (1/m) | Evans-Young quadric fit over the 3x3 window; profile along the gradient, contour across it; 0 where the gradient is under 1e-6 | 8 |
| 3 | Local relief at 1 and 5 km (m) | max minus min over non-overlapping square blocks of 33 and 167 cells (SPIKE-55's blocks), so SPIKE-55's numbers are reproduced | 2 |
| 4 | TPI at 300 m and 2 km (m) | `z` minus the mean over the square window of side `2r+1` cells (r = 10 and 67), from summed-area tables accumulated in f64 in row order | 16 |
| 5 | Geomorphon class at 1 km | 8 directions, lookup distance 33 cells, flatness threshold 1 degree; zenith and nadir angles give a ternary pattern; the 10 classes of Jasiewicz and Stepinski (2013): flat, peak, ridge, shoulder, spur, slope, hollow, footslope, valley, pit | 41 |
| 6 | Drainage area D8 (m2) and drainage density (km per km2) | priority-flood filling with an epsilon of 1e-3 m (Barnes et al. 2014) and ties broken by cell index; D8 receivers (steepest descent, ties by direction order N, NE, E, ...); area accumulated in topological order; density = channel length (cells with area of at least 1e5 m2, h or h*sqrt(2) per step to the receiver) over the counted area | 7 |
- Thread independence: kernels split rows across rayon workers and write disjoint output rows; every reduction (percentiles, sums, histograms) runs on one thread in index order. The flood fill and accumulation run single-threaded per raster (rasters run in parallel).

### 4. Distances (per descriptor, per site against its tile)
| Kind | Distance | Reported beside it |
|---|---|---|
| Continuous (slope, two curvatures, two reliefs, two TPIs, drainage area) | 1-D Wasserstein-1 over the percentiles p1 to p99, divided by the real p5-to-p95 range (dimensionless; 0 is equal) | generator / real at p50 and p99 |
| Geomorphons | L1 between class histograms (0 to 2) | the three classes with the largest gap |
| Drainage density | ratio generator / real | both values |
- No aggregate score. The report names, per landform, the three descriptors with the largest distance.

### 5. Report (phone-readable)
- `report.md`: the Copernicus notice and DOI first; then one short section per landform. Each holds a table of at most four columns (descriptor, real p50, generator p50 at both sites, distance) and one line naming the three largest gaps. Plain words, units in the header, no wide tables, no images required.
- `report.json`: every percentile, histogram and distance, plus the run's inputs: store path and definition hash, tile names, sites, commit, thread count, and wall time per phase.
- `--maps` (optional): one 512 px PNG per descriptor per raster under `maps/`, with the notice in `report.md`. These are not committed.

### 6. Budget
| Phase | Estimate |
|---|---|
| Read and resample 9 tiles | about 5 s (inflate of 9 x 25 MB) |
| Sample 18 generator windows of 3.4 M cells | to be measured in PR 3; SPIKE-55 did not record it; the budget assumes at most 30 s |
| Descriptors on 27 rasters of 3.4 M cells | about 75 slope-times per raster: 75 x 0.06 s = 4.5 s on one thread, so about 10 s with rasters in parallel |
| Report | under 1 s |
| Total | **at most 60 s** on Rasierklinge (BUILD-029). A correctness run, not a perf verdict: it needs no window (ADR 0087 point 3 lists no such check) |

## Tests (tier A unless named)
| Kernel | Synthetic input | Expected |
|---|---|---|
| Slope | plane rising 10 degrees east | 10 degrees at every counted cell (1e-9) |
| Curvature | paraboloid `z = a (x^2 + y^2)` and trough `z = a x^2`, sampled at `(x0, 0)` with `x0 > 0` | paraboloid: profile `2a / (1 + 4 a^2 x0^2)^(3/2)` and contour `1 / x0` (the contour circle), within 1 %; trough: the same profile, contour 0; sign convention as GEN-089 |
| Relief | a 100 m step through a block | 100 m in that block, 0 in flat blocks |
| TPI | `z = A sin(2 pi x / L)` with L much longer than the window | sign follows the crest and the trough; summed-area equals the direct mean (1e-9) |
| Geomorphons | cone, inverted cone, V valley, ridge, plane | peak at the cone's apex, pit at the inverted one, valley and ridge along the axes, slope and flat on the plane |
| Drainage | a tilted V valley with a pit | the pit is filled; area grows downstream by one cell row each step; density equals the analytic channel length over the area |
| Determinism | every kernel at 1 and 8 threads | byte-identical outputs |
| Reader | a small TIFF written by the test (deflate, predictor 3, two strips or tiles) | the decoded floats equal the input; georeference and no-data read back |
| Distances | two identical and two shifted percentile sets | 0 and the known shift over the range |
| Report | a synthetic run result | the notice is the first line after the title; no table over four columns |
- Tier B (ignored, `PLANET_GLO30_DIR`): the reader decodes `N47E011` and the central-square p50 slope is within 0.5 degrees of SPIKE-55's 19.3. NOT RUN when the directory is absent.

## Risks
| Risk | Control |
|---|---|
| Generator sampling dominates the run (18 x 3.4 M field reads) | measured in PR 3. If over 30 s, fewer threads per window and more windows in parallel, or a coarser sampling with the relief blocks unchanged |
| Geomorphon definitions differ between implementations | the 10-class lookup table is written into GEN-091's detail and tested on the synthetic forms |
| GLO-30 is a surface model (canopy, buildings, 1-2 m noise) | stated in the report's notes, as in SPIKE-55; plains' slope gaps are upper bounds |
| A reader bug passes the synthetic TIFF | the tier B check against SPIKE-55's numbers on a real tile |
| Licence notice missing from a published derivative | TEST-080's report test; the committed baseline carries it |

## Open points
1. Whether M9's acceptance turns per-descriptor distances into bounds (an M9 planning decision; this change reports only).
2. Hillslope asymmetry and HAND were dropped by SPIKE-57; re-add only if a generator change needs them.
