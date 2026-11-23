## ADDED Requirements

### Requirement: VIEW-073 Camera coordinates readout, jump parser, and saved site bookmarks
`planet_editor::bookmarks` SHALL compute a roll-free `CameraReadout` (latitude, longitude, ground clearance altitude, local compass bearing clockwise from north, and pitch) from a `FreePose` or `Camera` using `libm` and `tangent_frame` without pole singularities. `BookmarkShelf` SHALL store named bookmarks (`id`, trimmed name up to `MAX_NAME_CHARS`, coordinates, altitude, bearing, pitch, optional notes) in deterministic `version = 1` TOML outside any `WorldDefinition` or item store, load missing files as empty, refuse malformed or out-of-range files naming the field, write atomically, and reconstruct exact `FreePose` targets above ground. `parse_jump_target` SHALL accept `LAT,LON[,ALT]` (with optional `--site`/`--marker` prefixes, degree symbols, `N`/`S`/`E`/`W` suffixes, and metric altitude) and refuse ambiguous inputs.

Verify: A
Status: active
Source: companion backlog 1C (2026-11-22), VIEW-004, VIEW-017, AUTH-001

#### Scenario: Camera readout and bookmark round-trip
- **WHEN** camera poses at poles, equator, prime meridian, antimeridian, and mid-latitude ground sites are saved to `BookmarkShelf`, serialized to TOML, parsed back, and reconstructed as `FreePose`
- **THEN** each restored position matches within 1 mm and heading and pitch match within 1e-9

#### Scenario: Jump target parsing and persistence refusals
- **WHEN** decimal, `--site`, `--marker`, and hemisphere strings are parsed and invalid coordinates or malformed TOML files are loaded
- **THEN** valid spellings normalize into canonical range and invalid inputs or unknown TOML keys are refused naming the problem

### Requirement: VIEW-074 Directory scanner and metadata shelf for F9 repro captures
`planet_editor::repro_shelf` SHALL scan a repro directory (`PLANET_REPRO_DIR` or `target/review`) without GPU access, returning an empty shelf when the directory does not exist and collecting up to `MAX_REPRO_ENTRIES` `.bundle` files sorted newest `frame-<n>` first then reverse-lexicographically. Each valid `TerrainBundle` (`planet-terrain-repro` v1/v2) or `ReproBundle` (`planet-repro` v1) SHALL expose its geographic `CameraReadout`, exact `free_pose()`, `to_bookmark()`, sibling `<stem>-path.txt` sample count and `<stem>.png` presence, `ReproCompatibility` against an active seed and `definition_hash`, and `replay_command()` matching `cargo xtask repro` (`--window` over `<stem>-path.txt` when `open_commit` is recorded). Unparseable bundles SHALL be isolated in `broken` with their error without failing the scan.

Verify: A
Status: active
Source: companion backlog 1C (2026-11-22), VIEW-024, VIEW-071, BUILD-054

#### Scenario: Repro directory scan and corrupt isolation
- **WHEN** a directory holding valid `TerrainBundle`, `ReproBundle`, sibling path/PNG files, and a corrupt bundle is scanned
- **THEN** valid captures sort newest first with exact `free_pose()`, `replay_command()`, compatibility states, and bookmark conversion, while the corrupt bundle lands in `broken` without failing the scan
