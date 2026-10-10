## ADDED Requirements

### Requirement: BUILD-007 Walking skeleton in every mode
The walking skeleton (generate a tile, plan a frame, execute it, produce pixels) SHALL run in every run mode that can render: `test-render` and `editor --offscreen` (the window arrives in M2) SHALL produce the same frame for the same options, and `generate` SHALL print the content hash and cache key of a tile and write its height map and the face net; options a mode does not support SHALL be rejected, not ignored.

Verify: C
Status: active
Source: CON-01, ADR 0008

#### Scenario: Same frame
- **WHEN** the tiles scene is rendered by `test-render` and by `editor --offscreen` with the same options
- **THEN** the two PNGs are identical

#### Scenario: Generate
- **WHEN** `generate --tile 3,3,5,2` runs with output paths
- **THEN** it prints a content hash and a cache key and writes the height map and the face net

#### Scenario: Unsupported option
- **WHEN** `generate` is given `--repro`
- **THEN** it fails with a message naming the modes that support it
