# S-Region-Authoring: mark regions in the editor, bake them offline, reopen them at local detail
Status: active (owner, 2026-11-28)
Opened: 2026-11-28   Closes by: 2026-12-01   Owner look: 2026-12-01   R&D lane: none
Objective: region boxes marked in `planet editor` bake offline from the world's `WorldDefinition` and reopen together in `planet editor` at full local L detail inside the coarse globe, in one 10-minute owner run.
Out of scope:
- editing driver or source maps (World Authoring, later);
- background bake jobs inside the editor;
- regions that cross or touch a cube face edge (REND-063, refused);
- any change to B's erosion kernels;
- `docs/dev_screenshots/`.
## Exit items (at most 5)
| # | Item | Mode | Done when | Gate | Park | Lane | State | Result |
|---|---|---|---|---|---|---|---|---|
| E1 | `regions.toml` (WDEF-019) and `cargo xtask bake-region` (BUILD-078) | X | D1's format: `<world-dir>/regions.toml` with `--world`, `--regions <FILE>` with `--seed` (as `--bookmarks`); `format_version = 1`; each `[[region]]` has `name`, `face`, `tiles_per_edge`, `i0`, `j0`, `i1`, `j1`; one margin shared with E2; the command loads the world's `WorldDefinition` and `--items` (CON-05), bakes each box to `<regions-parent>/regions/<name>/`, a commit VIEW-069 opens | func: TEST-165 (2026-11-28) | lane-days 1.5 -> park, keep today's example | explorer | done 2026-11-28 | #1698; a 4x4 box bakes in 89 s |
| E2 | Editor regions (VIEW-077) | X | in `planet editor`: place, resize and delete named boxes snapped live to whole L tiles (2x2 to 4x4), the bounds and the face-edge margin drawn on the ground; every box the UI allows passes `RegionSpec::check`; saved to `regions.toml`; the panel derives `unbaked`, `ready` or `stale` from the commits; every ready region of the file reopens together in one run, later bakes keep earlier ones | func: TEST-166 (2026-11-28) | hot-swap days 0.5 -> reopen by the printed command; 2+ regions: lane-days 2.5 -> park, keep one | crusader | running | #1700 (one region) |
| E3 | The owner's end-to-end smoke | X | in 10 minutes: mark and bake a region, then a second one, reopen both in one editor run, fly across a coarse-to-local boundary | look: the smoke run, 2026-12-01 | look-unjudged 2026-12-01 -> park, keep today's | lead | open | run 1: one region, under 4 min |
| E4 | The process worked up to the close | M | the close report's process section (`/sprint-close`) gives every counted measure with P3's value beside it where one exists | look: the close report, 2026-12-01 | - | lead | open | - |
| E5 | P3's leftovers (owner, 2026-11-27 and 2026-11-28) | M | the M0-P2 files and `P4.md` in `docs/archive/<M>/` with their links; agentguard, nextid, GEMINI.md and CLAUDE.md's PR-body lines (BUILD-075) current; ADR 0128's `P3.md` erratum (ADR 0078); `tmp/g1*`, `tmp/g2*` binary and store dumps deleted (text, PNGs, fly1126 kept); parked RG worktrees pruned | func: spec-lint (2026-11-28) | lane-days 1 -> park, keep today's | crusader | done 2026-11-28 | #1694, #1696 |
## Decisions (at most 3 open)
| # | Decision | Lead's recommendation | Default | By | State |
|---|---|---|---|---|---|
| D1 | The `regions.toml` format | world folder, tile range, state derived | pauses E1 and E2 | 2026-11-28 | decided 2026-11-28: the owner's format, in E1 |
| D2 | Several ready regions open in one editor run (owner's smoke) | next sprint | next sprint | 2026-11-29 | decided 2026-11-28: in this sprint, E2 (owner) |
## Changes
- 2026-11-28 | file | the local draft of 2026-11-27 -> the sprint format | lead
- 2026-11-28 | Status | draft -> active | owner
- 2026-11-28 | E1 Done when | D1's format folded in | owner
- 2026-11-28 | E2 Done when | the margin shared with E1; state derived | owner
- 2026-11-28 | E4 Done when | the next-sprint clause dropped | lead
- 2026-11-28 | E5 Item, Done when, Park | proposed -> in, widened; 0.5 -> 1 lane-day | owner
- 2026-11-28 | E2 Done when, Park; E3 Done when; Objective | several regions reopen together (D2) | owner
## Close
- report: docs/milestones/S-Region-Authoring-report.md; signed off: -
