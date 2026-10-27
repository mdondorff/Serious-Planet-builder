## MODIFIED Requirements

### Requirement: LOOK-001 Look file schema, validation and canonical text
The look of a body SHALL be presentation data (ADR 0035) held in a TOML look file with a top-level `schema_version`, the marker `kind = "look"`, and tables `[body.<id>.units]`, `[body.<id>.exposure]`, `[body.<id>.histogram]`, `[body.<id>.tone_map]` and, since schema version 2, `[body.<id>.sun]` and `[body.<id>.albedo]` since schema version 3, `[body.<id>.atmosphere]` since schema version 4, `[body.<id>.water]` and `[body.<id>.ground]` (LOOK-011), addressed by body id as the world definition is (`body.<n>`, a non-negative integer without leading zeros). Schema, parsing, validation and the canonical writer SHALL live in a GPU-free crate (`planet_frame::look::file`, CON-03). Every table and key SHALL be optional and a missing one SHALL be the Earth default. Values SHALL be finite floats (an integer literal exact as a float is accepted); an unknown table or key, a value of the wrong type, a body id that is not canonical, an unknown tone curve and a value outside its range (including an array of the wrong length and a pair of thresholds out of order) SHALL be errors that name the full key (`body.2.exposure.min_ev100`) and the value. Writing SHALL be canonical (version and kind first, bodies in id order, every key written, floats in the shortest round-trip form) and reading it back SHALL give an equal file. Each attribute is presentation data, never read back by a CPU system (other than the CPU reference and the plan builders), never stored or keyed as world data and never an input to a field; its declared bound is the valid range below (a value outside it is refused, so a shader never sees it) and the precision with which the value reaches the GPU:

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
| `atmosphere.rayleigh_scattering`, `mie_scattering`, `mie_absorption`, `ozone_absorption` (RGB, per metre at sea level and at the ozone peak) | presentation | (5.802e-6, 13.558e-6, 33.1e-6), (3.996e-6) x 3, (0.444e-6) x 3, (0.650e-6, 1.881e-6, 0.085e-6): Hillaire 2020, Earth | every channel 0 to 1e-3 | rounded once to f32; the GPU passes match the f64 reference of ATMO-001 on these f32 values |
| `atmosphere.rayleigh_scale_height_m`, `mie_scale_height_m` | presentation | 8000, 1200 | 1 to 1e6 | rounded once to f32 |
| `atmosphere.mie_g` (Cornette-Shanks asymmetry) | presentation | 0.8 | -0.99 to 0.99 | rounded once to f32 |
| `atmosphere.ozone_center_m`, `ozone_half_width_m` (the tent profile of the ozone layer) | presentation | 25000, 15000 | 0 to 1e6; 1 to 1e6 | rounded once to f32 |
| `atmosphere.top_height_m` (the top of the atmosphere above the reference surface) | presentation | 100000 | 1e3 to 1e6; also below the body radius, which the plan builder checks (the radius is world data, never a look key) | rounded once to f32 |
| `atmosphere.ground_albedo` (RGB; what the sky bounces off in the multiple-scattering table and below the horizon) | presentation | (0.3, 0.3, 0.3) | every channel 0 to 1 | rounded once to f32 |
| `atmosphere.sun_angular_radius_rad` (the sun's disc, also the width of the terminator's penumbra) | presentation | 0.004675 (0.5357 degrees across) | 1e-5 to 0.5 | rounded once to f32 |
| `water.absorption_per_m` (scale of the extinction per metre of a liquid, `-ln(max(colour, 1e-3)) / -ln(1e-3)` per channel) | presentation | 0.5 | 1e-3 to 100 | rounded once to f32; the surface radiance matches the f64 reference within the bound of ATMO-014 |
| `water.fresnel_f0` (reflectance at normal incidence), `water.glint_roughness` (GGX roughness of the sun glint) | presentation | 0.02, 0.08 | 0 to 0.2; 0.01 to 1 | as above |
| `water.shore_fade_m` (depth over which the surface layer fades in), `water.foam_width_m` (depth of the foam band), `water.foam_strength` (share of foam at its densest) | presentation | 0.05, 0.4, 0.8 | 1e-3 to 10; 1e-2 to 100; 0 to 1 | as above (the shader divides by the fade and the width) |
| `water.shore_filter` (strength of the `fwidth` filter of the shore bands: the cover and the foam are stretched by this multiple of the pixel's change of depth; 0 is off) | presentation | 0.0 | 0 to 4 | as above |
| `water.choppy` (horizontal displacement of a crest relative to its height), `water.gravity_m_s2` (deep-water dispersion), `water.wave_scale` (multiplies the wave height of every liquid; 0 switches the waves off) | presentation | 0.8, 9.81, 1.0 | 0 to 2; 0.1 to 100; 0 to 10 | wave displacement within 1 mm + 1e-3 x the sum of the amplitudes (ATMO-014) |

Verify: A
Status: active
Source: ADR 0042 point 6, ADR 0048 point 2, ADR 0041 points 4 to 6, ADR 0035, CON-03, CON-10, owner (M5 gate, 2026-10-18: `water.shore_filter` default 0)

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
The look file SHALL have its own schema version (5 since `ground`; 4 `water`, ADR 0041 points 4 to 6; 3 added the `atmosphere` table, ADR 0040; 2 added the `sun` and `albedo` tables, ADR 0039; 1 was the first layout), independent of the world definition's and of any generator version, and tested migrations in the manner of WDEF-002 (CON-10). **Every change that adds a table or a key to the look file SHALL raise the schema version and add an identity migration step**, so that a build that does not know the table says "newer schema version N, update the program" and not "unknown key" (the clearer failure under CON-10); moving or renaming a key needs a real step as well. Loading SHALL require the version, SHALL refuse a newer version with a message that gives both versions and says to update the program, SHALL refuse a gap in the migration chain naming the missing step, SHALL apply the migrations from the file's version to the current one in order, and SHALL fail with the step's message when a migration fails. The layout read after the chain SHALL be the current one, and a migrated file SHALL equal the file of the current layout with the same content and write the same canonical text. A table that no version of this build knows, in a file of the current version, is an unknown key naming the table (a typo, not a release). A look file SHALL NOT load as a world definition and a definition SHALL NOT load as a look file (the `kind` marker). The production chain SHALL have one step per older version (`MIGRATIONS.len() == SCHEMA_VERSION - 1`, a test pins it) and a fixture of every older version SHALL load through it.

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
- **WHEN** the version 1 fixture is loaded by the production chain (the identity steps 1 to 2 and 2 to 3) and written back, and the same content is read as a version 3 file
- **THEN** it reads as written, the tables versions 2 and 3 added are the Earth default, the written text reads back equal and equals the version 3 file's text

#### Scenario: Version 2 fixture
- **WHEN** the version 2 fixture with a partial `albedo` table and a `sun` table is loaded through the identity step 2 to 3
- **THEN** the keys it names are read, the keys it leaves out and the `atmosphere` table are the Earth default and the canonical text reads back equal

#### Scenario: Version 3 fixture
- **WHEN** the version 3 fixture with a partial `atmosphere` table (integer literals exact as floats among the numbers) is loaded
- **THEN** the keys it names are read, the keys it leaves out are the Earth default, the canonical text reads back equal, and a build that knows only version 2 refuses the file as a newer schema while a misspelt table or key in a version 3 file is an unknown key naming it

#### Scenario: Version 4 fixture
- **WHEN** the version 4 fixture with a partial `water` table is loaded
- **THEN** the keys it names are read, the keys it leaves out are the Earth default, the canonical text reads back equal, a version 3 file with the same content reads equal (the identity step 3 to 4), a build that knows only version 3 refuses the file as a newer schema, a misspelt table or key is an unknown key naming it, and a water value outside its range is refused naming the key and the value

#### Scenario: Version 5 fixture
- **WHEN** the version 5 fixture with a partial `ground` table is loaded
- **THEN** the keys it names are read, the curves and materials it leaves out are the Earth default, the canonical text reads back equal, a version 4 file with the same content reads equal (the identity step 4 to 5) and a build that knows only version 4 refuses the file as a newer schema

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build (for example 5), has no migration path, or a migration step fails
- **THEN** loading fails with an error that names the versions or the step and what to do

#### Scenario: Not interchangeable with a definition
- **WHEN** a look file is given to the definition loader and a definition to the look loader
- **THEN** both fail and the look file's error names the missing `kind`

## ADDED Requirements

### Requirement: LOOK-011 The ground look in the look file
The look file SHALL hold the ground look of a body in an optional table `[body.<id>.ground]` (schema version 5): `detail`, the strength of the ground detail (LOOK-009), and per material (`rock`, `sand`, `grass`, `soil`) a sub-table with the `(lo, hi)` pair of each input of LOOK-008 as two numbers (`slope_cos = [0.86, 0.62]`). A missing key SHALL be the default: detail `NATURAL_STRENGTH` and the Earth curves. A value SHALL be refused naming its key and value as LOOK-008 and LOOK-009 refuse it; an unknown material or input SHALL be an unknown key. The canonical text SHALL write every key. Presentation data (ADR 0035).

Verify: A
Status: active
Source: opus-planner (2026-10-28; owner: compare the looks live), LOOK-008, LOOK-009, CON-10

#### Scenario: A ground table
- **WHEN** a look file gives soil a moisture curve and a detail of 0.6, and another gives a detail of 1.5 or a curve with one value
- **THEN** the first reads with the Earth curves for everything else and writes back equal; the others are refused naming `ground.detail` and `ground.<material>.<input>` with the value

#### Scenario: No table
- **WHEN** a look file has no `ground` table
- **THEN** its ground look is the natural detail and the Earth curves, which is what the window shows without the file
