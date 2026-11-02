## Why
The one-off perf logs (`global-bake`, `coupled`, `coupled-make`, `envelope`, `edit-latency`) are written without `mode` or `load_record`, and the lit-pass rows carry no `loaded_verdicts` (Tracked item "Mode in the one-off logs", perf-loaded-mode, 2026-11-01). BUILD-032 asks for both on every row.

## What changes
- `planet_perf::stamp_one_off_json` opens a one-off JSON log with `mode` and (loaded) `load_record`, and closes it with `loaded_verdicts` for its time readings: bake seconds (GEN-014), coupled seconds (GEN-042), judged as unpinned CPU times (BUILD-031). The envelope log is judged on the coarse share, not a time, so it carries no verdict.
- `edit-latency.md` opens with `mode:` (and `load:`) lines and, in a loaded session, a section of loaded verdicts for the gated rows (preview and drawn-exact times against their budgets); the M6 verdict column stays as it is.
- `loaded::row_verdicts` judges the `lit/` rows (terrain, atmosphere and water GPU passes against their budgets, frame p99).
- The Tracked line is deleted from `docs/status.md`.

## Impact
- Spec: BUILD-032 (modified).
- Code: `crates/perf` (`lib.rs`, `loaded.rs`), `crates/app` (`globalperf.rs`, `envelopeperf.rs`, `editperf.rs`). No perf run: tests use synthetic records.
