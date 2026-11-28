## ADDED Requirements

### Requirement: WDEF-019 The regions file names a world's region boxes outside the definition
A world's region boxes SHALL live in `regions.toml` beside its definition file (`<world-dir>/regions.toml`), outside the `WorldDefinition` and every cache key, like the site bookmarks.
The file SHALL hold `format_version = 1` and one `[[region]]` per box with exactly `name`, `face`, `tiles_per_edge`, `i0`, `j0`, `i1`, `j1`: an inclusive rectangle of L tiles of one face. Names SHALL be unique, 1 to 40 characters of `a-z`, `0-9`, `-`, `_`.
One rule, `check_box`, SHALL decide which boxes can be baked, for the bake and the editor alike; every box it accepts SHALL pass `RegionSpec::check`.
A box's commit SHALL live in `<regions-parent>/regions/<name>/`; its state `unbaked`, `ready` or `stale` SHALL be derived from that commit, never stored.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring D1, the format; E1 and E2 Done when), GEN-177, REND-063, CON-05

#### Scenario: The format
- **WHEN** a file is written and read back, or a file holds another version, an unknown key, a missing or negative field, a bad or repeated name
- **THEN** the read gives the same boxes in order, or an error naming the key, field or name; a missing file reads as no boxes; a write replaces the file whole (a temporary file, then a rename)

#### Scenario: The shared box rule
- **WHEN** a box is checked in the production layout (12,288 regional cells, 128 L tiles of 768 local cells a face edge, a solve margin of 200 regional cells)
- **THEN** a side of 2 to 4 tiles inside tiles 3 to 124 of a face, with `tiles_per_edge = 128`, is accepted and passes `RegionSpec::check`; a box one tile further out, smaller, larger, empty, on face 6 or of another tile grid is refused with the reason

#### Scenario: The derived state
- **WHEN** a box has no commit, a commit of exactly its box, layout and inputs, a commit of another box, layout or definition (its items included), or a commit file that cannot be read
- **THEN** its state is `unbaked`, `ready`, `stale` or `stale`; the inputs id is the class inputs' key with the definition's hash, which holds the seed, the radius and the items, and the bake's code versions that no key carries

#### Scenario: The limit on items
- **WHEN** a world with items is baked
- **THEN** the base heights do not read the items (the class inputs are the seed's); the items enter the commit's identity only, so any item change makes every commit of the world `stale`, and the command says so
