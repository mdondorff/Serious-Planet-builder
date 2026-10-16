## ADDED Requirements

### Requirement: LOOK-001 Look file schema, validation and canonical text
The look of a body SHALL be presentation data (ADR 0035) held in a TOML look file with a top-level `schema_version`, the marker `kind = "look"`, and tables `[body.<id>.units]`, `[body.<id>.exposure]`, `[body.<id>.histogram]` and `[body.<id>.tone_map]`, addressed by body id as the world definition is (`body.<n>`, a non-negative integer without leading zeros). Schema, parsing, validation and the canonical writer SHALL live in a GPU-free crate (`planet_frame::look::file`, CON-03). Every table and key SHALL be optional and a missing one SHALL be the Earth default. Values SHALL be finite floats (an integer literal exact as a float is accepted); an unknown table or key, a value of the wrong type, a body id that is not canonical, an unknown tone curve and a value outside its range SHALL be errors that name the full key (`body.2.exposure.min_ev100`) and the value. Writing SHALL be canonical (version and kind first, bodies in id order, every key written, floats in the shortest round-trip form) and reading it back SHALL give an equal file bit for bit. Each attribute is presentation data, never read back by a CPU system (other than the CPU reference and the plan builders), never stored or keyed as world data and never an input to a field; its declared bound is the valid range below (a value outside it is refused, so a shader never sees it) and the precision with which the value reaches the GPU:

| Attribute (key under `body.<id>`) | Class | Earth default | Valid range | Precision bound |
|---|---|---|---|---|
| `units.solar_illuminance_lux` (the lux of one scene unit) | presentation | 126600.0 | 1 to 1e7 | decided in f64, the pre-exposure factor rounded once to f32 (relative 6e-8) |
| `exposure.min_ev100`, `exposure.max_ev100` | presentation | -2.0, 16.0 | -20 to 40, min <= max, and, as a conservative proxy for the pre-exposure the view chooses, one unit written at max_ev100 stays a normal f16 (lux / (1.2 x 2^max) >= 2^-14) | rounded once to f32; the adapted EV matches the CPU step within 1e-4 EV (REND-022) |
| `exposure.speed_up`, `exposure.speed_down` (stops per second) | presentation | 3.0, 1.0 | 0 to 100 | rounded once to f32 |
| `histogram.ev_min`, `histogram.ev_max` | presentation | -10.0, 20.0 | -50 to 50, at least 1 stop apart | 64 bins over the axis; bin counts equal the CPU reference away from bin edges (REND-022) |
| `histogram.band_low`, `histogram.band_high` | presentation | 0.4, 0.9 | 0 to 1, high - low >= 0.01 | rounded once to f32; target within 1e-4 EV of the CPU (REND-022) |
| `tone_map.curve` | presentation | `"aces"` | `"aces"` | within 1/255 per channel of the CPU reference (REND-023) |

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
The look file SHALL have its own schema version (1 at this change), independent of the world definition's and of any generator version, and tested migrations in the manner of WDEF-002 (CON-10). Loading SHALL require the version, SHALL refuse a newer version with a message that gives both versions and says to update the program, SHALL refuse a gap in the migration chain naming the missing step, SHALL apply the migrations from the file's version to the current one in order, and SHALL fail with the step's message when a migration fails. The layout read after the chain SHALL be the current one, and a migrated file SHALL equal the file of the current layout with the same content and write the same canonical text. A look file SHALL NOT load as a world definition and a definition SHALL NOT load as a look file (the `kind` marker).

Verify: A
Status: active
Source: CON-10, ADR 0042 point 6, WDEF-002

#### Scenario: Older layouts
- **WHEN** files of an older synthetic layout (versions 1 and 2 of a three-version chain, until a second real layout exists) and the current layout of the same content are loaded
- **THEN** all three equal each other and write the same canonical text

#### Scenario: Future tables
- **WHEN** a build adds an optional table (items 4, 5, 7) and an older build reads a file that has it
- **THEN** the older build refuses it as an unknown key naming the table; the schema version is raised, with an identity step, for a release that must be recognised as newer, a decision taken by the item that adds the table

#### Scenario: Version 1 fixture
- **WHEN** the version 1 fixture is loaded by the production chain and written back
- **THEN** it reads as written and the written text reads back equal

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build (for example 2), has no migration path, or a migration step fails
- **THEN** loading fails with an error that names the versions or the step and what to do

#### Scenario: Not interchangeable with a definition
- **WHEN** a look file is given to the definition loader and a definition to the look loader
- **THEN** both fail and the look file's error names the missing `kind`

### Requirement: LOOK-003 Earth defaults and the plan
The Earth default look SHALL be physically based and SHALL equal the values that were code constants before the look file existed: a scene unit of 126,600 lux, exposure range -2 to 16 EV100 with speeds 3 and 1 stops per second, the histogram over EV -10 to +20 with the band 40 % to 90 %, and the ACES curve. A body without an entry SHALL have the default as a whole. A lit plan built without a look (`LitPlan::fixed`, `LitPlan::adaptive`) SHALL equal the plan built from the default look and SHALL keep its text form and hash: the look enters a plan's text only when it differs from the default, as exact bit patterns, and the text round-trips. A plan SHALL be refused, naming the key and the value, when its scene unit or histogram is outside the ranges of LOOK-001 or its pre-exposure factor does not fit an f32. The CPU references (`pre_exposure_factor`, the histogram, band mean and target) SHALL take the look as parameters, and the functions without them SHALL be the Earth default. Class: presentation data (ADR 0035).

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
