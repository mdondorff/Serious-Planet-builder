# 0117 Driver regions (rainfall, temperature, lithology, uplift) are a world-level table, not tile items
Status: proposed (crusader, 2026-11-11; CON-10: the architect decides, then the owner for the schema)
Date: 2026-11-11
Source: briefing item 16 (preview level 1); A3 section 11.1; ADR 0066, 0069, 0070, 0076 point 7; constitution CON-09, CON-10, CON-29; `docs/design/driver-regions.md`

## Context
- Preview level 1 for rainfall and temperature regions exists (`bake::preview::ClimateRegion`: a cap of `reach_m` round a direction, a rain factor and a temperature offset, faded to nothing at the rim), but the editor cannot create one: `world-def` has no such item kind.
- An item's reach is at most one cell edge, 40 km (`Item::validate`, ADR 0070), and the tile reach index (ADR 0066) is built on it. A climate or lithology edit of a mountain belt or a continent is hundreds to thousands of km.
- ADR 0076 point 7: authoring acts on drivers, softly and feathered; it never paints a type.
- A3 section 11.1 gives the three edit kinds the same rows (L0 the driver map, L1 H0 or a coarse carve, L2 the dirty catchments); their stages (H0 climate, K3 lithology, K4 uplift) read the whole planet, not tiles.
- A new item kind or table is a schema change (CON-10): its own version and a tested migration.

## Decision
1. Driver regions are stored in a world-level table of the WorldDefinition file, not as tile items: `[[driver_region]]` entries, each a cap (anchor, `reach_m` up to a hemisphere) with a feather and one driver term. The tile reach index and ADR 0070's 40 km cap are untouched.
2. Four term kinds in the first version: `rainfall` (a factor), `temperature` (an offset in K), `lithology` (a K multiplier, a hardness class), `uplift` (an offset in m/Ma). A region carries exactly one term; overlapping regions combine in table order (factors multiply, offsets add), each feathered by a smoothstep of the distance to its rim.
3. The table is part of the keys of the stages that read it: H0 climate reads rainfall and temperature regions, K3 lithology, K4 uplift; their identity is the hash of the parsed regions that touch the stage (WDEF-003 form: parsed, canonical, `-0.0` is `0.0`); a world without regions has the identity of today (nothing moves).
4. The schema is bumped by one version with a migration that adds the empty table; fixtures of the older version load (CON-10).
5. Editor commands put, move, resize and remove a region with undo (the command log, ADR 0066): level 0 draws the driver map re-evaluated with the region over the affected area (GEN-122, 125, 130 maps through the overlay path), level 1 is the existing preview for climate (`ClimateRegion` is built from the table) and the coarse carve for lithology and uplift when item 15 and GEN-153 land.
6. Preview and authority: the preview reads the table and is non-authoritative (CON-07); level 2 re-solves the dirty catchments through the normal stages, whose keys changed by point 3 (CON-29: incremental equals fresh).

## Consequences
- Climate and driver edits become authorable at continent scale without changing the tile item machinery; the cost is one more world-level input to four stage keys.
- The owner must accept a schema version (CON-10); no stored world changes until a region is authored.
- Not decided here: the editor's tool UI, the term units beyond those named, regions on bodies other than the planet, class paint (it acts on drivers through ADR 0089 decision 8 and is its own item).
