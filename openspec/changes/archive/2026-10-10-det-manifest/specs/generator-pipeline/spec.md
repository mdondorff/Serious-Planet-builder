## MODIFIED Requirements

### Requirement: GEN-003 Cross-OS tile hashes
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash and mesh versions. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

Verify: A
Status: active
Source: CON-14, ADR 0010

#### Scenario: CI comparison
- **WHEN** the determinism job diffs the Windows and Linux manifests
- **THEN** they are equal, and a difference names the kind, the tile or point and both values

#### Scenario: Different builds
- **WHEN** the manifests carry different version lines
- **THEN** the report lists the version lines first and says the manifests come from different builds
