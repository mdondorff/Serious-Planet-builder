## ADDED Requirements

### Requirement: STRM-064 The horizon maps are baked without a chunk stage
With no chunk stage the horizon jobs SHALL bake the maps of the chunks the plan's tiles of the pyramid's top level and finer lie in, from the base terrain and the stamps, in the same format, march and slots as with a stage. Every chunk SHALL count as ready with no change. The jobs SHALL keep the maps of the wanted chunks and, of the others, the most recently wanted up to the configured number beyond the wanted ones, at most 24 chunks being wanted. An edit that reaches a chunk or the halo of its march, a neighbour included, SHALL change its edit word and bake its map again once the word has been unchanged for the settle window. A change of the world word (the coupled lens, the base of the definition, the stage) SHALL do the same for every wanted map, and a height tile of the bake SHALL be kept only if no edit was published since the field it was evaluated from was built.

Verify: A, C
Status: active
Source: owner fallback 2026-10-28, opus-planner M8-F step 2, STRM-048, STRM-060, finding 16 of the owner test 2026-10-27

#### Scenario: Base-only maps
- **WHEN** the horizon jobs run over a source with no chunk stage and two chunks are wanted
- **THEN** both start at once, land, and are baked again after an edit word change and the settle window; maps of chunks no longer wanted go beyond the cap, the oldest first

#### Scenario: Window without a stage
- **WHEN** the window runs lit over a store without `--chunks`
- **THEN** the horizon jobs bake at least one map and `chunk jobs` stays `None`
