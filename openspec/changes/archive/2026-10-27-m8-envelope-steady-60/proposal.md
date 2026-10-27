## Why
Owner (2026-10-27, #790): the 200 m/s floor of criterion 7 applies to steady flight, the speed the flight holds. The steady trial of m8-envelope-run-in measures 12 s with one chunk edge; a flight that holds a speed crosses many.

## What Changes
- The steady trial measures `--steady-window-s` seconds after its run-in (default 60) over ground the run-in did not cover, crossing several chunk edges; the bound is judged over every 1 s window of the whole stretch (the worst window). The first edge is half a quick window into the stretch.
- `--steady-window-s 12` is the quick single-edge trial. The cold trial and the smoke run are unchanged.
- The log gives `steady_window_s`.

## Impact
- Spec: TEST-057. Code: `app/envelopeperf.rs`, `app/lib.rs` (the option), `xtask` (the option passes through `perf envelope`). A steady search now flies 72 s per trial: the window is longer by the search's trials.
