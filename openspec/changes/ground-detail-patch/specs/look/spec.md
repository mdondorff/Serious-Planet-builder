## ADDED Requirements
### Requirement: LOOK-012 The patch key of the ground look
The `[body.<id>.ground]` table of the look file SHALL take an optional `patch` (schema version 6), the patch break-up of the material boundaries (REND-055) as a share of the slope cosine: 0 (the default, off) or from 1e-6 to 0.5; a value outside it SHALL be refused naming `ground.patch`, and the canonical text SHALL write the key. The lit plan SHALL carry the value, in its text and key only when it is not 0, take it from the file's table with the file's curves (VIEW-060), and the poses file's `[look] ground_patch` SHALL set it for the stills (TEST-053).

Verify: A
Status: planned
Source: REND-055, LOOK-011, LOOK-002; planner 2026-11-04

#### Scenario: The key
- **WHEN** a look file gives `patch = 0.12`, none, 0.7, -0.01 and 1e-9, a build that knows version 5 reads the version 6 fixture, and the plans of a file and of the default choice are built
- **THEN** 0.12 reads and writes back equal, none is 0 and is written as `patch = 0.0`, the others are refused naming `ground.patch` with the value, the older build refuses the file as a newer schema, the file's plan carries 0.12 with a text and key that differ from the plain plan's, and the default choice keeps 0

## MODIFIED Requirements
### Requirement: LOOK-002 Look file migrations
The look file SHALL have its own schema version (7 since the octave coefficients, LOOK-013; 6 `patch`, LOOK-012; 5 `ground`; 4 `water`, ADR 0041 points 4 to 6; 3 added the `atmosphere` table, ADR 0040; 2 added the `sun` and `albedo` tables, ADR 0039; 1 was the first layout), independent of the world definition's and of any generator version, and tested migrations in the manner of WDEF-002 (CON-10). **Every change that adds a table or a key to the look file SHALL raise the schema version and add an identity migration step**, so that a build that does not know the table says "newer schema version N, update the program" and not "unknown key" (the clearer failure under CON-10); moving or renaming a key needs a real step as well. Loading SHALL require the version, SHALL refuse a newer version with a message that gives both versions and says to update the program, SHALL refuse a gap in the migration chain naming the missing step, SHALL apply the migrations from the file's version to the current one in order, and SHALL fail with the step's message when a migration fails. The layout read after the chain SHALL be the current one, and a migrated file SHALL equal the file of the current layout with the same content and write the same canonical text. A table that no version of this build knows, in a file of the current version, is an unknown key naming the table (a typo, not a release). A look file SHALL NOT load as a world definition and a definition SHALL NOT load as a look file (the `kind` marker). The production chain SHALL have one step per older version (`MIGRATIONS.len() == SCHEMA_VERSION - 1`, a test pins it) and a fixture of every older version SHALL load through it.

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

#### Scenario: Version 6 fixture
- **WHEN** the version 6 fixture with a `patch` in the `ground` table is loaded
- **THEN** the key is read, the canonical text reads back equal, a version 5 file with the same content reads equal with the patch 0 (the identity step 5 to 6) and a build that knows only version 5 refuses the file as a newer schema

#### Scenario: Version 7 fixture
- **WHEN** the version 7 fixture with `octave` tables in the `ground` table is loaded
- **THEN** the tables are read, the canonical text reads back equal, a version 6 file with the same content reads equal with the Earth coefficients (the identity step 6 to 7) and a build that knows only version 6 refuses the file as a newer schema

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build (for example 8), has no migration path, or a migration step fails
- **THEN** loading fails with an error that names the versions or the step and what to do

#### Scenario: Not interchangeable with a definition
- **WHEN** a look file is given to the definition loader and a definition to the look loader
- **THEN** both fail and the look file's error names the missing `kind`
