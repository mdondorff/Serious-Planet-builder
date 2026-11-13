# e1-label

Why: E1 (P1, lead 2026-11-12): every picture of the flight over the R3a commit is labelled with its known defect, "too many lakes; cause: basins of the starting surface (SPIKE-109)", so the reader judges the lakes with the cause in mind.

What: `planet_render::annotate::annotate_with_note` draws a note above the pose label (wrapped at spaces to the picture's width, the label's text scale); the label font gains the glyphs the note needs (`b j q w z : ; , ( )`; capitals draw as lower case, as before). `capture-stills` takes `--note TEXT` and draws it on every still (refused for any other scene). Goldens are never annotated (TEST-128 as before).

Requirement: TEST-128 gains the note bullet (MODIFIED). Source: lead 2026-11-12 (E1 item), 2026-11-13 (the label on the pose line).
