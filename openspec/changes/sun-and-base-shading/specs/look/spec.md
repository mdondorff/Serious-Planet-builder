## MODIFIED Requirements

### Requirement: LOOK-001 Look file schema, validation and canonical text
The look of a body SHALL be presentation data (ADR 0035) held in a TOML look file with a top-level `schema_version`, the marker `kind = "look"`, and tables `[body.<id>.units]`, `[body.<id>.exposure]`, `[body.<id>.histogram]`, `[body.<id>.tone_map]` and, since schema version 2, `[body.<id>.sun]` and `[body.<id>.albedo]`, addressed by body id as the world definition is (`body.<n>`, a non-negative integer without leading zeros). Schema, parsing, validation and the canonical writer SHALL live in a GPU-free crate (`planet_frame::look::file`, CON-03). Every table and key SHALL be optional and a missing one SHALL be the Earth default. Values SHALL be finite floats (an integer literal exact as a float is accepted); an unknown table or key, a value of the wrong type, a body id that is not canonical, an unknown tone curve and a value outside its range (including an array of the wrong length and a pair of thresholds out of order) SHALL be errors that name the full key (`body.2.exposure.min_ev100`) and the value. Writing SHALL be canonical (version and kind first, bodies in id order, every key written, floats in the shortest round-trip form) and reading it back SHALL give an equal file bit for bit. Each attribute is presentation data, never read back by a CPU system (other than the CPU reference and the plan builders), never stored or keyed as world data and never an input to a field; its declared bound is the valid range below (a value outside it is refused, so a shader never sees it) and the precision with which the value reaches the GPU:

| Attribute (key under `body.<id>`) | Class | Earth default | Valid range | Precision bound |
|---|---|---|---|---|
| `units.solar_illuminance_lux` (the lux of one scene unit) | presentation | 126600.0 | 1 to 1e7 | decided in f64, the pre-exposure factor rounded once to f32 (relative 6e-8) |
| `exposure.min_ev100`, `exposure.max_ev100` | presentation | -2.0, 16.0 | -20 to 40, min <= max, and, as a conservative proxy for the pre-exposure the view chooses, one unit written at max_ev100 stays a normal f16 (lux / (1.2 x 2^max) >= 2^-14) | rounded once to f32; the adapted EV matches the CPU step within 1e-4 EV (REND-022) |
| `exposure.speed_up`, `exposure.speed_down` (stops per second) | presentation | 3.0, 1.0 | 0 to 100 | rounded once to f32 |
| `histogram.ev_min`, `histogram.ev_max` | presentation | -10.0, 20.0 | -50 to 50, at least 1 stop apart | 64 bins over the axis; bin counts equal the CPU reference away from bin edges (REND-022) |
| `histogram.band_low`, `histogram.band_high` | presentation | 0.4, 0.9 | 0 to 1, high - low >= 0.01 | rounded once to f32; target within 1e-4 EV of the CPU (REND-022) |
| `tone_map.curve` | presentation | `"aces"` | `"aces"` | within 1/255 per channel of the CPU reference (REND-023) |
| `sun.direction` (unit vector towards the sun in the planet-fixed axes, an array of three numbers) | presentation | (0.5014059, 0.6016871, 0.6217433) widened from f32, the sun of the first lit views | finite, length 1 within 1e-6 | decided in f64, rounded once to f32 in the plan (6e-8) |
| `albedo.rock`, `snow`, `grass`, `forest`, `desert`, `tundra`, `beach`, `seabed` (linear RGB, arrays of three numbers) | presentation | (0.30, 0.27, 0.24), (0.85, 0.87, 0.90), (0.14, 0.19, 0.06), (0.045, 0.075, 0.03), (0.42, 0.33, 0.22), (0.20, 0.19, 0.15), (0.52, 0.46, 0.34), (0.06, 0.08, 0.09): published ranges of natural surfaces (Oke, Boundary Layer Climates, 1987) | every channel 0 to 1 | rounded once to f32; the GPU albedo equals the f64 reference within the bound of REND-025 |
| `albedo.dry_mm`, `wet_mm` (desert to vegetation), `forest_mm_lo`, `forest_mm_hi` (grass to forest) | presentation | 250, 900, 1000, 2200 | 0 to 20,000, each pair ordered lo < hi | as above |
| `albedo.tundra_c_lo`, `tundra_c_hi` (tundra to vegetation), `snow_c_full`, `snow_c_none` (snow by temperature) | presentation | -3, 5, -8, -1 | -100 to 100, each pair ordered | as above |
| `albedo.rock_cos_full`, `rock_cos_none`, `snow_cos_none`, `snow_cos_full` (cosines of the slope angle) | presentation | 0.62, 0.86, 0.55, 0.80 | 0 to 1, each pair ordered | as above |
| `albedo.beach_height_m`, `beach_fade_m`, `shore_blend_m` (heights above the water level W) | presentation | 3, 5, 1 | 0 to 1000; the fade and the blend at least 1e-3 (the shader divides by them) | as above |

Verify: A
Status: active
Source: ADR 0042 point 6, ADR 0048 point 2, ADR 0035, CON-03, CON-10

#### Scenario: Complete and partial entries
- **WHEN** a file writes body 0 in full (with `min_ev100 = -2` as an integer literal) and body 3 with two overridden tables
- **THEN** body 0 equals the Earth default, body 3 has its overrides and the defaults for everything it leaves out

#### Scenario: Canonical text
- **WHEN** a file with awkward floats is written, read and written again
- **THEN** the files are equal bit for bit and the texts equal, with bodies in id order

#### Scenario: Refusals name the key and the value
- **WHEN** a value is out of range, not finite, of the wrong type or unknown, or a body id is `earth`, `-1`, `01` or too large
- **THEN** loading fails with a message that contains the full key and the value as written

### Requirement: LOOK-002 Look file migrations
The look file SHALL have its own schema version (2 since the `sun` and `albedo` tables, ADR 0039; 1 was the first layout), independent of the world definition's and of any generator version, and tested migrations in the manner of WDEF-002 (CON-10). **Every change that adds a table or a key to the look file SHALL raise the schema version and add an identity migration step**, so that a build that does not know the table says "newer schema version N, update the program" and not "unknown key" (the clearer failure under CON-10); moving or renaming a key needs a real step as well. Loading SHALL require the version, SHALL refuse a newer version with a message that gives both versions and says to update the program, SHALL refuse a gap in the migration chain naming the missing step, SHALL apply the migrations from the file's version to the current one in order, and SHALL fail with the step's message when a migration fails. The layout read after the chain SHALL be the current one, and a migrated file SHALL equal the file of the current layout with the same content and write the same canonical text. A table that no version of this build knows, in a file of the current version, is an unknown key naming the table (a typo, not a release). A look file SHALL NOT load as a world definition and a definition SHALL NOT load as a look file (the `kind` marker). The production chain SHALL have one step per older version (`MIGRATIONS.len() == SCHEMA_VERSION - 1`, a test pins it) and a fixture of every older version SHALL load through it.

Verify: A
Status: active
Source: CON-10, ADR 0042 point 6, WDEF-002, ADR 0039

#### Scenario: Older layouts
- **WHEN** files of an older synthetic layout (versions 1 and 2 of a three-version chain, for the engine's own steps) and the current layout of the same content are loaded
- **THEN** all three equal each other and write the same canonical text

#### Scenario: Future tables
- **WHEN** a build adds an optional table and an older build reads a file of the next version that has it
- **THEN** the older build refuses it as a newer schema naming both versions, and the same table in a file that claims the current version is an unknown key naming the table

#### Scenario: Version 1 fixture
- **WHEN** the version 1 fixture is loaded by the production chain (the identity step 1 to 2) and written back, and the same content is read as a version 2 file
- **THEN** it reads as written, the tables version 2 added are the Earth default, the written text reads back equal and equals the version 2 file's text

#### Scenario: Version 2 fixture
- **WHEN** the version 2 fixture with a partial `albedo` table and a `sun` table is loaded
- **THEN** the keys it names are read, the keys it leaves out are the Earth default and the canonical text reads back equal

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build (for example 3), has no migration path, or a migration step fails
- **THEN** loading fails with an error that names the versions or the step and what to do

#### Scenario: Not interchangeable with a definition
- **WHEN** a look file is given to the definition loader and a definition to the look loader
- **THEN** both fail and the look file's error names the missing `kind`

### Requirement: LOOK-003 Earth defaults and the plan
The Earth default look SHALL be physically based and SHALL equal the values that were code constants before the look file existed: a scene unit of 126,600 lux, exposure range -2 to 16 EV100 with speeds 3 and 1 stops per second, the histogram over EV -10 to +20 with the band 40 % to 90 %, the ACES curve, the albedo table and the sun direction of LOOK-001 (the sun of the first lit views, the albedo ranges of natural surfaces). A body without an entry SHALL have the default as a whole. A lit plan built without a look (`LitPlan::fixed`, `LitPlan::adaptive`) SHALL equal the plan built from the default look and SHALL keep its text form and hash: the look enters a plan's text only when it differs from the default, as exact bit patterns (the albedo table as 39 f64 bit patterns after the units), and the text round-trips; the sun direction is always in the plan's text, and a plan's sun is the look's sun rounded once to f32. A plan SHALL be refused, naming the key and the value, when its scene unit or histogram is outside the ranges of LOOK-001 or its pre-exposure factor does not fit an f32, or its albedo table or water level is outside the ranges of LOOK-001. The CPU references (`pre_exposure_factor`, the histogram, band mean and target) SHALL take the look as parameters, and the functions without them SHALL be the Earth default. Class: presentation data (ADR 0035).

Verify: A
Status: active
Source: ADR 0048 points 2 and 3, ADR 0038, ADR 0042 points 3 and 6, ADR 0035

#### Scenario: Defaults equal the old constants
- **WHEN** the default look is read
- **THEN** its numbers are 126600, -2, 16, 3, 1, -10, 20, 0.4, 0.9 and `aces`, and a body missing from a file has them

#### Scenario: Plan text unchanged
- **WHEN** a default lit plan is written to text and a text from before the look file is read
- **THEN** the text has no look tokens and the old text reads as the default look

#### Scenario: A body's look
- **WHEN** a plan is made from a body whose scene unit is 25,000 lux and whose exposure maximum is 10.5
- **THEN** the plan carries them, its text and hash differ from the default plan's and round-trip, and the pre-exposure factor scales with the unit

## ADDED Requirements

### Requirement: LOOK-007 The albedo table, the sun and the water level reach the shader
The renderer SHALL take the albedo table from the plan (`AlbedoParams::to_words`, ten `vec4<f32>` appended to the globals), the sun direction from the plan's sun and the water level W from the plan (`water_level_m`, 0 until the water-level field of ADR 0041 exists), and SHALL hold no albedo colour, threshold or sea level as a constant in a shader. Class: presentation data. Bound: with a table whose every colour is the same grey the albedo view is that grey wherever there is terrain, whatever the climate, the slope and the height (to within f16 rounding, 2e-3 relative); with W far above the terrain every pixel is the seabed colour and with W far below it none is; and the lit view equals the albedo view times the lighting view (2e-3 relative).

Verify: C
Status: active
Source: ADR 0039 points 1 and 3, ADR 0041, ADR 0042 point 6, ADR 0035

#### Scenario: Flat table
- **WHEN** the albedo view of a terrain frame is drawn with every colour of the table equal to 0.5, in both tile formats
- **THEN** every covered pixel is 0.5 / pi of a unit in the scene target

#### Scenario: Water level
- **WHEN** the water level of the plan is 100 km above and 100 km below the terrain
- **THEN** every pixel is the seabed colour in the first frame and none is in the second

#### Scenario: Lit is albedo times lighting
- **WHEN** the lit, albedo and lighting views of one plan are drawn
- **THEN** every lit pixel equals the albedo pixel times the lighting pixel over a lit unit

