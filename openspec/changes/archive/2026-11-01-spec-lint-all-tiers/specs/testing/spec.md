## MODIFIED Requirements

### Requirement: TEST-001 Spec traceability
Every requirement in the specs SHALL have a unique trace ID, a verification tier (A, B, C or D) and a status (active, planned or deprecated); every `// spec: ID` citation in the code SHALL name an existing requirement or constitution rule; every active tier A, B or C requirement SHALL be cited by at least one test of each declared tier, where a citation counts only when the `// spec:` comment sits directly above a `#[test]` function (only comments and attributes, including wrapped multi-line ones, may sit between; only the `#[test]` attribute counts) and integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B and all others (including unit tests under `src/`) tier A, matching the test runner, and a `// spec:` comment directly above a performance-harness scene entry point (a `fn perf_*` item under `crates/app/src/`, with the same comments-and-attributes rule between) counts as tier C evidence only (CON-17: the scene measures what no test may; there is no further tier); every tier D requirement SHALL name its review artefact; and every main spec SHALL have a Purpose section that is not empty and does not contain the placeholder "TBD" (in any case).

Verify: A
Status: active
Source: CON-16, ADR 0009, ADR 0012

#### Scenario: Placeholder Purpose
- **WHEN** a spec's Purpose is missing, empty or contains "TBD"
- **THEN** `cargo xtask spec-lint` fails and names the spec

#### Scenario: Uncited active requirement
- **WHEN** an active tier A requirement has no citing test
- **THEN** `cargo xtask spec-lint` fails and names the requirement and the comment to add

#### Scenario: Unknown citation
- **WHEN** a test cites an ID that is not in the specs or the constitution
- **THEN** `cargo xtask spec-lint` fails and names the file and the ID

#### Scenario: Citation not on a test
- **WHEN** a `// spec:` comment sits above a helper function, a constant or a module instead of a `#[test]` function
- **THEN** it does not count towards coverage and the requirement is reported as not cited

#### Scenario: Wrong tier
- **WHEN** an active tier C requirement is cited only by tests outside `gpu_*` files
- **THEN** `cargo xtask spec-lint` fails and says which tiers cite it and which tier is required
- **AND** a requirement of several tiers (`Verify: A, C`) fails for each declared tier no test of that tier cites, and a tier other than A, B, C or D on the line is refused

#### Scenario: Perf scene citation
- **WHEN** a `// spec:` comment sits directly above a `fn perf_*` item under `crates/app/src/`
- **THEN** it counts as tier C evidence of the requirement (and of no other tier); above any other function, in another crate or under `tests/` it covers nothing

### Requirement: TEST-078 The GLO-30 reader
`planet_testkit::glo30` SHALL read a Copernicus GLO-30 COG GeoTIFF tile: the TIFF tags, deflate (through `miniz_oxide`) with the floating-point predictor, tiles or strips, the georeference (origin and pixel scale) and the no-data value, into an f32 raster with marked no-data cells. It SHALL refuse a tile with any other compression, predictor or sample format, naming the tag. `planet-testkit` stays a dev-dependency.

Verify: A
Status: active
Source: SPIKE-55 (reader in Python, outside the repo), SPIKE-57, GLO-30 licence notes, ADR 0009

#### Scenario: Round trip
- **WHEN** a test writes a small TIFF with deflate and predictor 3 in two tiles and reads it back
- **THEN** the floats, the georeference and the no-data cells equal the input, and a tile with LZW compression is refused naming the compression tag

#### Scenario: A real tile
- **WHEN** `PLANET_GLO30_DIR` names the sample tiles and N47E011 is read
- **THEN** the p50 slope of the whole tile (central differences at the true metric spacing) lies within 0.5 degrees of SPIKE-55's 19.3, which checks the decoder; without the variable the check is NOT RUN, never a pass

### Requirement: TEST-079 The terrain scorecard run
The scorecard run SHALL compute, for each landform of the sites file, the seven descriptors (GEN-089 to GEN-092) on three rasters: the real tile's central 0.5-degree square and the generator's two matched sites (`world.field()` of the store), each 1,853 x 1,853 cells at 30 m on an east-north grid. Per site and descriptor it SHALL give a distance to the tile: Wasserstein-1 over percentiles p1 to p99 divided by the real p5-to-p95 range for continuous descriptors, L1 between class histograms for geomorphons, and the generator-to-real ratio for drainage density.

Verify: A
Status: active
Source: SPIKE-57 (seven descriptors, about 1 minute), SPIKE-55 (sites, 30 m windows), ADR 0009

#### Scenario: Distances
- **WHEN** two identical percentile sets and two sets shifted by a quarter of the real p5-to-p95 range are compared, and two class histograms with disjoint support
- **THEN** the distances are 0, 0.25 and 2

#### Scenario: SPIKE-55 reproduced
- **WHEN** the ignored test `the_scorecard_reproduces_spike_55` runs over the store (`cargo test -p planet --release --test it the_scorecard_reproduces_spike_55 -- --ignored --nocapture`)
- **THEN** each generator site's slope and 1 km and 5 km relief p50 and p99 lie within 2 % of SPIKE-55's generator rows (`crates/app/data/spike55-generator-rows.toml`); the real rows are re-stated on the central 0.5-degree square (N47E011 slope p50 is 23.7, not the whole tile's 19.3), not compared; without the store the check is NOT RUN, never a pass
