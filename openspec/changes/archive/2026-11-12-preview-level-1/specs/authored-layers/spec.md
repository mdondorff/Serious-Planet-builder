## ADDED Requirements
### Requirement: AUTH-007 A level-1 preview is a non-authoritative field with its own id
A level-1 preview (A3 11.1) SHALL be a field whose id is a hash of a domain tag of its own, the base's identity word and the content hashes of the edit's items and climate regions. The same base and edit SHALL give the same id and any change of the edit or the base another. The preview SHALL be stored in no authoritative cache and SHALL be the key of no tile.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), A3 11.1, CON-07

#### Scenario: Ids
- **WHEN** the same edit is previewed twice, then an edit with one stamp moved, then a climate region added, then another base
- **THEN** the first two ids are equal and each of the others differs from them and from each other

### Requirement: AUTH-008 Level 1 re-routes H0 over the edit and returns the trunk rivers and the watersheds
For a height or landform stamp edit and for a rainfall or temperature region edit, the level-1 preview SHALL re-route the planet's H0 grid over the edited heights (the sea mask from the edited heights) and SHALL return as plain data the trunk cells (discharge at least a threshold), the watershed label of every cell and the cells whose receiver or watershed changed against the baseline. A climate-only edit SHALL keep the baseline routing and change only the discharge.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), A3 11.1

#### Scenario: Two edit kinds
- **WHEN** a hill is stamped on a drainage divide, and a rainfall region is placed over a catchment without changing any height
- **THEN** the first changes receivers or watersheds, most of them near the hill, and the second changes no receiver and raises the discharge below the region

### Requirement: AUTH-009 The preview is decided outside the renderer
The level-1 preview SHALL be a GPU-free function of the resident base and the edit, SHALL reach the frame plan as data only, and SHALL never make the plan request a solve (CON-03, A3 11.2).

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), CON-03, A3 11.2

#### Scenario: The layering
- **WHEN** the layering check runs
- **THEN** the preview module is in a GPU-free crate and no crate that renders depends on a solve entry of it

### Requirement: AUTH-010 A superseded preview is dropped
When an edit is superseded by a later one before its preview finishes, the result of the earlier SHALL be dropped and only the newest edit's result SHALL be accepted.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), ADR 0072 point 3

#### Scenario: A drag
- **WHEN** three edits are submitted and their results arrive in the order second, third, first
- **THEN** only the third is accepted
