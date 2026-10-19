## Why
M6 plan: the editor edits a `WorldDefinition` (E4), but the window's `WorldTerrain` owned its definition immutably and its fields built no stamp stage, so an edit never reached a plan or a tile. The stamp stage (E2) exists; the app does not use it.

## What Changes
- **Live definition** (STRM-028): `WorldTerrain` holds the definition as a replaceable `Arc` (`set_definition`, `definition()` returns a snapshot). Every reader takes the snapshot once per job (`field()`), so a job in flight keeps the definition it started with and its keys say which.
- **Stamp stages in the app field** (STRM-028): `field()` stacks one `StampStage` per layer that holds a stamp (`WorldDefinition::stamp_layers`, the layer stack of ADR 0007 first, other names after, by name) over the base, below the water bound. A definition without stamps builds the field it built before.
- **Items only**: `set_definition` refuses a definition whose `base_hash` differs from the current one (the bake was made from the base; a node change needs another terrain).
- **Order for the caller**: swap first, then `ManagedLoop::edit(tiles)`.
- `PlanField::item_revision` (default 0) lets the plan worker's residency set follow the items; the world planner returns the definition's revision.

## Impact
- Specs: STRM-028 (added).
- Code: `app` (worldfield, pipelined), `generators` (`StampStage<'a>` over a base that borrows the bake, `STAMP_STAGE_VERSION`), `world-def` (`stamp_layers`, `LAYER_STACK`), `editor` (re-exports the stack constant).
- Goldens: none (no world has stamps).
- Not here: the caller side (U3, sonnet-crusader), the edit journal (key cost after an edit), the tier C stamped scene (stamp-stage 3b).
