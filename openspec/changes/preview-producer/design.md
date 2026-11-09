## Producer
- `previewjob::PreviewWorker::new(base_source)`: one thread, named `level-1 preview`, ended when the worker is dropped (the same pattern as `hoverpick`). The thread builds the base from `base_source` on its first ask (minutes at n = 3,072, seconds at 768), then loops: take the newest ask (older queued asks are dropped), run `PreviewBase::preview`, post `(ticket, Result<PreviewData, String>)`.
- `request(ticket, edit)` never blocks; `poll()` returns the newest finished answer. The window takes an answer with `accept_preview(ticket, data)` (VIEW-067): a ticket that is no longer the newest is dropped there too.
- The edit: the stamps of the world definition (`footprint_items_cap` over the whole sphere, every layer in the order the layer list applies them); a held handle drag re-asks when `footprint_hash` of that list differs from the last ask's.
- `PreviewData` from a `Preview`: for every trunk cell with a receiver, the line from the cell's direction to its receiver's, the watershed (`outlet_of`), whether the receiver or the watershed differs from the base's, the discharge.

## Flag and the debug view (CON-18)
`--preview-l1` (editor only, refused elsewhere like `--erosion`): without it no thread starts. `--preview-grid N` (default 768, 8 to 3,072).

## Tier C
`gpu_preview_overlay`: a 256 x 256 gradient frame composed by `UiPainter::offscreen` with no UI, and again with a `UiFrame` tessellated from `draw_preview_overlay` over a small `PreviewOverlay`; every pixel outside the union of the lines' and the label's rectangles (with a margin for coverage) equals the plain frame; the lines' pixels differ.
