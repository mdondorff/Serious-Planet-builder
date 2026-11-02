## Design notes
- `duty_idle_s(previous_run_s)` is pure (tier A). `ab::Runner` gains `rest()` (default: nothing); `ab::execute` calls it before every run but the first, and not between a refused try and its retry (`wait()` covers those). `TreeRunner` times each child and sleeps the idle in `rest()`.
- `start_steps` returns no cool start for every streaming run; the call site stays, so a later ADR can restore a cool start in one place (ADR 0094 point 4).
- The idle is a share of the previous run, so short runs (teleport, 40 s) rest 20 s and long ones (dive, 77 s) 38.5 s; window 2's fixed 45 s became the cap.
- PR 1's calibration comment in `streamperf.rs` says "after the settle and the cool-down"; after both PRs merge, the stream reading is after the settle only. The comment is corrected when this branch is rebased on PR 1.
- The sleeps themselves (`TreeRunner::rest`, `CargoChildren::settle_after` for `perf stream all`) have no test: they call the tested `duty_idle_s` and `std::thread::sleep`; the call pattern is tested through the fake A/B runner and `session::run_scenes`' settle calls.
- Stop rule: `clock::hot_start_line` (tier A) is printed by `streamperf.rs` from the run's first clock sample.
