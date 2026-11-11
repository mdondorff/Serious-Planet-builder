# Driver regions: the world-definition design note for ADR 0117 (proposed)
Status: proposal for the architect (crusader, 2026-11-11). Read with ADR 0117, A3 section 11.1 and `crates/world-def` (`item.rs`, `region.rs`, `lib.rs`).

## What exists
| Piece | Where | State |
|---|---|---|
| Level-1 preview of a climate region | `generators::bake::preview::{ClimateRegion, PreviewEdit, PreviewBase}` | built, active (AUTH-007 to AUTH-010, GEN-157) |
| The overlay and the worker (`--preview-l1`) | `editor::preview_overlay`, `planet::previewjob` | built, active (VIEW-067, VIEW-068) |
| Level-0 driver maps | `generate --lithology-preview`, `--uplift-preview`, `--climate-preview` | built, CLI only (GEN-122, 125, 130) |
| Items with a polygon (regions of a surface type) | `world-def::region`, item kind `region` (WDEF-012) | built; reach at most 40 km |
| An item or table for climate, lithology, uplift | none | this note |

## The table (schema)
```toml
[[driver_region]]
id = "01J..."          # ULID, as an item id
body = "planet"
kind = "rainfall"      # rainfall | temperature | lithology | uplift
lat = 12.5             # anchor, geo-coordinates on the reference surface (CON-09)
lon = -40.0
reach_m = 1500000.0    # cap radius along the surface, up to a hemisphere (10,007,000 m on the Earth-size sphere)
feather = 0.5          # the fraction of the reach over which the term fades to nothing at the rim
factor = 1.8           # rainfall: multiplier; lithology: K multiplier
offset = 0.0           # temperature: K; uplift: m/Ma
priority = 0           # order in the table: lower first, ties by id
```
- Exactly one of `factor` or `offset` per kind (rainfall and lithology use `factor`, temperature and uplift `offset`); the loader refuses the other and unknown keys (the item loader's rule).
- Hash and keys read the parsed form (WDEF-003): fields in a fixed order, floats in shortest round-trip form, `-0.0` as `0.0`, the id and the notes out of the hash as for items.
- Bounds: `factor` in 0.05..20, `offset` of temperature within +-60 K, of uplift within +-10 m/Ma; a region of a hemisphere or more is refused (a whole-planet change is the preset's job).

## How the stages read it
- A weight of a region at a direction is `1 - smoothstep(reach * (1 - feather), reach, d)` for a distance `d`; the term is `1 + w (factor - 1)` for a factor, `w * offset` for an offset. Combined in table order. All f64, no transcendental beyond what `smoothstep` and the arc distance use (`libm`, CON-14).
- H0 climate takes the rainfall and temperature terms at each H0 cell; K3 lithology and K4 uplift take theirs at each regional cell. The identity of a stage's input is the hash of the regions of its kinds whose cap meets the stage's domain (a region elsewhere changes no key).
- `ClimateRegion` of the preview is built from a `rainfall` and a `temperature` region at the same anchor (the pair the preview already takes), or one of each kind separately: the preview takes a list.

## How the editor reads it
- Level 0: the driver map of the kind (the GEN-122, 125, 130 map code) evaluated over the region's bounding cap with the table including the draft region, drawn as a translucent equirectangular overlay (CON-18 debug view); a preview field with its own id (CON-07).
- Level 1: for climate, `PreviewEdit::climate` from the table; for lithology and uplift, the coarse carve row of A3 11.1 (waits for item 15 and GEN-153).
- Commands (E-series): put, move, resize, set term, remove; each is one undoable entry of the command log with the region's before and after text.

## What CON-10 needs
1. The schema version of the WorldDefinition file goes up by one; the migration from the previous version adds the empty table (a fixture of the old version loads and equals a new world with an empty table).
2. The parser refuses what it does not know, with the item loader's messages (key, line, the allowed values).
3. The canonical text of a definition with an empty table is byte-identical to today's (nothing in an existing world's text or hash moves).
4. The owner accepts the new schema version (it is a persisted format).

## Tests (tier A, ids from the planner)
- Round trip of the table: parse, canonical text, parse equals; hash independent of key order, spacing, `-0.0`; a changed field changes the hash of exactly the stages that read it.
- Refusals: an unknown kind or key, both `factor` and `offset`, a bound exceeded, a reach over a hemisphere, a duplicate id.
- Migration: the previous-version fixture loads; its canonical text under the new version has an empty table and the same hash of everything else.
- The weight and term at sample directions (inside, at the rim, outside, overlapping regions in table order); the combination is order-sensitive only through the table order.
- The preview: `PreviewEdit::climate` from a table equals the hand-made `ClimateRegion` list.
- The incremental result of an H0 climate with a region equals the fresh one (CON-29).

## Order
1. The architect decides ADR 0117; the owner accepts the schema version. 2. `world-def`: the table, the loader, the migration, the hash (no stage reads it yet). 3. The stages read it (H0 climate first, its key moves only for worlds with regions). 4. The editor commands and level 0. 5. Level 1 for lithology and uplift with item 15 and GEN-153.
