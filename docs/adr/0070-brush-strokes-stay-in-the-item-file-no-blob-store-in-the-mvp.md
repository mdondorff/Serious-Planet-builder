# 0070 Brush strokes stay in the item file; the blob store is specified and deferred
Status: accepted (owner, 2026-10-20: inline TOML strokes now, the blob store kept for a later extension; drafted by sonnet-explorer). Extends ADR 0066 point 6 (grouped brush strokes, item files) and ADR 0069 (point lists); decides the "blob store" ADR that ADR 0007 deferred to M6, for the MVP.
Date: 2026-10-20
Source: M6 plan row E6; ADR 0007 (storage: binary payloads in content-addressed blobs outside git); ADR 0066 point 6; ADR 0069; CLAUDE.md (no binary over 1 MB without an ADR); WDEF-003, WDEF-010, AUTH-005

## Context
- A brush (raise, lower, smooth, flatten; radius and strength in metres) is painted as strokes. ADR 0066 point 6: one brush item groups many strokes of one area, never one file per stroke; stroke data beyond the TOML parameters goes to content-addressed blobs.
- Item files are TOML, one per item, git-mergeable; an item's reach is at most one cell edge (40 km); schema 2 already holds an ordered point list (3 to 64 points for a region).
- A stroke is a polyline plus two scalars. A freehand stroke is simplified to a point spacing of about a quarter of its radius, so a long drag is a few dozen points.
- A stage reads the height field at points only and keys by chain (ADR 0066 point 4); the item hash covers every field (WDEF-003).

## Decision
1. **No blob store in the MVP.** A brush item holds its strokes in its TOML file (schema 3, an ordered list `[[strokes]]`; each stroke has `kind` (`raise`, `lower`, `smooth`, `flatten`), `radius_m`, `strength_m`, an optional `target_m` (flatten) and its own `[[strokes.points]]`). The strokes are in the item hash in file order; the file order is the application order within the item.
2. **Caps.** At most 64 points per stroke, 64 strokes and 4,096 points per item, and the item's reach covers every point plus its largest radius within one cell edge. A put over a cap is refused naming the cap; the file stays under about 350 KB.
3. **Grouping by area is the editor's job (E4/E6 commands), not the world definition's.** A new stroke joins the open brush item of its layer whose reach disc contains the stroke and which has room; otherwise it starts a new item with the same `group`; a stroke over 64 points splits into consecutive strokes sharing a point. The definition sees independent items.
4. **The blob store is specified here and not built.** If a stroke list outgrows the caps (trigger: a body whose brush items total over 5 MB of TOML, or an owner request for freehand detail finer than the caps allow), an item replaces its inline strokes by `blob = "<32 hex>"`:
   - location: `<body items dir>/blobs/<first two hex digits>/<hash>.bin`, outside git by the body's `.gitignore` and synchronised with the project like the base bake;
   - format: magic `PBSB` and format version `u16`, stroke count `u32`, then per stroke `kind u8`, `radius`, `strength`, `target` as f64 bits (little endian, `-0.0` as `0.0`), point count `u32`, points as latitude and longitude f64 bits;
   - hash: BLAKE3-128 of the whole file under the context `planet-builder stroke blob v1` (derive-key mode), the file name; the item hash covers the blob hash, not the bytes;
   - size: at most 1 MiB per blob (CLAUDE.md), 4,096 strokes; a missing or corrupt blob refuses the load naming the item and the hash;
   - collection: blobs no item names are removed by an explicit command, never at load.
   An inline stroke list and a blob are two spellings of one list: a conversion changes the item hash (the blob hash replaces the points) and so re-keys the tiles once, nothing else.
5. **Rejected.** One file per stroke (the owner's file-count concern). Blobs now: a second storage layer (location, sync, collection, missing-file handling, a binary format that cannot be diffed or merged) for lists that fit a TOML file.

## Consequences
- Specs: AUTH-005 gains schema 3 (`[[strokes]]`); WDEF-013 (the brush item and its caps), GEN-030 (the stroke stage) come with the change `brush-stage`; the stage semantics are in its design, not here.
- Merge: two sessions adding strokes to one item conflict as a TOML hunk; adding strokes to different items never does. The editor starts a new item rather than appending when the open item's last change is not its own session's.
- Hard to reverse: the blob format of point 4 (nothing is built, so nothing is stored); the schema 3 spelling is additive and schema 1 and 2 files load unchanged.
- Easy to reverse: the caps (they sit in the put's validation and the editor's grouping).
- Cost: a sample evaluates only strokes whose reach disc it lies in (a bounding-sphere test per stroke), so a 64-stroke item costs a few distance tests per sample away from its strokes.

## Licences of adopted code or techniques
None.
