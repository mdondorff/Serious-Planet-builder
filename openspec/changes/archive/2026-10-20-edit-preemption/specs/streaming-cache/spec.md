## ADDED Requirements

### Requirement: STRM-031 Pre-emption of stale refreshes
Refresh work made stale by a newer edit SHALL NOT be computed (ADR 0072). When `Streamer::refresh` (STRM-026) names a tile again: a refresh of it that is still waiting, queued or held, SHALL stay one entry and be counted as `refresh_coalesced`; a job running for it SHALL be told to stop (once; STRM-005 polling) and be counted as `refresh_cancelled`; and a result that arrives for a job stopped by a refresh SHALL be dropped before the tile is made resident or uploaded and be counted as `refresh_dropped`. The three counts SHALL be part of the stream statistics. `Streamer::set_refresh_settle(ms)` SHALL set a settle window on the injected clock: with `ms` above 0 a resident tile that a refresh named is not requested until `ms` have passed since the latest refresh that named it, it stays resident and stale and is served from its old version meanwhile (STRM-026), and the request is made at the first step after that; a tile that is no longer in any class when the window ends is not requested (it stays stale, as in STRM-026). With `ms` 0, the default, nothing is held and the streamer behaves as before this requirement. A refresh SHALL be dispatched in its class (STRM-026) and the draw class goes before the residency and prefetch classes, so the refreshes of the tiles the plan draws start before those of tiles it does not draw, whatever the order the edit names them in. `ManagedLoop::set_refresh_settle` SHALL forward the setting and the editor window SHALL set 40 ms. Twenty edits of one resident tile, each a call of `ManagedLoop::edit` less than the settle window after the one before, SHALL start at most two refresh jobs for it (the one that may already run at the first edit, and one after the last).

Verify: A
Status: active
Source: ADR 0072, owner (2026-10-20), STRM-026, STRM-005

#### Scenario: Coalesced
- **WHEN** a tile is named by three refreshes while its refresh request waits in the queue, and again while the settle holds it
- **THEN** one request is dispatched when it is its turn or the window ends, and the coalesced count is the number of refreshes that found an entry waiting

#### Scenario: Cancelled and dropped
- **WHEN** a refresh job is running for a tile and a later edit names it, and the job ends with a result
- **THEN** the job was told to stop once and counted as cancelled, its result is dropped before the tile is made resident (the old version is still the resident one, and nothing is uploaded) and counted as dropped, and the tile is refreshed once from the newest definition

#### Scenario: Settle
- **WHEN** twenty edits of one resident tile come 10 ms apart with a settle of 40 ms, and a single edit comes with a settle of 40 ms and of 0
- **THEN** the twenty start at most two jobs and the last result is made from the last definition, the single edit's job starts 40 ms after it with the settle and at once without it, and a streamer whose settle was never set behaves as before

#### Scenario: Drawn first
- **WHEN** an edit names resident tiles of the draw, residency and prefetch classes in the opposite order
- **THEN** their refreshes are dispatched draw first, then residency, then prefetch
