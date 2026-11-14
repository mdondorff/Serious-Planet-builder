# k7-probe-emit (GEN-161 follow-up; briefing item 18, after PR 2)

## What
- One read-only `probe::emit(Stage::K7 { face, ti, tj }, ...)` (the tile in the stage, so sheets are `k7-f<face>-t<ti>-<tj>-z.png`: a multi-tile run writes one set per tile) at the end of `k7_tile` in `crates/generators/src/bake/k7.rs`: the finished window height `z` (Height) and the number `pits_after_pass`.
- Tier A in `tests/bake_probe.rs`: the K7 tile (`z`, `change`) is bit-identical with the probe on and off, and the `k7` sheet and its numbers line are written.

## Not in this change
- No requirement text change: GEN-161 already lists K7 among the stages.
- No phase-plan move, no new field, no manifest entry (the manifest fixture does not run K7).
