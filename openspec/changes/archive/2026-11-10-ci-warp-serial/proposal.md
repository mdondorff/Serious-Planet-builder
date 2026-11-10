# ci-warp-serial (inbox #108 item 4; BUILD-049)

Architect 2026-11-10 (decision D2): the heavy WARP gpu tests of the Windows `C` shards must not run in parallel; one `warp-heavy` group, max-threads 1, Windows only, no limit raised; the wall time of all four `C` shards before and after in the PR; if a shard doubles or comes within 20 % of its job timeout, max-threads 2 and both reported.

## What
- `.config/nextest.toml`: one override (`platform = 'cfg(windows)'`) puts eight tests of `gpu_all` that took over 60 s in the failed attempt of CI run 42780685812 (`C 1/4`) into `warp-heavy`: the look-2km smoke and the lit scene smoke of `gpu_stream_perf`, `gpu_stills` (one pose by name), `gpu_still_flicker` (streaming settle), `gpu_ground_look` (hills march shadow), `gpu_indirect`, `gpu_overlay` (inspection images), `gpu_steep_terrain`. No slow-timeout, test body, skip or retry changes.
- BUILD-049 and a tier-A check in `xtask` (`repocheck`).
- The shard wall times before and after: in the PR body.
