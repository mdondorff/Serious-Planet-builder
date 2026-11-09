# preview-producer (VIEW-068, briefing item 16)

## What
- A worker thread of the editor window (`planet::previewjob`) that builds the resident base of a preview session once (`PreviewBase`, from the default class source at `--preview-grid N` cells a face edge, default 768) and answers the newest ask only: the edit is the stamps of the definition, the answer is a `PreviewData` for `planet_editor::preview_overlay` (VIEW-067).
- The flag `--preview-l1 [--preview-grid N]` of the editor: off, nothing runs and nothing is drawn (the debug view of CON-18). On, a held handle drag asks for a preview each time the stamps' footprint hash changes; the accepted preview is drawn over the planet and removed when the edit is committed or undone (VIEW-067).
- Tier A: the worker with a fake source (newest ask wins, a stale answer is dropped, an error is reported and does not stop the worker), the conversion of a `Preview` into lines, the flag's parsing.
- Tier C (VIEW-068's pixel assertion, `crates/app/tests/gpu_preview_overlay.rs`): a frame composed through the egui path with the overlay on against off: the pixels differ only inside the lines' and the label's rectangles, and differ somewhere; not a golden.

## Not in this change
- The coarse-carve rows, level 2, the owner's look (tier D at the prototype review).
- The base at n = 3,072: the default grid is small enough to answer in about a second; the 3,072 latency is TEST-099 (unauthorised timing).
