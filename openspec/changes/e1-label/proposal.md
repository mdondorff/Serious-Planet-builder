# e1-label

Why: E1 (P1, lead 2026-11-12): every picture of the flight over the R3a commit is labelled with its known defect, "too many lakes; cause: basins of the starting surface (SPIKE-109)", so the reader judges the lakes with the cause in mind.

What: `planet_render::annotate::annotate_with_note` draws a note above the pose label (wrapped at spaces to the picture's width, the label's text scale); the label font gains the glyphs the note needs (`b j q w z : ; , ( )`; capitals draw as lower case, as before). `capture-stills` takes `--note TEXT` and draws it on every still (refused for any other scene). Goldens are never annotated (TEST-128 as before).

And the opening route (lead, 2026-11-13, option b): no stored bake matches the R3a commit's world under the current code (`global16384`, `-m9c14`, `-m9r2` refuse it), and a new world bake waits for the owner. `--commit DIR` opens a commit in the viewer and in `capture-stills` over the production base computed in memory (`WorldTerrain::open_commit_in_memory`: the composition the G-RT fixture used, now a production route; `open_fixture` keeps its own note for the perf scene), through the normal key check; the log, the title and every still say so.

Requirements: TEST-128 gains the note bullet (MODIFIED); VIEW-069 (new, lead 2026-11-13). Source: lead 2026-11-12 (E1 item), 2026-11-13 (the label on the pose line).
