## ADDED Requirements

### Requirement: STRM-061 A horizon bake takes one thread while chunk jobs run and stops when an edit makes it stale
The chunk horizon baker SHALL march on one thread while any chunk job is queued or running, and on its configured threads otherwise. A bake whose chunk's edit word (STRM-057) changed while it ran SHALL return no map: the job SHALL publish and store nothing, free its slot, and the chunk SHALL be baked again against the new word once the neighbourhood has settled (STRM-048).

Verify: A
Status: active
Source: opus-planner (2026-10-27, window 10), STRM-048, STRM-057, STRM-060

#### Scenario: A bake made stale by an edit
- **WHEN** a horizon job's baker reports that the heights went stale while it ran
- **THEN** no map lands or is published for the chunk, the cancelled count is 1, the job's slot is free, and the next update bakes the chunk again and that map lands
