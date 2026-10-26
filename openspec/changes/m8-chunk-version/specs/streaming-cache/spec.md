## ADDED Requirements

### Requirement: STRM-056 A stale chunk keeps drawing and is re-solved when read
An edit SHALL mark the ready chunks whose read set (GEN-079) it touches stale. A stale chunk SHALL keep drawing its old version (STRM-054) until the new version lands, and SHALL be re-solved only when the plan reads it: nearest to the camera first, ahead of prefetch (STRM-042), with at most two of the chunk workers on stale chunks. The new version SHALL land as a second landing (STRM-052, STRM-053). No frame waits for it (CON-22).

Verify: A
Status: planned
Source: m8-chunk-version, STRM-040, STRM-042, STRM-054

#### Scenario: An edit over a drawn chunk
- **WHEN** a world with a stand-in chunk stage on a fake clock has a chunk ready, gets an edit inside its halo, and the plan keeps reading the chunk
- **THEN** the chunk draws its old version until the new one lands, one job runs for it, a chunk the plan does not read stays stale and unsolved, and at most two stale jobs run at once
