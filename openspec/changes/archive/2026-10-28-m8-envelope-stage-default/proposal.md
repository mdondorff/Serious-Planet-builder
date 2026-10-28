## Why
Owner fallback 2026-10-28: the chunk stage is off by default (STRM-046), and criterion 7 is measured on tiles and horizon maps. The envelope scene ran the chunk stage whenever `--store` was given (it never read `--chunks`), so explorer's criterion 7 run (place 1: 190 m/s) was a stage-on measurement. Its `--out DIR` also wrote to the directory itself (Windows os error 5) after the 20-minute run.

## What Changes
1. The envelope scene runs the chunk stage only with `--chunks`; otherwise the horizon jobs alone from the base terrain (as the window does). Flight starts straddle a chunk edge only with the stage.
2. `--out DIR` gets `DIR/envelope.json` (`out_log_path`); a `.json` path is the file.
3. `envelope.json` records `chunk_stage`; `accept M8` reads criterion 7 as MISSING when it is true or absent.

## Impact
- Spec: TEST-060 (modified Detail), STRM-046 (unchanged text). Code: `app/envelopeperf.rs`, `perf/accept_m8.rs`. Tests: `envelopeperf`, `accept_m8` (STRM-046, TEST-047, TEST-060).
- Criterion 7 is re-measured in the next window with `--out` as a directory and no `--chunks`.
