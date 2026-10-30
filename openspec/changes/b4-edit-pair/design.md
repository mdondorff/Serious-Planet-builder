## Context
- REND-033: a draw plan carries at most 32 preview stamps and flags stale or edit-named nodes; REND-045: the delta applies only to a tile at the oldest state of the drag, other stale tiles draw as they are.
- Existing evidence: `crates/render/tests/gpu_preview.rs` (tier C, generated world): the preview over stale tiles is close to the exact tiles in two views at one grey threshold; a stroke chain and 32 stamps are drawn. The edit-latency scene prints a preview cost table (TEST-043).
- Owner report (2026-10-30): a dragged edit looks systematically different from the same edit at rest, also on lowland, not only at a large relief item.

## Decisions (proposed)
1. The pair is one pose drawn twice through the managed loop on the real store: the edit is made through the same call as the editor (`ManagedLoop::edit` with the transient preview), frame A is the first frame after the edit with the preview over the stale tiles, frame B is the frame after every refreshed tile has landed (the settle of the edit-latency scene).
2. The comparison is a pure function over two RGBA buffers and the unedited frame: pixels changed by the edit (unedited against B), pixels left different after the preview (A against B) over 3 grey levels, their ratio against 1/20, the mean and largest channel difference, and the bounding box of the differences. It is unit-tested on planted pairs (equal, a shifted edge, a tint) in tier A.
3. The cause class comes from drawing the pair in more than one view: normals (shading), relief or height contours (shape), the lit frame with the horizon off (everything but shadows) and with it on (shadows added). The report names the first view whose ratio is over the bound.
4. Sites: one lowland and one hill site of the real store at 200 m and 2 km altitude, sun 20 degrees; the sites are named in a poses file (TEST-050) so the owner can ask for others.
5. Output: `target/review/edit-pair/<site>-<altitude>-<horizon>/` with A, B, the diff (enlarged, TEST-038 style) and `pair.md`; the package (B4-5) links `pair.md`.

## Risks
- The refresh on the real store takes seconds to a minute; the scene waits for the plan and tile jobs to settle and says NOT RUN if the settle times out.
- With the chunk stage still in the code the exact tiles may differ for reasons outside the preview; the scene runs with the stage off (the default).
- A GPU-timing or lock-screen stall must not read as a difference: the scene refuses to run when the session is locked (the NOT RUN rule of the lock change) and compares only frames, not times.

## Open questions for the planner
- A requirement ID for the pair scene and one for the pure comparison.
- Whether the lit pair uses the look default or the sample driver look.
