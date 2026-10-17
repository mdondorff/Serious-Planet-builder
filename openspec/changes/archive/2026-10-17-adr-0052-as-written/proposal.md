## Why
PR #236 read ADR 0052 point 1 as "at most 64 pixels beyond delta 1 plus at most 32 pixels at delta 1" (opus-planner, 2026-10-17). The review pass of PR #238 showed the 32-pixel cap was the wrong call: the pixels beyond delta 1 are all small (at most 24 against 64), while the delta-1 counts (36 to 3,448) appear only between the RTX and the two software adapters, never WARP against llvmpipe: hardware-against-software float rounding across the image. The planner withdrew the reading; ADR 0052 point 1 stands as written: per adapter pair at most 64 pixels beyond delta 1, any number at delta 1.

## What Changes
- `testkit::review`: the ADR 0052 path (a new debug view of a first M5 golden, candidate against candidate) no longer caps the pixels at delta 1; `ViewDiff.at_rounding` stays as a reported count. The message and the evidence line still name the cap exceeded and the counts. The existing-golden path (ADR 0017: 32 px, delta 1, candidate against golden; the 8 px agreement floor) is unchanged.
- Bound-test names in `ADR 0037 conditions:` may be bare, `binary::name` or `crate::binary::name` (the water-pass proposal wrote `gpu_water::name`, which the tool did not match).
- A candidate byte-equal to its golden (a CI artefact beside a golden blessed earlier in the same pass) is `Unchanged`, not a zero-pixel `Changed`: with the second reading, blessing a debug view mid-run made the goldens that name it as their view escalate ("is a lit view"). Tier-A test on the bin's `observe`.
- TEST-022, the skill text, the log line. The review pass is re-run on the same evidence with `--apply`; the status item about the 11 escalated goldens is removed.

## Classes and bounds (ADR 0035)
No field and no shader value is added; test tooling.

## Out of scope
The existing-golden path, the driver gate, the NOT RUN rule, the new-family escalation (all unchanged).

## Outcome
Re-run on the same evidence (CI run 39029009545, local RTX, this run's JUnit) with `--apply`: all 11 pending goldens AUTO-M5 on all three adapters (33 PNGs), in two applies because `lit_aerial20km_ev15` and `lit_ground1m_ev15` name `lit_aerial20km_albedo_ev15` as their view, which was blessed by the first apply. Nothing escalates. `tests/goldens/pending.txt` is empty; `cargo xtask test gpu --real` is green with no candidates left.
