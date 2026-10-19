# 0065 CPU calibration decides whether a perf scene counts
Status: accepted (opus-planner, 2026-10-19, under the owner's delegation of 2026-10-17; easy to reverse). Extends ADR 0020 (readiness gate) and ADR 0036 (validity).
Date: 2026-10-19
Source: perf windows 7 and 11, the desktop-app smoke (`docs/perf/smoke-desktop-app.md`), the planner-terminal window logs; sonnet-crusader's analysis of 2026-10-19

## Context
- Each streaming scene records a fixed CPU kernel's time alone and on the pool's threads, before and after the case (STRM-022); nothing judges it.
- A prose rule in `docs/status.md` (single <= 50,000 us, parallel median <= 100,000 us) disagreed with the readiness gate, which judges load and temperature only.
- On a quiet machine the prose rule fails about one clean scene in five: before parallel spreads 85,042 to 106,885 us, and after single drifts upward after a long scene.
- The slow state of the planner terminal (1.4x per thread) shows in both readings: before single 56,696 us and up, parallel 127,845 us and up.
- Lit scenes record no calibration.

## Decision
1. A scene's verdicts count only if its CPU calibration lies within limits taken from the quiet-machine spread (its maximum plus about 5 %), checked before and after the case; the limits are the spec's numbers.
2. A scene outside the limits is "no verdict", with the reading, the limit and before or after named, in its log and in `cargo xtask accept`.
3. The lit scenes record the same calibration and fall under the same rule; logs without a calibration are "not judged", not invalid.
4. Limits relative to a per-machine baseline are deferred until perf runs on a second machine.

## Consequences
- The prose rule in `docs/status.md` is replaced by a pointer to the spec.
- Every planner-terminal scene and the git-disturbed smoke dive come out "no verdict"; every quiet scene of windows 7 and 11 counts.
- A quiet-machine drift beyond the limits needs new data and a spec change, not a looser reading.

## Licences of adopted code or techniques
None.
