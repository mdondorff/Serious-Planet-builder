# 0066 Authored items: reach, region summaries, stages, selection bounds, item files and the command log
Status: accepted (points 1 to 5 and 7: opus-planner under the owner's delegation of 2026-10-12; point 6: owner, 2026-10-19, with the item store interface, grouped brush strokes and the revisit trigger; drafted by sonnet-m4). Decides ADR 0021 points 2, 5, 6 and 8; amends ADR 0007 (the cell is no longer the invalidation unit; the command log is decided), ADR 0018 and ADR 0023 points 2 and 3 (query kinds, material, format tag; CON-23); points 2 and 5, and the key role of point 6's item hash for stamps, amended by ADR 0067, 2026-10-19.
Date: 2026-10-19
Source: ADR 0021 leanings; ADR 0007; ADR 0018; ADR 0022; ADR 0023; SPIKE-13 (`spikes/edit-latency/results/results.md`, variant D); M6 editor plan, row E0

## Context
- ADR 0021 left points 2, 5, 6 and 8 open; ADR 0007 deferred the command log. Nothing authored is persisted yet, so each choice is cheap now.
- SPIKE-13: cell invalidation re-keys 59 % (2 km altitude) to 83 % (1 m) of the resident tiles for a 150 m stamp, of which 36 to 2,812 touch it; the find costs 0.6 to 3.7 ms by footprint, 260 to 820 ms by key.
- SPIKE-13: widening every tile's height bounds by a constant 120 m made planning fail at 200 m and 1 m altitude (empty morph intervals).
- ADR 0023 point 2 gates item files on 128-bit item ids and hashes (ADR 0021 point 4, decided).
- M6 exit criteria: edits visible within 1 s, undo over 1,000 operations, item files merge cleanly in git. Out of scope: global-scale authoring, the blob store.

## Decision
1. **Reach (ADR 0021 point 2) is per item.** `reach_m` is the distance from the anchor beyond which the item changes no output of any stage. M6 limit: one cell edge. A load or a radius edit that leaves an item over the limit is refused with the item id and the limit. Rejected: a per-layer maximum (re-keys the tiles of every small item).
2. **The invalidation unit is the item footprint, not the cell.**
   - A tile reads a layer through one query entry, kind 3: `body u32 | layer (u32 length + UTF-8) | tile raw id u64 | halo f64 bits` (halo as a fraction of the tile edge, ADR 0022, canonical at `-0.0`).
   - An item is returned when a bounding-sphere test of its reach against the tile and halo passes in f64 with `libm`, with a fixed relative margin. False positives change a key, never an output.
   - Result material, kind 3: items in application order `(priority i32, id bytes big-endian)`, each `id 16 B | content hash 16 B` (the anchor is inside the content hash, so no separate direction bits), under the context `planet-builder footprint result v1`; an empty result is that context over empty input.
   - Cells stay the lookup index; the query margin `ceil(reach_m / cell edge) + 1` stays a read-only parameter. Format tag 3.
3. **Region summaries (point 5) replace the whole-body read.** A tile covering more cells than a limit (spec value) records, as kind 4 entries `body u32 | layer | level u8 | node raw id u64`, the smallest set of Merkle quadtree nodes covering its cells.
   - Cell hash: BLAKE3-128 under `planet-builder cell hash v1` over the cell's item rows in application order.
   - Node hash: under `planet-builder summary node v1` over `level u8` and the four child hashes in fixed order; an empty subtree is the same function over empty children, so the level is inside it.
   - An edit rewrites one node per level.
4. **Authored layers are stages after the base, keyed as a chain (point 6).**
   - The 128-bit stage key covers the stage version, the layer, its index in ADR 0007's stack (not its priority), the previous stage's key for the same tile (the base tile key first) as the source key, and the layer's query entry.
   - Editing a layer re-keys that layer's tiles and the layers above it. Inserting a layer into the stack bumps the stage version of every layer above it.
   - Precondition: a stage reads the base only as a height field at points, and the base key is field-wide (true for the global bake and the eroded field). A per-tile base field needs neighbour keys and a new ADR.
   - `inputs_hash_for` stays for the base field.
5. **Height bounds (point 8) follow reach.**
   - An item declares `max_delta_m ≥ 0`, a bound on |height change| anywhere in its reach.
   - A tile's bounds are the base bounds widened on both sides by the sum of `max_delta_m` of the items its query returned (f64, summed in application order, rounded up).
   - The `SelectionCache` key gains the 128-bit result hash of the tile's query; an edit invalidates bounds and volumes only of the tiles its footprint find returns.
   - E1 checks on the steep field at levels 16 to 18 that no morph interval empties; the fallback is the bump at the tile's nearest point to the anchor.
6. **Item files.**
   - One TOML file per item, `<ULID>.toml` (26-character Crockford form, hash over the 16 bytes), in the body's item directory. Adding an item adds a file, so concurrent additions never conflict. Rejected: one file per body or per cell.
   - The ULID is made by the editor command from an injected clock and entropy, never by generation.
   - The writer emits a fixed key order, one key per line, floats in shortest round-trip form (ADR 0007); the loader refuses unknown keys and non-finite numbers.
   - Fields: `schema_version`, `id`, `body`, `surface`, `layer`, `priority` (integer), `kind`, `anchor` (latitude, longitude, height, heading), `reach_m`, `max_delta_m`, the kind's parameters, `base_hash`, the generator and stage versions, optional `name` and `notes`.
   - The content hash is BLAKE3-128 under `planet-builder item hash v1` over the parsed form (WDEF-003): every field except `name` and `notes`, f64 as bit patterns with `-0.0` made `0.0` at load. Versions are inside the hash, so a re-bake under a new generator version re-keys every item, as ADR 0007 intends.
   - Storage sits behind an item store interface; loose files are its first backend. The id and the content hash do not depend on the backend, so a packed or database backend needs a one-time conversion and no re-key.
   - A brush item groups many strokes in one area; stroke data beyond the TOML parameters goes to content-addressed blobs (ADR 0007), never one file per stroke.
   - Revisit trigger: more than 20,000 items in one body, or an item load over 2 s on Rasierklinge; the choice of a packed or database backend is then made from measurements.
7. **The command log is not persisted.**
   - Every command writes its item files through (undo and redo too); git is the history. A drag is coalesced into one command (E4).
   - The log is an in-memory stack for session undo and redo, at least 1,000 operations, each restoring the exact previous definition hash. Cross-session undo is not offered.
   - Rejected: a persisted log (duplicates git, conflicts on every merge, replays only under the generator versions it was made with).

## Consequences
- Spec deltas by step: WDEF-004, WDEF-005, the stage key requirements, the item schema and AUTH-001 to AUTH-003 with the first change that reads item files (E2); AUTH-004 (in-memory log) with E4; the selection cache and bounds with E1.
- ADR 0021's status line and ADR 0007's invalidation-unit sentence are marked when this ADR is accepted.
- A reach or `max_delta_m` declared too small gives stale tiles or wrong bounds: the contract test of ADR 0021 point 7 also asserts that a stage's output is unchanged beyond `reach_m` and within `max_delta_m`.
- Footprint reads need a per-tile memo invalidated by an item revision counter; the counter enters no key.
- Easy to reverse: the command log, the summary limit, the item store backend. The reach limit sits in the query margin, so changing it re-keys tiles.
- Not decided here: the blob store, blending at frozen-region borders, the expected-result snapshot policy, imported-data provenance.

## Licences of adopted code or techniques
None.
