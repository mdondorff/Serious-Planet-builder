## ADDED Requirements

### Requirement: VIEW-069 The viewer and capture-stills open a commit over the in-memory production base
With `--commit DIR` the viewer and `capture-stills` SHALL open the commit in DIR over the production generator's base computed in memory for the definition (`--world` or `--seed`): the exact base values on the commit's lattice over its tiles and one tile of margin, the in-memory bake elsewhere. The commit SHALL pass the normal key check (a commit of another world or seed is refused). `--store`, `--erosion`, `--erosion-store`, `--global-res`, `--allow-memory-bake` and the source flags SHALL be refused beside it. The log, the title and every still SHALL say "base: computed in memory, not read from a stored bake".

Verify: A
Status: active
Source: Lead 2026-11-13 (E1, option b; E4 is the owner flying it), TEST-128, REND-058

#### Scenario: Refusals
- **WHEN** `--commit` is given with a stored world, the erosion layer, a bake size, or to another scene
- **THEN** the run is refused naming the clashing options

#### Scenario: Another seed
- **WHEN** a commit baked for seed 3 is opened for seed 4
- **THEN** it is refused before any base is computed
