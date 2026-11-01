## Why
SPIKE-84 fix 2:
- Seven `planet-cache::concurrent_source` tests read a stored bake, and each rebuilt the same 64-cell store first.
- `build_global` is about 0.75 s of each test's 1 to 1.4 s alone; the in-memory bake is 0.04 s.
- Under the load of a full `test fast` the medians were 18 to 31 s per test.
- nextest runs each test in its own process: an in-memory `OnceLock` (SPIKE-84's wording) shares nothing, so the store is shared on disk.

## What Changes
- `crates/cache/tests/support/concurrent_source.rs`: `stored()` returns one store per test binary and fixture key under `CARGO_TARGET_TMPDIR` (inside `target/`).
  - Key: the bake's content key (generator version, world, resolution), the pack format and block codec versions, the block limit, the pack size, and the test binary's size and time (a relinked binary never reads a store built by older code).
  - A process builds into a directory of its own, writes a `MANIFEST` (each file's length and CRC-32C, and the key) and renames it into place; a reader never sees a half-built store. When two processes build at once, the first rename wins and the other uses it. A failed rename is retried for up to 2 s.
  - A slot whose files do not match its manifest is never read: the process builds and reads a private copy, deleted after the test. The damaged slot stays until the binary's next build removes it; removing it at once could remove a good slot that another process has just put there.
  - A rebuilt binary removes its earlier slots (best effort).
  - Each test still opens its own `TileStore` (read-only; `TileStore::open` writes nothing), so the read counters start at zero.
- The seven tier-A tests and the two tier-B oracles call `stored()`; the lock-proxy test keeps its own 16-cell store (its only user); the missing-block and default-budget tests keep their empty directories.
- No assertion, tile set, budget or thread count changes; no test is removed or weakened.

Load-dependent timings (Rasierklinge, another lane's perf sweep running):

| planet-cache tier A (165 tests) | before | after |
|---|---|---|
| parallel, median of 5 runs | 8.5 s | 4.6 s |
| one thread | 20.7 s | 12.3 s |

## Capabilities
### Modified Capabilities
- None: test fixtures only (STRM-014, STRM-022, GEN-020 unchanged); no spec delta.

## Impact
- `crates/cache/tests/` only; tier A and B. A store of about 1 MB per test binary stays in `target/tmp/` between runs; a binary with a new cargo hash (another profile or feature set) leaves the old slot until `cargo clean`.

Checked (`concurrent_source` tests of both binaries, 12 tests): a clean `target/tmp` (8.99 s), warm (4.67 s), a damaged slot (a flipped byte in its pack: private copies, 10 of 10 pass), one thread.
