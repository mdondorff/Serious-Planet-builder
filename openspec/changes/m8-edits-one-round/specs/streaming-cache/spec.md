## MODIFIED Requirements

### Requirement: STRM-056 A stale chunk keeps drawing and is re-solved when read
An edit SHALL mark the ready chunks whose read set (GEN-081) it touches stale. A stale chunk SHALL keep drawing its old version (STRM-054) until the new version lands, and SHALL be re-solved only when the plan reads it: nearest to the camera first, ahead of prefetch (STRM-042), with every chunk worker serving stale chunks once the missing chunks are served (`stale_workers` defaults to all, `workers` to the 4 chunks of a low view), and among the stale chunks first those whose own region holds an edit (`ChunkVersions::near_edit`), then those the wider read set only reaches (they may come out as they were), each group nearest first. While the loop says the tiles of the edit are still being built (`ChunkManager::hold_stale`) the stale chunks SHALL wait, at most `stale_hold_cap_ms` (1 s) from the first one seen stale, so the visible edit is drawn first (ADR 0083). The new version SHALL land as a second landing (STRM-052, STRM-053). No frame waits for it (CON-22).

Verify: A
Status: active
Source: m8-chunk-version, STRM-040, STRM-042, STRM-054

#### Scenario: An edit over a drawn chunk
- **WHEN** a world with a stand-in chunk stage on a fake clock has a chunk ready, gets an edit inside its halo, and the plan keeps reading the chunk
- **THEN** the chunk draws its old version until the new one lands, one job runs for it, a chunk the plan does not read stays stale and unsolved, and four stale jobs run at once on the default pool, the one with the edit in its own region first
