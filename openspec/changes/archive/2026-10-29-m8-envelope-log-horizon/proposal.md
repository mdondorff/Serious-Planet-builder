## Why
The verifier of #861 found two gaps: a stage-off `envelope.json` wrote `"horizon": false` and a `stand_in_reason` although the horizon jobs ran (`horizon = chunk_field && horizon`), and a log of an in-memory bake without horizon jobs could pass criterion 7 (`chunk_stage: false`). Planner (2026-10-28): fix both.

## What Changes
1. `envelope.json`: `horizon` is whether the horizon jobs ran (from the base terrain without the chunk stage); `stand_in_reason` is written only when they did not.
2. `accept M8` reads criterion 7 as MISSING unless the log shows a run over the production store (`source` without "NOT the production source") with `horizon` true, besides `chunk_stage` false.

## Impact
- Spec: TEST-060 (Detail). Code: `app/envelopeperf.rs`, `perf/accept_m8.rs`. Tests: `envelopeperf`, `accept_m8` (STRM-046, TEST-060).
