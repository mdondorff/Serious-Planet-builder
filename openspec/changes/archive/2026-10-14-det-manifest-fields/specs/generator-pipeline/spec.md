## MODIFIED Requirements

### Requirement: GEN-003 Cross-OS tile hashes
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. It SHALL also list the global bake at 64 cells per face edge (the whole-grid hash and the hash of every block of 16 cells at level 2), the raw `f64` height of the terrain field (`cpu-terrain-v1`) and of the eroded field (`cpu-erosion-v1`) at twelve face-coordinate points on every face (the four cube corners, the four edge midpoints, the centre and three further points on and near edges and corners), the versions and implementation ids of the bake and both fields, and the persistent known answers of ADR 0018 (keys, node, body and query hashes, the bake's stage and block keys and node set id; written by `app`, ADR 0023 point 9). A one-ulp change in the value behind any of the new entries SHALL change that entry. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

Verify: A
Status: active
Source: CON-14, ADR 0010, ADR 0023

#### Scenario: CI comparison
- **WHEN** the determinism job diffs the Windows and Linux manifests
- **THEN** they are equal, and a difference names the kind, the tile or point and both values

#### Scenario: Mutation of a new entry
- **WHEN** the value behind any bake, block, terrain or eroded entry is changed by one ulp
- **THEN** exactly that line of the manifest changes

#### Scenario: Different builds
- **WHEN** the manifests carry different version lines
- **THEN** the report lists the version lines first and says the manifests come from different builds
