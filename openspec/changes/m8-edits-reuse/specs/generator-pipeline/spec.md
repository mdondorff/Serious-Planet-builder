## ADDED Requirements

### Requirement: GEN-087 A re-solve reads again only the heights an edit reaches
The base heights of the cells of a chunk's region SHALL be kept with the definition they were read under. For a later definition that differs from it only in its authored items, a chunk job SHALL read again only the cells within the reach (`reach_m`, widened as every reach test is) of the old and of the new place of each item that differs and, for an item that is a brush, within the radius of one of its strokes (WDEF-007; a stroke changes no bit of a point beyond its radius, GEN-030), and SHALL take the other cells from the kept heights; the result of the chunk SHALL be the same bits as a cold solve of the later definition. The kept heights SHALL be at most 24 batches and 256 MiB, and SHALL NOT be kept when the definition moved while they were read.

Verify: B
Status: active
Source: opus-planner (2026-10-27), WDEF-010, GEN-026, STRM-034, GEN-082

#### Scenario: A hill put, moved and taken away
- **WHEN** a chunk is solved, a hill is put in its read set, moved, and taken away, and the chunk is solved again after each
- **THEN** each result equals the cold solve of the same definition bit for bit, the cells outside the hill's reach were not read again, and after the removal the result equals the first one

#### Scenario: A cell at the edge of the reach
- **WHEN** a cell lies exactly at the reach of a changed item
- **THEN** it is read again; a cell clearly beyond the reach is not

#### Scenario: A brush put, extended and taken away
- **WHEN** a chunk is solved, a brush of one stroke is put through its read set, given more points, and taken away, and the chunk is solved again after each
- **THEN** each result equals the cold solve of the same definition bit for bit, the cells beside the stroke, though inside the brush's reach disc, were not read again (most of the region), and after the removal the result equals the first one
