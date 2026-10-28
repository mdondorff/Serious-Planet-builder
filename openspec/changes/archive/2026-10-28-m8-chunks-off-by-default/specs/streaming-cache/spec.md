## MODIFIED Requirements

### Requirement: STRM-046 The viewer runs A-net chunks at runtime when asked
A world that has a stored coupled channel SHALL have a chunk stage when it is opened for viewing with `--chunks`: the A-net process on the chunk manager (jobs on the injected spawner, at most half the worker pool), the pyramid top level the measurement chose, and a disk cache of landed chunks in the store's directory. Without `--chunks`, and for a world without the channel, there SHALL be none and every tile SHALL be the base. No frame SHALL wait for a chunk (STRM-039).

Verify: A
Status: active
Source: D7 (runtime generation, disk cache), ADR 0077 point 7, STRM-039, STRM-041, STRM-043, owner fallback 2026-10-28

#### Scenario: Enabled by the channel
- **WHEN** a world with a stored coupled channel and a world without one are opened with the stage asked for
- **THEN** the first has a chunk manager whose fields carry the chunk stage and the second has none, and a chunk landed in the first is read from the disk cache after a restart without computing

#### Scenario: Off by default
- **WHEN** the window opens a world with a stored coupled channel without `--chunks`
- **THEN** it has no chunk manager and computes no chunk
