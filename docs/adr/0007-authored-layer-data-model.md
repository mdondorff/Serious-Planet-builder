# 0007 Authored-layer data model
Status: accepted
Date: 2026-10-10
Source: background report §6; constitution CON-09, CON-10

## Context
Authored content (stamps, splines, placed objects, masks) must coexist with a procedural base that will change. The owner decided (2026-10-06) that surviving generator-algorithm changes is a nice-to-have; "old data unsupported after a generator change" is acceptable if version pinning and freeze/bake exist.

## Options (with trade-offs)
The report lists strategies A to G (version pinning, absolute rasters, relative deltas, geo-anchored intent, constraints, freeze/bake, re-fit). Absolute rasters alone seam against a changed base; constraints turn generators into solvers; re-fit tooling is costly per change.

## Decision
A + C + D + F now: **version pinning, relative deltas, geo-anchored vector intent, freeze/bake.** Constraints (E) only for a few high-value intents later; re-fit (G) deferred.
- **Storage:** authored items are stored separately from the base, one **TOML** file per item, git-mergeable, in a per-body store (ADR 0001). Binary payloads go in content-addressed blobs outside git (store and limits decided by an M6 ADR; CLAUDE.md's 1 MB rule).
- **Spatial index:** items are indexed by spatial cell = `TileId` at one fixed level per body (a level of about 40 km cells, e.g. level 8 at Earth radius, fixed in the M3 `world-def` change). A generator's spatial query reads cells, and the hashes of the cells it read, including empty ones, enter its read set (ADR 0006). The cell is the invalidation unit for authored edits.
- **Every item records:** a stable item id (ULID); `BodyId` and the reference-surface id; geo-anchor (lat, lon, height above the reference surface, heading to true north on that body); semantic type; parameters; layer and priority; the **base hash** it was authored against (hash of the procedural base definition, excluding authored layers) plus the version of each generator it depends on; optionally a small expected-result snapshot (for example heights along a spline).
- **Hashing and text:** keys hash the parsed, canonical form (WDEF-003), never the file bytes, so formatting, key order and merges do not change keys. Floats are written in shortest round-trip form (Rust's `f64` formatting), never with fixed decimals.
- **Layer stack, bottom to top:** base (pinned generator version), imported data, intent constraints, stamps, splines, placed objects, frozen/baked regions. Within a layer, items apply in order (priority, item id), never file, load or time order (CON-14).
- **Authored layers are generation stages** after the base (CON-08): a relative delta reads base output around its anchor with a halo, and its outputs are keyed like any staged output, including the keys of the base tiles it read (ADR 0006).
- The editor is a command pattern over the WorldDefinition (M6); runtime tiles are only invalidated.

**Deferred to M6, named so nothing builds around them early:** the item files are the persistent truth and git is their history; whether the command log is persisted beyond session undo is decided then. The blob store ADR. Blending at frozen-region borders against a changed live base. Tolerance and warn-or-fail policy of expected-result snapshots. Provenance and licence records for imported data (CON-27).

## Consequences
- The schema carries its own version with tested migrations (CON-10), independent of `generator_version`.
- Easier: undo, git merges, cloud later; an edit invalidates only the tiles whose cells it touches; adding an item does not make other items look stale (they record the base hash, not the whole definition hash). Harder: UI must prefer relative and semantic edits ("flatten to local average + 2 m") over absolute ones.
- M3 builds `world-def` with the cell index, canonical hashing and base hash above, even though authored items arrive in M6.
- Tests (A): anchor round trip ≤ 1 mm through TOML; items carry their versions; load fails clearly on a version mismatch unless pinned or frozen; reformatting or reordering keys in an item file leaves its hash unchanged; overlapping items give the same result in any load order; adding an item invalidates only tiles whose read set includes its cells.

## Licences of adopted code or techniques
None. OpenUSD interop is a later export, not the core format.
