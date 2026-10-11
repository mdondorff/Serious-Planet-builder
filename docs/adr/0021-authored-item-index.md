# 0021 Authored-item index: extent, layers, widths, staging
Status: proposed
Date: 2026-10-11
Source: spec-guardian and numerics reviews of m3-cell-index (2026-10-11); ADR 0006, ADR 0007 (cell index "fixed in the M3 world-def change"), proposed ADR 0018; constitution CON-05, CON-06, CON-08, CON-23

## Context
The M3 cell index (WDEF-005) is in memory only: items are `(body, id) -> (content hash, unit direction)`, filed in tangent-warp cells of about 40 km, and a generator reads whole cells plus a margin ring; the hash of every cell read enters its read set. Nothing is persisted yet, so every choice below is cheap to change now and expensive once item files exist (ADR 0007: items are TOML files in git; M6). The reviews found five points the code only scaffolds.

## Questions and leanings
1. **Cell level.** ADR 0007 says one fixed level per body. The code derives it from the body's radius and re-files the items whenever the radius node changes (a test crosses the boundary), so items are never stranded. Alternative: a stored node `body.N.cell_level`, default from the formula, so a radius edit never moves cells. *Leaning:* keep the derived level; revisit if radius edits become frequent in the editor.
2. **Extent.** An item is filed by its anchor point only, but a stamp or spline reaches further. *Leaning:* every item declares a `reach_m` (a per-layer maximum is enough to start); a reader queries a margin of `ceil(reach_m / cell_edge) + 1` cells, and the margin is an output parameter of the key (ADR 0006). The code has the margin parameter (`query_tile`, `with_margin`); the reach declaration is M6.
3. **Layers.** One item set per cell means a spline edit re-keys stamp-stage tiles. *Leaning:* the query is `(body, layer, cell)` with the layer in the recorded path (ADR 0018 already puts the layer id in the query parameters).
4. **Widths.** Item ids are `u64` and content hashes `u64`; ADR 0007 wants ULIDs (128 bit) and ADR 0018 wants 128-bit query result hashes over item content hashes. *Leaning:* ids and content hashes become 128-bit when item files are defined; the in-memory 64-bit keys stay for hot paths (ADR 0018). Changing the width before files exist needs no migration.
5. **Coarse tiles.** A tile that would touch more cells than a limit now reads the whole body (exact, never stale, but any item edit anywhere re-keys it). *Leaning:* region summary hashes (a Merkle quadtree over cell hashes with a fixed empty hash per level), so a coarse tile records one entry and is invalidated exactly by edits inside it.
6. **Staging.** Today a spatial field folds item reads into the height key of the base tile. ADR 0007 makes authored layers generation stages after the base, keyed by the keys of the base tiles they read (CON-08, ADR 0006 chain). *Leaning:* the staged-pipeline change introduces stage keys (`upstream keys + read set + margin`) and authored stages use them; `inputs_hash_for` remains for fields that need a per-tile read set inside one stage.
7. **Declared versus recorded reads.** A tile's read set is declared at plan time (`inputs_hash_for`); `height(dir)` has no context to record against. A generator that reads more than it declares would key wrongly. *Leaning:* a contract test that runs generation under a recording context and asserts the reads are a subset of the declared ones, required before the first spatial generator ships.
8. **The LOD selection cache.** `SelectionCache` keys its per-tile bounds on the field-wide inputs hash, and the bounds come from `field.height_bounds`. A field that reads cells and whose heights (so bounds) depend on items would keep stale bounds after an item edit. Nothing ships such a field yet. *Leaning:* the first spatial generator either reports bounds that do not depend on items (a safe over-estimate) or the cache key gains the per-tile inputs hash.

## Decision
None yet: this ADR records the open points and the leanings so M6 and the staged-pipeline change start from them. Points 2, 3 and 4 must be decided before item files exist; 5 and 6 before the first authored stage.

## Consequences
- Nothing built so far is wasted: the cell index, the margin parameter, the whole-body fallback and the per-tile read sets are the base each leaning builds on.
- The owner decides at the M6 start, or earlier if item files are wanted sooner.

## Licences of adopted code or techniques
None.
