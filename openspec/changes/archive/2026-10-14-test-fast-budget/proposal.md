## Why

M3 close-out item 3 (CLAUDE.md "tier A < 10 s"): the `accept M3` timing step read 11.0 s once. Measured on a quiet machine (20 threads), `cargo xtask test fast` took 9.3 s wall (nextest 7.1-8.5 s); the sum of test times was 129 s, 58 s of it in `planet-frame::height_aware` and 10 s in `planet-frame::lod`: sweeps that select nodes from scratch at hundreds of camera heights.

## What Changes

- `planet-frame` gains `select_nodes_cached` and `select_nodes_field_cached` (no frustum; the same `select_impl` and `SelectionCache` the in-view cached variant and the app use; results identical by LOD-008's cache contract).
- The morph sweeps (`morph_is_bounded_and_continuous_as_the_camera_recedes`, `morph_grows_monotonically_within_one_level`, the twenty `morph_is_bounded_and_merges_do_not_pop_*_chunk_*` tests) share one warm cache per test, as a flight does. Every 16th step is also selected cold and must be bit-identical, so the cold path stays under test. Every original assertion is kept at every original height.
- No test moves tier, none is removed or loosened.

## Result

Three quiet runs, nextest 5.4-5.7 s, wall 6.2-6.4 s (before: 9.3 s wall, 7.1-8.5 s nextest).
