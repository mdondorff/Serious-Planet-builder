## Why
Owner fallback (2026-10-28, applied in PR #848, `docs/briefings/m8-plan.md` "Amendment 2026-10-28"): the chunk stage is off by default and hydrology and erosion are out of M8. `cargo xtask accept M8` still judges criterion 6 by the eroded chunk result within 10 s and lists the failed between-levels bound of criterion 3 and the river forms of criterion 1.

## What Changes
1. `accept_m8` criterion 6 judges the M6 table (`verdict`) and a horizon table (`verdict (10 s, horizon maps)`: the horizon shadows of an edit on screen within 10 s for every map the view wants); the chunk table (`verdict (10 s, drawn tiles)`) is not judged. A log without the horizon table is MISSING, never a pass.
2. `accept M8` runs `gate --small` (TEST-063) as an automated step; criterion 3 on the real store is the manual `cargo xtask gate` with the stage at its default (off) (TEST-061, TEST-062).
3. The manual list says criterion 1 without river forms and criterion 7 from a run with the stage off.
4. `docs/milestones/M8/acceptance-runbook.md` is rewritten to the amended criteria; the 0.712 m figure is identified (below).

## Impact
- Spec: TEST-060 (modified). Code: `crates/perf/src/accept_m8.rs`, `xtask/src/accept.rs`. Test: `accept_m8` tests (TEST-060).
- The edit scene does not log the horizon table yet: it is explorer's (stage off, horizon from the base terrain); until then criterion 6 reads MISSING.
- The 0.712 m of the runbook (15/14, `anet_steps_report`, chunk 0x0b000000002cba11) is the step between pyramid levels 15 and 14 of the chunk's own pyramid, not the production boundary (level 14 reads the base); D0 measured the same quantity as 0.390 m on chunk 0x0b000000002bd53a (`docs/milestones/M8/d0/README.md`).
