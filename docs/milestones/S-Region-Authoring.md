# S-Region-Authoring: mark regions in the editor, bake them offline, reopen them at local detail
Status: draft (lead, 2026-11-28): waits for the owner's `decision: open S-Region-Authoring`. Nothing starts before it. The pilot of ADR 0130 (proposed).
Opened: 2026-11-28   Closes by: 2026-12-01   Owner look: 2026-12-01   R&D lane: none
Objective: a region box marked in `planet editor` bakes offline from the world's `WorldDefinition` and reopens in `planet editor` at full local L detail inside the coarse globe, in one 10-minute owner run.
Out of scope:
- editing driver or source maps (World Authoring, later);
- background bake jobs inside the editor;
- regions that cross or touch a cube face edge (REND-063; `RegionSpec::check` refuses them);
- any change to B's erosion kernels (M-Erosion follows).
## Exit items (at most 5)
| # | Item | Mode | Done when | Gate | Park | Lane | State | Result |
|---|---|---|---|---|---|---|---|---|
| E1 | `regions.toml` (WDEF-019) and `cargo xtask bake-region --world <W> --regions <R> --name <N>` (BUILD-078) | X | the command loads the world's `WorldDefinition` and its `--items` (CON-05; not `single_planet`), bakes each named box to a commit that VIEW-069 opens, `base_id` covering the items | func: TEST-165 (2026-11-28) | lane-days 1.5 -> park, keep today's example | explorer | open | - |
| E2 | Editor regions (VIEW-077) | X | in `planet editor`: place, resize and delete named boxes snapped live to whole L tiles (2x2 to 4x4), with a ground overlay of the bounds and the face-edge margin shaded; saved to `regions.toml`; the panel shows `unbaked`, `ready` or `stale`; reopening a ready region | func: TEST-166 (2026-11-28) | hot-swap days 0.5 -> reopen by the printed `--commit` command; lane-days 2 -> park, keep today's | crusader | open | - |
| E3 | The owner's end-to-end smoke | X | in 10 minutes: mark a region, run `bake-region`, reopen it in the editor, fly across the coarse-to-local boundary | look: the smoke run, 2026-12-01 | look-unjudged 2026-12-01 -> park, keep today's | lead | open | - |
| E4 | The process worked up to the close | M | the close report's process section gives the combined plan's counted measures with P3's values beside them (rules without a check, most open decisions at once, defaults fired against paused, unprefixed messages that changed nothing, looks judged by their date, records present, planned against used lane-days, owner minutes); steps after the close are checked at the next sprint | look: the close report, 2026-12-01 | - | lead | open | - |
## Decisions (at most 3 open)
| # | Decision | Lead's recommendation | Default | By | State |
|---|---|---|---|---|---|
| D1 | The `regions.toml` format (a new file format, the owner's) | in the world folder beside its definition; one `[[region]]` per box: `name`, `face`, L tile range `i0 j0 i1 j1`; the panel state derived from the commits, not stored | pauses E1 and E2 (a format; ADR 0130 point 5) | 2026-11-28 | open |
## Changes
- 2026-11-28 | file | the local draft of 2026-11-27 -> the sprint format | lead
## Close
- report: docs/milestones/S-Region-Authoring-report.md; signed off: -
