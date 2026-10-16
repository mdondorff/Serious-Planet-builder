## ADDED Requirements

### Requirement: STRM-022 Tile load counters
The streamer, the block source, the tile pack and the streaming perf scenes SHALL record what a tile's load costs and where it goes, as readings that no decision reads (the request order, the dispatch, the cache contents and every result stay a function of the wants, the resident set, the failure times and the limits, CON-14): (1) the streamer SHALL log every tile job that ran its load (`Streamer::job_log`, `JobSample`): the microseconds from dispatch to start, the microseconds of the load, the number of loads inside their source when it started (itself included), the tile's level, and whether it produced a tile; a job cancelled before it starts is not logged; (2) the stored-bake reader (`StoreSource::stats`) SHALL count its shared-cache lookups and hits, the blocks pushed out of the cache for the budget, the lookups that found a block another thread was still decoding, the decodes, and the nanoseconds all threads together spent on the cache lock, fetching a block's record and decoding it, with `shared_lookups - shared_hits = decodes` when no read fails and `evictions = decodes - budget` once the budget is exceeded, and SHALL expose per thread the nanoseconds spent getting blocks and the blocks it decoded (`thread_block_wait_ns`, `thread_block_decodes`); a read inside the block the thread read last SHALL touch no shared counter or lock; (3) the tile pack SHALL count its lookups, those that found a record and the nanoseconds they took (`TileStore::read_stats`); (4) the world's terrain SHALL count the pre-filled tile lookups that reached the pack, those that found the tile, the lookups skipped for a tile too deep to be stored, their nanoseconds, the fields built, and the jobs' own block wait and decodes (`WorldTerrain::counters`); (5) each streaming perf case SHALL write into its log, for the measured frames, the `load_` counts of those readings (`load_jobs`, the median, p99, maximum and sum of the load time and the median, p99 and sum of the queue wait, the median load time by concurrency band and by level band, the block cache's decodes, lookups, hits, evictions, waits, lock, fetch and decode time, residency and budget, the decodes and lookups per 1000 tiles, the pack's and the pre-filled lookups' counts and time, the jobs' block wait and decodes, the fields built) and, in a run that is not a smoke run, a CPU calibration before and after the case (`load_calib_before_*`, `load_calib_after_*`: a fixed ALU kernel's time alone, and its median and maximum time on as many threads as the generation pool has at once, the same value computed on every call); (6) the regression comparison SHALL record these counts and SHALL NOT report a change of any count named with the prefix `load_`, nor judge any budget by one.

Verify: A, C
Status: active
Source: perf window 3 (2026-10-16), M4 fix F11, STRM-014, STRM-020, STRM-021, TEST-012, TEST-018, CON-14, CON-20

#### Scenario: The job log counts jobs, levels, outcomes and concurrency
- **WHEN** jobs run on a manual spawner, with a source that fails one level, with a job cancelled before it starts, and on a pool of four workers whose loads meet at a barrier
- **THEN** one sample is logged per job that ran (the dispatched count), the levels are those of the dispatched tiles, a failed load is not `done`, a cancelled job is not logged, loads one at a time have concurrency 1 and four loads inside the source together give a sample with concurrency 4 and none above

#### Scenario: The block source counts its cache exactly
- **WHEN** one thread reads a grid through a cache of five blocks, and eight threads read it through a cache that holds every block
- **THEN** the lookups are the hits plus the decodes, the evictions are the decodes beyond five, nothing waited for another thread, the thread's decodes equal the source's, and with the roomy cache every block is decoded once with no eviction and no more waits than hits, every read returning the stored bits

#### Scenario: The pack counts its lookups
- **WHEN** blocks are decoded through a store and a key it does not hold is asked for
- **THEN** the store counts one lookup that found a record per decode, and the missing key one lookup that found none

#### Scenario: The pre-filled lookups are counted
- **WHEN** pre-filled tiles of levels 0-3 are loaded through the streamer's source, then a level-5 tile and a level-9 tile
- **THEN** each tile loaded on the pack's world is a lookup and a find, the cold reader counts none, the level-5 tile is a lookup without a find, the level-9 tile is skipped without a lookup, and the pack saw at least those lookups

#### Scenario: The load counts are a pure summary
- **WHEN** fixed jobs and counters are summarised
- **THEN** the medians, tails, sums, bands and per-1000 figures are those of the inputs, every name has the prefix `load_` and none repeats, no jobs give zeros, the calibration kernel gives the same value for the same work and reports its thread count, and a comparison of two logs that differ only in `load_` counts reports no changed count

#### Scenario: A smoke log carries the counts
- **WHEN** the streaming scenes run in smoke mode on the software adapter
- **THEN** every log has the job, block, pack, pre-fill and field counts, the pack's lookups are at least the block decodes, and no calibration
