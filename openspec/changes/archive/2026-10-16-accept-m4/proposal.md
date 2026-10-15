## Why

M4's last queue item is `cargo xtask accept M4` and `docs/milestones/M4-report.md`. The hardware criteria need the architect's perf window, but the command that judges them can be built and tested now, on recorded fixtures, so that the window's logs go straight into a verdict (opus-planner decision of 2026-10-15). The owner amended the exit criteria on 2026-10-15 (ADR 0032): five scenes with the same hard measures, detail lag gated for the dive descent and the still look, tracked for the teleports and the dive's ground segment, `dive-fast` report-only.

## What Changes

- perf: `planet_perf::accept` (GPU-free): reads a scene's `<scene>.json` and `<scene>.frames.txt`, refuses a missing, invalid, in-memory-source, wrong-field, old-format, partial or inconsistent log, re-derives the verdicts with `streaming::case_verdicts`, judges the five gated scenes, reports `dive-fast`, renders the report with named thresholds and the findings slots. Tier A on logs written by the real log writer.
- app: the scene log gains the count `scene_alone` (1 when the process ran only this scene), so the RAM figure is known to be the scene's own; compare treats it as recorded-only.
- xtask: `accept M4 [--logs DIR]` runs the M3 steps without the discrete-GPU run, reads the logs, refuses or writes `docs/milestones/M4-report.md` (never over an existing one), prints the window commands.
- spec: TEST-021 (new), TEST-019 (the `scene_alone` count).

Not in this change: a perf window, any real log, the report itself (written from the logs after the window).
