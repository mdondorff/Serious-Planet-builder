## ADDED Requirements

### Requirement: STRM-064 The horizon maps are baked without a chunk stage
With no chunk stage the horizon jobs SHALL bake the maps of the chunks of the plan's tiles (chunk-grid level 11 and finer) from the base terrain and the stamps, in the format, march and slots of a run with a stage, and SHALL bake a map again when an edit or a change of the world word reaches it.

Verify: A, C
Status: active
Source: owner fallback 2026-10-28, opus-planner M8-F step 2, STRM-048, STRM-060, finding 16 of the owner test 2026-10-27

Detail: Every chunk counts as ready with no change. The jobs keep the maps of the wanted chunks (at most 24) and, of the others, the most recently wanted up to the configured number beyond them. An edit that reaches a chunk or the halo of its march, a neighbour included, changes its edit word; a change of the world word (the coupled lens, the base of the definition, the stage) changes every word; the map is baked again once its word has been unchanged for the settle window. A height tile of the bake is kept only if no edit was published since the field it was evaluated from was built.

#### Scenario: Base-only maps
- **WHEN** the horizon jobs run over a source with no chunk stage and two chunks are wanted
- **THEN** both start at once, land, and are baked again after an edit word change and the settle window; maps of chunks no longer wanted go beyond the cap, the oldest first

#### Scenario: Window without a stage
- **WHEN** the window runs lit over a store without `--chunks`
- **THEN** the horizon jobs bake at least one map and `chunk jobs` stays `None`
