## Why
SPIKE-19 (spikes/morph-refusal/results/results.md): the window aborted ("plan for frame 35: invalid LOD parameters ... morph interval ... is empty") when F4 had been pressed twice and a plan refusal for the coarse view arrived after the switch was off. The cause is in F4: `LodParams::coarse` takes the largest factor `validate` accepts, with no margin, but the plan adds the height extent of a tile's neighbourhood to the morph interval's start and is a hair stricter (level 4: start 0.95077 of the end against 0.95). Owner-facing: the editor window ended mid-session on a steep-enough world.

## What Changes
- `LodParams::coarse` keeps the plan's own start (`morph_interval_worst`: `split(l) + 2 x extent(r (1 + H / R), H)`, H = `max_height_m`) plus a margin of 1 % (`COARSE_MARGIN`) at every level where the base parameters meet that start and the node radius is at least half of H (`COARSE_WORST_CASE_RADIUS_FACTOR` = 0.5, measured: 4 and 1 left refusals at levels 8 to 10 in a pose sweep from 20 km; 0.5 covers them and keeps a coarse factor of about 1.7 at 1080p; where the global height bound is the only bound there is), and the node-radius rule with the margin at every level. `validate` is unchanged: applying the plan's start at every level would make the window's own base LOD invalid at deep levels, where the plan reads the tiles' own tight height bounds. The plan's check stays the assertion; nothing is clamped.
- `PlanPipeline::begin_frame` drops, and counts (`dropped_plan_errors`), a plan failure whose request carried other LOD parameters than the frame's, instead of returning it; the failure of the frame's own request is returned as before. `PlanFailure` carries the request.
- VIEW-032, the spike's failing test (`the_coarse_lod_the_window_derives_is_never_refused_by_the_plan_it_makes`) and a pipeline test; the Tracked frame-35 flake is removed from `docs/status.md`.
- Backspace in the editor mode never deletes an edit (opus-planner, owner's call, easy to reverse): it takes the last draft point back; Delete deletes (VIEW-031 amended in its change).

## Classes and bounds (ADR 0035)
No field and no shader value is added; the coarse factor of the production parameters falls from about 2.6 to about 1.7 (VIEW-014 amended: the "about 2.6" becomes "about 1.7"); no golden moves (F4 is the editor window only).

## Out of scope
Why a field's neighbourhood bound is global at coarse levels (the field's `height_bounds`), a cache for it, or a change of the plan's rule (nothing is clamped).
