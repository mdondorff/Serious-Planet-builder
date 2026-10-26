# look Specification

## Purpose
The look file (ADR 0042 point 6, ADR 0048 point 2): presentation parameters per body in a schema-versioned TOML file beside the world definition and outside its hash and every cache key, with tested migrations (CON-10), physically based Earth defaults, validation that names the key and the value, and loading by the viewer and the test harness. GPU-free (CON-03); the class and bound of each attribute are stated in LOOK-001.

## Requirements

### Requirement: LOOK-001 Look file schema, validation and canonical text
The look of a body SHALL be presentation data (ADR 0035) held in a TOML look file with a top-level `schema_version`, the marker `kind = "look"`, and tables `[body.<id>.units]`, `[body.<id>.exposure]`, `[body.<id>.histogram]`, `[body.<id>.tone_map]` and, since schema version 2, `[body.<id>.sun]` and `[body.<id>.albedo]` since schema version 3, `[body.<id>.atmosphere]` and, since schema version 4, `[body.<id>.water]`, addressed by body id as the world definition is (`body.<n>`, a non-negative integer without leading zeros). Schema, parsing, validation and the canonical writer SHALL live in a GPU-free crate (`planet_frame::look::file`, CON-03). Every table and key SHALL be optional and a missing one SHALL be the Earth default. Values SHALL be finite floats (an integer literal exact as a float is accepted); an unknown table or key, a value of the wrong type, a body id that is not canonical, an unknown tone curve and a value outside its range (including an array of the wrong length and a pair of thresholds out of order) SHALL be errors that name the full key (`body.2.exposure.min_ev100`) and the value. Writing SHALL be canonical (version and kind first, bodies in id order, every key written, floats in the shortest round-trip form) and reading it back SHALL give an equal file bit for bit. Each attribute is presentation data, never read back by a CPU system (other than the CPU reference and the plan builders), never stored or keyed as world data and never an input to a field; its declared bound is the valid range below (a value outside it is refused, so a shader never sees it) and the precision with which the value reaches the GPU:

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
The look file SHALL have its own schema version (4 since the `water` table, ADR 0041 points 4 to 6; 3 added the `atmosphere` table, ADR 0040; 2 added the `sun` and `albedo` tables, ADR 0039; 1 was the first layout), independent of the world definition's and of any generator version, and tested migrations in the manner of WDEF-002 (CON-10). **Every change that adds a table or a key to the look file SHALL raise the schema version and add an identity migration step**, so that a build that does not know the table says "newer schema version N, update the program" and not "unknown key" (the clearer failure under CON-10); moving or renaming a key needs a real step as well. Loading SHALL require the version, SHALL refuse a newer version with a message that gives both versions and says to update the program, SHALL refuse a gap in the migration chain naming the missing step, SHALL apply the migrations from the file's version to the current one in order, and SHALL fail with the step's message when a migration fails. The layout read after the chain SHALL be the current one, and a migrated file SHALL equal the file of the current layout with the same content and write the same canonical text. A table that no version of this build knows, in a file of the current version, is an unknown key naming the table (a typo, not a release). A look file SHALL NOT load as a world definition and a definition SHALL NOT load as a look file (the `kind` marker). The production chain SHALL have one step per older version (`MIGRATIONS.len() == SCHEMA_VERSION - 1`, a test pins it) and a fixture of every older version SHALL load through it.

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

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build (for example 5), has no migration path, or a migration step fails
- **THEN** loading fails with an error that names the versions or the step and what to do

#### Scenario: Not interchangeable with a definition
- **WHEN** a look file is given to the definition loader and a definition to the look loader
- **THEN** both fail and the look file's error names the missing `kind`

### Requirement: LOOK-003 Earth defaults and the plan
The Earth default look SHALL be physically based and SHALL equal the values that were code constants before the look file existed: a scene unit of 126,600 lux, exposure range -2 to 16 EV100 with speeds 3 and 1 stops per second, the histogram over EV -10 to +20 with the band 40 % to 90 %, the ACES curve, the albedo table and the sun direction of LOOK-001 (the sun of the first lit views, the albedo ranges of natural surfaces). A body without an entry SHALL have the default as a whole. A lit plan built without a look (`LitPlan::fixed`, `LitPlan::adaptive`) SHALL equal the plan built from the default look and SHALL keep its text form and hash: the look enters a plan's text only when it differs from the default, as exact bit patterns (the albedo table as 39 f64 bit patterns after the units), and the text round-trips; a plan's atmosphere (`LitPlan::with_atmosphere`: the look's coefficients on the world definition's body radius, which a plan builder passes in and which is never a look key) enters the text only when present, as `atmosphere`, its view and 23 exact f64 bit patterns after the `view` token, and a plan without one keeps its text and hash; the sun direction is always in the plan's text, and a plan's sun is the look's sun rounded once to f32. A plan SHALL be refused, naming the key and the value, when its scene unit or histogram is outside the ranges of LOOK-001 or its pre-exposure factor does not fit an f32, or its albedo table is outside the ranges of LOOK-001 (also after rounding to f32: a pair of thresholds that collapses is refused) or its water level is not finite or its atmosphere is outside its ranges (also the radius of the body, and a top that is not below it), and a plan's sun SHALL be a unit vector within 1e-3 (the look's sun within 1e-6). The CPU references (`pre_exposure_factor`, the histogram, band mean and target) SHALL take the look as parameters, and the functions without them SHALL be the Earth default. Class: presentation data (ADR 0035).

Verify: A
Status: active
Source: ADR 0048 points 2 and 3, ADR 0038, ADR 0042 points 3 and 6, ADR 0035

#### Scenario: Defaults equal the old constants
- **WHEN** the default look is read
- **THEN** its numbers are 126600, -2, 16, 3, 1, -10, 20, 0.4, 0.9 and `aces`, and a body missing from a file has them

#### Scenario: Plan text unchanged
- **WHEN** a default lit plan is written to text and a text from before the look file is read
- **THEN** the text has no look tokens and the old text reads as the default look

#### Scenario: An atmosphere in the plan
- **WHEN** a plan is made with the Earth atmosphere on a body of radius 6,371 km, written to text and read back
- **THEN** the text has the atmosphere tokens after the view, reads back equal, differs in hash from the plan without one, and a radius of 500 m or a top at or above the radius is refused naming the value

#### Scenario: A body's look
- **WHEN** a plan is made from a body whose scene unit is 25,000 lux and whose exposure maximum is 10.5
- **THEN** the plan carries them, its text and hash differ from the default plan's and round-trip, and the pre-exposure factor scales with the unit

### Requirement: LOOK-004 Outside the definition hash and every cache key
The look file SHALL be outside the world definition's hash and every cache key (CON-05, CON-09): loading a look file, changing its values or its bodies, or keeping any number of looks alive SHALL change no definition hash, no base hash, no definition text, no read set, no node-set id, no store parameter and no persistent tile key, so a look change re-keys nothing and invalidates no cache or authored content. Nothing a generator reads SHALL be look data (kind ids, liquid kinds and thresholds that shape the world stay in the world definition).

Verify: A
Status: active
Source: ADR 0042 point 6, CON-05, CON-09

#### Scenario: Fingerprint
- **WHEN** a definition's hash, base hash, canonical text, node-set id, store parameters, height keys of two tiles at two resolutions and read set are taken before and after a default look and a look with every value changed for bodies 0, 1 and 7 are loaded
- **THEN** every part is equal, the definition hash equals its recorded known answer, and a control change to a definition node changes the hash and the keys

### Requirement: LOOK-005 Loading in the application
`planet editor` and `planet test-render` SHALL load the look before any world is built or device opened: the file named by `--look`, else `<stem>.look.toml` beside the `--world` file when it exists, else the Earth defaults for every body. An explicit file that cannot be read, and any file that is bad, SHALL stop the run with the path and the error of LOOK-001 or LOOK-002; a run that uses a file SHALL print one line naming it and its bodies, and a run that uses none SHALL print nothing more than before. `generate` SHALL refuse `--look` (the look never changes a generated tile). The loaded look SHALL answer per body (`LookSet::params`), with the Earth default for a body without an entry.

Verify: A
Status: active
Source: ADR 0042 point 6, BUILD-004

#### Scenario: Resolution order
- **WHEN** a world file has a look file beside it and `--look` names another
- **THEN** the explicit file is used; without `--look` the one beside the world is; with neither the defaults are

#### Scenario: Bad file
- **WHEN** the file has a future version, an out-of-range value, or does not exist
- **THEN** the run fails before any other work with the path, and the key and value or the versions

#### Scenario: Default runs unchanged
- **WHEN** `editor --smoke --no-window` runs without a look
- **THEN** its output has no look line

### Requirement: LOOK-006 The look reaches the passes
The renderer SHALL take the histogram axis and band from the plan (the plan's `HistogramParams`, rounded once from f64 to f32 by the CPU) and the scene-unit scale from the plan's `SceneUnits` (the pre-exposure factor decided in f64 and rounded once to f32), and SHALL hold no look constant in a shader beyond the structure of the histogram (64 bins). With the Earth defaults every number a shader receives SHALL be the one it used before, so every existing image, histogram and digest is unchanged. Class: presentation data. Bound: with a look unlike the default, the GPU histogram equals the CPU histogram under that axis bin for bin (REND-022's bound) and the exposure target equals the CPU target under that band and range within 1e-4 EV, and a scene unit scaled by a factor scales every written scene value by it within the f16 rounding of the target.

Verify: B
Status: active
Source: ADR 0042 point 6, ADR 0048 points 2-4, ADR 0035, CON-03

#### Scenario: Histogram and band
- **WHEN** a scene with pixels at the bin centres of the axis -8 to +24 is rendered with the band 25 % to 75 % and a range of -6 to 20
- **THEN** the GPU bins equal the CPU's under that axis and the target equals the CPU's, which differs from what the Earth band would give

#### Scenario: Scene unit
- **WHEN** a terrain frame is rendered with the default unit and with a quarter of it
- **THEN** every written value is a quarter, the same pixels are covered, and the default look writes exactly the old image

### Requirement: LOOK-007 The albedo table, the sun and the water level reach the shader
The renderer SHALL take the albedo table from the plan (`AlbedoParams::to_words`, ten `vec4<f32>` appended to the globals), the sun direction from the plan's sun and the water level W from the plan (`water_level_m`: the viewer sets it from the sea of the definition, the sea of the definition, which is M5's only producer of W, GEN-023, through `LitPlan::with_water_level`; a later change reads `WaterField::plan_water` per view; 0 for the Earth default, which leaves the plan text and hash as they were), and SHALL hold no albedo colour, threshold or sea level as a constant in a shader. Class: presentation data. Bound: with a table whose every colour is the same grey the albedo view is that grey wherever there is terrain, whatever the climate, the slope and the height (to within f16 rounding, 2e-3 relative); with W far above the terrain every pixel is the seabed colour and with W far below it none is; and the lit view equals the albedo view times the lighting view (2e-3 relative).

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

### Requirement: LOOK-008 Ground material weights
The weights of the ground materials (rock, sand, grass, soil; the detail of the ground at the lowest levels, M7 G2) SHALL be one function, `MaterialParams::weights`, of a fixed vector of eight inputs in a fixed order (the slope cosine, the height above the water, the temperature, the precipitation, the region's surface type, and three slots reserved for driver maps: soil depth, moisture, insolation), with the response curves per material as look data and no material as a branch of code. A material's weight SHALL be the product over the inputs of the smoothstep response of a `(lo, hi)` pair (0 at `lo`, 1 at `hi`, falling when `lo > hi`; a pair with `lo == hi` leaves the input out; a material whose pairs are all equal is off), the weights SHALL be normalised to sum to 1 when any is positive and all be 0 otherwise, and they SHALL be monotone in each input a curve uses. The default curves SHALL use the thresholds of the base albedo (`AlbedoParams::EARTH`), so that the detail and the colour agree on what is rock, desert and vegetation. A driver map SHALL be a new value in a reserved slot and new curves in the look, with no change of the shader's interface (the input vector and the curve table). The curves SHALL be part of the lit plan, written in its text only when they are not the default (so a plan made with the defaults keeps its text and hash), and the table SHALL reach the shader as `MATERIAL_VEC4S` words of `f32`, each value rounded once. Presentation data (ADR 0035): nothing is stored, keyed or read back.

Verify: A
Status: active
Source: M7 G2 (opus-planner, 2026-10-21), SPIKE-27, ADR 0039 point 3, ADR 0035

#### Scenario: The default
- **WHEN** the weights are evaluated on steep ground, on flat dry ground and on flat wet ground with the default curves
- **THEN** rock, sand and grass are the material of each, the weights sum to 1 everywhere, and soil is off

#### Scenario: A driver slot
- **WHEN** a look gives soil a curve on the reserved soil-depth input
- **THEN** soil takes weight where the depth is, with the same function and the same input vector

### Requirement: LOOK-009 The strength of the ground detail
The lit plan SHALL carry the strength of the procedural ground detail (REND-034) as a number from 0 to 1, default 0 (off). The strength SHALL be written in the plan's text only when it is not 0 (so a plan made with the default keeps its text and hash), SHALL round-trip exactly, and SHALL be refused unless it is 0 or from 1e-6 to 1 (a smaller one differs between adapters that flush subnormals), naming the key. Presentation data (ADR 0035).

Verify: A
Status: active
Source: M7 G2a (opus-planner, 2026-10-21), SPIKE-27, ADR 0035

#### Scenario: The default and a set strength
- **WHEN** a lit plan has the default strength, and one has the strength 0.5
- **THEN** the first writes no `detail` token and parses back equal; the second writes one, parses back equal and writes the same text again; a strength of 1.5 or NaN is refused naming `ground_detail`

### Requirement: LOOK-010 The gloss of the ground
The lit plan SHALL carry the gloss table of the ground (REND-035) as look data: a roughness for each ground material (rock, sand, grass, soil; a number above 0 and up to 1), two roughness modifiers (the wet and the icy ground), each an input of the weights' vector, a `(lo, hi)` smoothstep pair on it and a factor above 0 and up to 1 that multiplies the roughness at response 1 (a pair with `lo == hi` leaves the modifier out), and the base reflectance `f0` from 0 to 1. The roughness of a ground point SHALL be the weights' mean of the materials' roughness (the weights of LOOK-008) times the modifiers, and none where no material has weight. The default SHALL be rock 0.85, sand 0.65, grass 0.75, soil 0.80, the wet modifier on the reserved moisture input (out in the default look (GEN-073)), the icy modifier on the temperature falling from 0 to -5 C with the factor 0.4, and `f0` 0.04. The table SHALL be written in the plan's text only when it is not the default (so a plan made with the defaults keeps its text and hash), SHALL round-trip exactly, and SHALL be refused naming the key when a value is not finite, a roughness or factor is not above 0 and up to 1, a modifier's input is not one of the inputs, `f0` is outside 0 to 1, or a pair differs only below `f32` (the shader decides on the `f32` words). Presentation data (ADR 0035).

Verify: A
Status: active
Source: M7 G2b (opus-planner, 2026-10-21; owner: roughness and shine are in M7), LOOK-008, ADR 0035

#### Scenario: The table and the roughness
- **WHEN** a lit plan has the default gloss and one with another rock roughness and `f0`, each is written as text and parsed back, and values outside the limits are given
- **THEN** the first writes no `gloss` token, the second writes one and round-trips, each bad value is refused naming its key, and the roughness of dry flat ground is that of sand, of icy ground the sand's times 0.4, with the wet modifier out by default and in when a look gives it a pair, and none for no weights
