## ADDED Requirements

### Requirement: TEST-078 The GLO-30 reader
`planet_testkit::glo30` SHALL read a Copernicus GLO-30 COG GeoTIFF tile: the TIFF tags, deflate (through `miniz_oxide`) with the floating-point predictor, tiles or strips, the georeference (origin and pixel scale) and the no-data value, into an f32 raster with marked no-data cells. It SHALL refuse a tile with any other compression, predictor or sample format, naming the tag. `planet-testkit` stays a dev-dependency.

Verify: A, B
Status: planned
Source: SPIKE-55 (reader in Python, outside the repo), SPIKE-57, GLO-30 licence notes

#### Scenario: Round trip
- **WHEN** a test writes a small TIFF with deflate and predictor 3 in two tiles and reads it back
- **THEN** the floats, the georeference and the no-data cells equal the input, and a tile with LZW compression is refused naming the compression tag

#### Scenario: A real tile
- **WHEN** `PLANET_GLO30_DIR` names the sample tiles and N47E011 is read
- **THEN** the p50 slope of its central 0.5-degree square at 30 m lies within 0.5 degrees of SPIKE-55's 19.3; without the variable the check is NOT RUN

### Requirement: TEST-079 The terrain scorecard run
The scorecard run SHALL compute, for each landform of the sites file, the seven descriptors (GEN-089 to GEN-092) on three rasters: the real tile's central 0.5-degree square and the generator's two matched sites (`world.field()` of the store), each 1,853 x 1,853 cells at 30 m on an east-north grid. Per site and descriptor it SHALL give a distance to the tile: Wasserstein-1 over percentiles p1 to p99 divided by the real p5-to-p95 range for continuous descriptors, L1 between class histograms for geomorphons, and the generator-to-real ratio for drainage density.

Verify: A
Status: planned
Source: SPIKE-57 (seven descriptors, about 1 minute), SPIKE-55 (sites, 30 m windows)

#### Scenario: Distances
- **WHEN** two identical percentile sets and two sets shifted by a quarter of the real p5-to-p95 range are compared, and two class histograms with disjoint support
- **THEN** the distances are 0, 0.25 and 2

#### Scenario: SPIKE-55 reproduced
- **WHEN** the run reads the store and the tiles
- **THEN** each generator site's slope and 1 km and 5 km relief p50 and p99 lie within 2 % of SPIKE-55's rows

### Requirement: TEST-080 The scorecard report reads on a phone
The run SHALL write `report.md` and `report.json` under `target/review/terrain-score/<date>/`. `report.md` begins with the Copernicus WorldDEM-30 notice for derived data and the no-liability sentence (`LICENCE-NOTES.md` of the tiles), then one section per landform: a table of at most four columns and one line naming the three descriptors with the largest distance. `report.json` holds every percentile, histogram and distance with the run's inputs and the wall time per phase.

Verify: A
Status: planned
Source: SPIKE-57, GLO-30 licence (Art. 6), owner (2026-10-28: honour the licence for derived data)

#### Scenario: Notice and width
- **WHEN** a report is written from a synthetic run result
- **THEN** the notice follows the title, every table has at most four columns, and each landform section names three descriptors
