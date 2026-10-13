## Decisions

- The production-resolution planning check is a recorded acceptance run, not a test: a 16384-cell bake is 11.7 GB and 6 minutes, far beyond any tier. The command (`generate --plan-check`) is tested for its refusals and on a 256-cell bake in tier B; its output at 16384 cells is kept in `docs/milestones/M3-plan-check-16384.txt`.
- `--global-res` is required (a store holds one bake per resolution; a default would check the wrong bake).
- The M3 report's recommendation is "Go with conditions" because ADR 0030 and the 16384 perf-window line are on an unmerged planner PR.
