## Why
Briefing item 14 (inbox, A3 section 10): G-STORE asks for real serialized data of the build against the 40 GB budget. Only the field bake, kind 13 and the area and depth bytes had writers; the items the new chain adds (round records, routing, channel graph, basins, keys, border summaries) were estimates.

## What Changes
- `examples/landmass_scale <k> <threads> dump`: writes the raw state of a landmass build (round heights, routing, filled surface, channel graph, basins, labels) to `target/store_dump_k<k>/`.
- `cache/examples/store_measure <k>`: encodes the dump as real pack records (`PackWriter`, 208 km tiles) and reports bytes on disk per item, record headers and index entries included.
- STRM-078 added (streaming-cache).

## Impact
- Code: two examples. No store format is decided: the encodings are measurement encodings; layouts are the proposal the bytes are counted for.
- Not in this change: a reader for the records, edit overrides (no serializer), temporary and replacement space (no code), kind 13 and the field bake on the new chain.
