# Change: f9-items

## Why
- The owner's F9 in the editor failed: "the repro files were not written: recording the world: the definition has authored items, which this file format cannot carry yet" (`LoadError::ItemsNotSupported`). The definition text has no items, so no bundle of an edited world could be written, and the camera path was lost with it.

## What changes
- A bundle's world section carries each authored item as its item-file TOML (`world_item` lines of the bundle text, after `world_toml`); the definition text stays the base without items. An old bundle reads as before; a bundle with items needs this build.
- `reproworld::record_world` writes the base text and the item texts and refuses a record that does not rebuild to the definition the frame was planned from (an edit in flight: the message says press F9 again). `rebuild_world` puts the items into the parsed base; `cargo xtask repro` and `test-render --repro` use it.
- F9 writes `frame-<n>-path.txt` first; a bundle that fails is reported with the path file's name.
- VIEW-024 modified (items in the bundle, the path first).

## Tests
- `reproworld` (tier A): a definition with a stamp, a line and a brush stroke rebuilds to an equal definition from the recorded texts; an item list that is not the definition's is refused with both hashes.
- `terrain::a_world_bundle_records_...` (tier A): item texts with newlines and backslashes survive the bundle text.

Expected golden changes: none.
