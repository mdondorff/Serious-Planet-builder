# 0007 Authored-layer data model
Status: proposed
Date: 2026-10-10
Source: background report §6; constitution CON-09, CON-10

## Context
Authored content (stamps, splines, placed objects, masks) must coexist with a procedural base that will change. The owner decided (2026-10-06) that surviving generator-algorithm changes is a nice-to-have; "old data unsupported after a generator change" is acceptable if version pinning and freeze/bake exist.

## Options (with trade-offs)
The report lists strategies A to G (version pinning, absolute rasters, relative deltas, geo-anchored intent, constraints, freeze/bake, re-fit). Absolute rasters alone seam against a changed base; constraints turn generators into solvers; re-fit tooling is costly per change.

## Decision
A + C + D + F now: **version pinning, relative deltas, geo-anchored vector intent, freeze/bake.** Constraints (E) only for a few high-value intents later; re-fit (G) deferred.
- Authored items are stored separately from the base, as one text file per item or spatial cell (TOML/JSON, git-mergeable); binary payloads go in content-addressed blobs.
- Every item records: geo-anchor (lat/lon/height + heading), semantic type, parameters, the `generator_version` and definition hash it was authored against, and optionally a small expected-result snapshot (for example heights along a spline).
- Layer stack, bottom to top: base (pinned generator version), imported data, intent constraints, stamps, splines, placed objects, frozen/baked regions.
- The editor is a command pattern over the WorldDefinition with an append-only edit log (M6); runtime tiles are only invalidated.

## Consequences
- The schema carries its own version with tested migrations (CON-10), independent of `generator_version`.
- Easier: undo, git merges, cloud later. Harder: UI must prefer relative and semantic edits ("flatten to local average + 2 m") over absolute ones.
- Tests (A): anchor round trip ≤ 1 mm, items carry their versions, load fails clearly on a version mismatch unless pinned or frozen.

## Licences of adopted code or techniques
None. OpenUSD interop is a later export, not the core format.
