## Why
M9 "Realistic terrain" needs one number set that every generator change is judged by (SPIKE-57 recommendation 1). SPIKE-55 measured slope and relief only, with Python code outside the repo: hills and mountains came out 4 to 9 times too gentle. SPIKE-57 picked seven descriptors from the descriptors survey (Argudo et al., CGF 44(2) 2025) by cost and correlation, and sized a run at about 1 minute. This change builds that scorecard as a repeatable command (Option B item B7, M9 step R1).

## What Changes
- `planet_generators::descriptors`: pure f64 kernels with `libm`, the same result at any thread count (CON-14):
  - slope, profile and contour curvature (GEN-089);
  - local relief at 1 and 5 km, TPI at 300 m and 2 km (GEN-090);
  - geomorphons at 1 km (GEN-091);
  - depression filling, D8 drainage area and drainage density (GEN-092).
- `planet_testkit::glo30`: a reader for the Copernicus GLO-30 COG GeoTIFF tiles (deflate with the floating-point predictor, georeference, no-data) on `miniz_oxide`, a crate already in the lockfile (TEST-078).
- `crates/app/examples/terrain_score.rs`: the scorecard run.
  - For every SPIKE-55 landform, the real tile's central 0.5-degree square and the generator's two matched sites, each resampled to 30 m on an east-north grid.
  - The seven descriptors on each raster, and a distance per descriptor between the generator and the real tile (TEST-079).
- The report: `target/review/terrain-score/<date>/report.md`, readable on a phone, with `report.json` beside it and the Copernicus notice at the top (TEST-080).
- `cargo xtask terrain-score [--store DIR] [--dem DIR] [--out DIR] [--sites FILE]`: under the heavy lock, CPU only, about 1 minute (BUILD-029).

## Capabilities
### New Capabilities
- none

### Modified Capabilities
- `generator-pipeline`: GEN-089 to GEN-092 added
- `testing`: TEST-078 to TEST-080 added
- `build-tooling`: BUILD-029 added

## Impact
- Code: `generators/src/descriptors.rs` (new, about 900 lines with tests); `testkit/src/glo30.rs` (new, about 300 lines); `app/examples/terrain_score.rs` (new); `xtask/src/main.rs`, `xtask/src/terrainscore.rs` (the command); `app/src/terrainscore.rs` (the pure parts); `crates/app/data/terrain-score-sites.toml` (SPIKE-55's sites).
- Dependencies: `miniz_oxide` 0.8.9 becomes a direct dependency of `planet-testkit` (already in the lockfile through `png`; pure Rust; no new crate). `planet-testkit` stays a dev-dependency of `planet` (app), so the product binary does not link the reader.
- Data: the GLO-30 tiles stay outside the repo (`C:/DEV/SeriousPlanetData/dem/glo30-samples/`). Reports derived from them carry the Copernicus notice (`LICENCE-NOTES.md` there). Only the owner-requested baseline report is committed (tasks.md 4.2).
- No product behaviour, golden or store changes. GPU not used.
- Follow-ups enabled: M9's generator changes (hill band, crease octaves, erosion) are tuned against this scorecard, and profile and contour curvature and TPI are available as M10 driver channels.

## Decisions
- Geomorphon class table: from Jasiewicz and Stepinski (2013), checked against GRASS `r.geomorphon` `geom.c` (`determine_form`); no code adopted, no GRASS code or comments copied (GRASS is GPL). All 45 (lower, higher) pairs are pinned by a test.
- Ternary rule: the reference default (`compare_multi`): a direction is non-level only where its angle exceeds the threshold; when both the zenith and the nadir angle exceed it the larger wins, and an exact tie is "higher". It is not the paper's sum rule. Decision: follow the reference default.
- Contour curvature: uncounted where the gradient is below 1e-3, because its 1/g^3 term turns the near-flat noise of GLO-30 (lake edges, resampling) into the p1 and p99 tails. Profile curvature keeps the 1e-6 floor (GEN-089).
- Central square: the whole-tile p50 slope of N47E011 reproduces SPIKE-55's 19.3 (19.26); the central 0.5-degree square reads 23.7. The real rows of the scorecard are re-stated on the central square, not compared with SPIKE-55; the generator rows are compared (TEST-079).
- Ranking of gaps: the report names the three largest per site, and `report.json` ranks the mean over the two sites per landform (continuous: the normalised Wasserstein distance; classes: the L1; drainage density: the size of the log of the ratio); no aggregate score.
- Evidence (one run, 2026-10-31, commit b830eb38, the store and nine tiles): all 18 generator sites reproduce SPIKE-55 on slope and 1 km and 5 km relief at p50 and p99 within 2 % (worst 0.055 %, N27E086 site 2, slope p99; relief 0.00 %); total 35.4 s (tiles 0.4 s, sampling 17.4 s, descriptors 17.5 s), under the 60 s of BUILD-029. The comparison is now the ignored test `the_scorecard_reproduces_spike_55` (TEST-079), which reads SPIKE-55's generator rows from `crates/app/data/spike55-generator-rows.toml`.
- Placement: the pure parts (sites file, grids, statistics, distances, report writers) are `planet::terrainscore` in the app crate, so tier A runs their tests; the reader stays a dev-dependency used by the example only.
