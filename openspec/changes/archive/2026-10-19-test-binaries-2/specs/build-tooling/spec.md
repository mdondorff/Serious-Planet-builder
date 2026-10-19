## MODIFIED Requirements

### Requirement: BUILD-015 One test binary per tier where it shortens the edit-to-test build
The crates done are now `generators`, `cache`, `streaming`, `app`, `frame` (`it`, `oracle_all`) and `render` (`it`, `oracle_all`, `gpu_all`): a `frame` edit rebuilt the test binaries of `frame`, `render`, `editor` and `app` in 33.5 s before and 10.7 s after (median of three, 68 % below). The `real-gpu` profile also names the `gpu_lit`, `gpu_water`, `gpu_atmosphere` and `gpu_compact_blocks` tests of `planet-render::gpu_all` and the `oracle_lit` tests of `planet-render::oracle_all` in the group `real-gpu-heavy`. Everything else of the requirement is unchanged.
