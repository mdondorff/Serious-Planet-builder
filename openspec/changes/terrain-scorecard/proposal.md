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
- Code: `generators/src/descriptors.rs` (new, about 900 lines with tests); `testkit/src/glo30.rs` (new, about 300 lines); `app/examples/terrain_score.rs` (new); `xtask/src/main.rs`, `xtask/src/testcmd.rs` (the command); `crates/app/data/terrain-score-sites.toml` (SPIKE-55's sites).
- Dependencies: `miniz_oxide` 0.8.9 becomes a direct dependency of `planet-testkit` (already in the lockfile through `png`; pure Rust; no new crate). `planet-testkit` stays a dev-dependency of `planet` (app), so the product binary does not link the reader.
- Data: the GLO-30 tiles stay outside the repo (`C:/DEV/SeriousPlanetData/dem/glo30-samples/`). Reports derived from them carry the Copernicus notice (`LICENCE-NOTES.md` there). Only the owner-requested baseline report is committed (tasks.md 4.2).
- No product behaviour, golden or store changes. GPU not used.
- Follow-ups enabled: M9's generator changes (hill band, crease octaves, erosion) are tuned against this scorecard, and profile and contour curvature and TPI are available as M10 driver channels.
