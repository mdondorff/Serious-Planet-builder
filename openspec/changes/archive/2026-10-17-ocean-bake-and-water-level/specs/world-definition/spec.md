## MODIFIED Requirements

### Requirement: WDEF-002 Schema version and migrations
The WorldDefinition SHALL be stored as TOML with a top-level `schema_version` (2 since the liquid table and the sea of a body, WDEF-006: version 2 only adds keys, so the migration from version 1 is the identity), independent of the generator version. Loading SHALL require the version, reject a newer one and a gap in the migration chain with an error that names what to do, apply the migrations from the file's version to the current one in order, and fail with the step's message when a migration fails. Files of every older schema version in the fixture set SHALL load through tested migrations and equal the current-layout definition of the same content. Values SHALL be non-negative integers (a `u64` above `i64::MAX`, and only that, as a canonical decimal string) and finite floats, typed by their literal except for the paths the program reads (`world.seed` an integer, `body.<n>.radius_m`, `body.<n>.sea.level_m`, `body.<n>.sea.min_area_fraction` and every `liquid.<k>.*` node a float, `body.<n>.sea.liquid` an integer, where an exact integer literal is accepted for the float and a float for the integer is an error); the one string value is `world.height_field`, the implementation id of the height field (letters, digits, `_` and `-`), which every other path rejects as a string and which is rejected as a number; any other value, a negative integer, a key outside letters, digits, `_` and `-`, or a reserved name (`derived`, `schema_version`) SHALL be an error naming the path. Writing SHALL be canonical (version first, sorted tables, shortest round-trip floats) and reading it back SHALL give the same nodes and height-field id bit for bit.

Verify: A
Status: active
Source: CON-10

#### Scenario: Old fixture
- **WHEN** a fixture written by an older schema version is loaded
- **THEN** it migrates and equals the current-version fixture of the same content (the version 1 fixture through the identity step to version 2; the engine is also tested with a synthetic three-version chain)

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build, or has no migration path, or a value cannot be stored exactly
- **THEN** loading fails with an error that names the version or the path

#### Scenario: Height field id
- **WHEN** a file sets `[world] height_field = "cpu-terrain-v1"`, and when it sets the same path to a number or any other path to a string
- **THEN** the first loads, equals the definition built in code and round-trips; the others fail naming the path

## ADDED Requirements

### Requirement: WDEF-006 Liquid kinds and the sea of a body
Liquid kinds SHALL be a **parameter table in the world definition**, not a code enum (the owner's M5 constraint 3, ADR 0041 point 3): a row is a kind id `<k>` and nine float nodes `liquid.<k>.colour_r|colour_g|colour_b` (the liquid's colour in deep water, linear RGB, each in [0, 1]), `emission_r|emission_g|emission_b` (emitted radiance in units of the scene's sun illuminance, linear RGB, each at least 0; water 0, lava above), `viscosity_pa_s` (above 0), `wave_amplitude_m` (at least 0; 0 means no waves) and `wave_length_m` (above 0 when there are waves); a row with a missing, non-numeric, non-finite or meaningless node SHALL be an error naming it. A world that defines no `liquid.*` node SHALL have the Earth table, one liquid with id 0, water; a world that defines any SHALL have exactly the rows it defines (nothing merged), so a definition that never mentions liquids keeps the hash it had and a definition with a table is a different definition (WDEF-003). The **sea** of a body SHALL be three optional nodes with Earth defaults: `body.<n>.sea.level_m` (the water level W of the sea, a finite float, default 0), `body.<n>.sea.liquid` (the kind id, an integer up to `u32::MAX`, default 0) and `body.<n>.sea.min_area_fraction` (the area threshold of the ocean connectivity as a fraction of the body's surface, from 0 to 1, default `EARTH_OCEAN_MIN_AREA_FRACTION`, 1e6 km^2 of an Earth surface); a wrong type, a non-finite level or a fraction outside [0, 1] SHALL be an error naming the node. A generator reads the table and the sea through the recording context (`GenContext::liquids`, `GenContext::sea`), which records the nodes the definition sets (none when it sets none), so adding or changing a node changes the read set of exactly the generators that read it. Class of the attributes (ADR 0035): colour, emission, wave amplitude and wavelength are **presentation parameters held as definition nodes** (nothing in a generator or a CPU system reads them; bound: exact, the validated ranges above); viscosity is world data that a field may read later; the sea's level, liquid and threshold are field inputs (GEN-022, GEN-023). Because the nodes are in the definition, editing a liquid's colour changes the definition hash (CON-09 cost, ADR 0051 point 1); the look file (ADR 0042 point 6) overrides how a liquid is presented for display only, and where it sets a value it wins over the definition's. The sea's liquid SHALL name a row of the table, and a body whose sea names none SHALL be refused when its bake is opened, naming the node.

Verify: A
Status: active
Source: owner's M5 constraint 3, ADR 0041 points 1 to 3, ADR 0006, ADR 0035, CON-05, CON-10

#### Scenario: Defaults
- **WHEN** a definition sets no liquid and no sea
- **THEN** its table is the Earth table (water), its sea is level 0 of liquid 0 at the Earth threshold, its hash is the hash it had before the table existed, and a generator that reads them records nothing

#### Scenario: A table
- **WHEN** a world defines liquids 0 and 7 (water and lava) and a sea of kind 7
- **THEN** its table has exactly those rows, every field of every row is in the hash, the sea names kind 7, another body has the Earth sea, and a read of the table records its nodes

#### Scenario: A dangling kind
- **WHEN** a body's sea names liquid 5 and the table has no such row
- **THEN** opening its bake is refused naming `body.0.sea.liquid` and the kind

#### Scenario: Bad rows
- **WHEN** a row has a colour above 1, a negative emission, a zero viscosity, waves without a wavelength, a missing node or an integer where a float is read
- **THEN** it is refused naming the liquid and the field, and nothing is written by a refused row

#### Scenario: The file
- **WHEN** a hand-written file defines two liquids and a lava sea with integer literals for float nodes
- **THEN** it loads to the definition built in code and writes back canonically
