## MODIFIED Requirements

### Requirement: VIEW-069 The viewer and capture-stills open a commit over the in-memory production base
With `--commit DIR` the viewer and `capture-stills` SHALL open the commit over the production base computed in memory for the definition: exact base values over its tiles and one tile of margin, the in-memory bake elsewhere. The commit SHALL pass the normal key check. `--store`, `--erosion`, `--erosion-store`, `--global-res`, `--allow-memory-bake` and the source flags SHALL be refused beside it. The log, the title and every still SHALL say "base: computed in memory, not read from a stored bake". The viewer SHALL start held still 25 km over the ground, 20 km south of `--site` or the centre of the commit's tiles, looking north 50 degrees down; the title SHALL say "inside the region" or "outside the region: coarse base".

Verify: A
Status: active
Source: Lead 2026-11-13 (E1, option b; E4 is the owner flying it), TEST-128, REND-058

#### Scenario: Refusals
- **WHEN** `--commit` is given with a stored world, the erosion layer, a bake size, or to another scene
- **THEN** the run is refused naming the clashing options

#### Scenario: Another seed
- **WHEN** a commit baked for seed 3 is opened for seed 4
- **THEN** it is refused before any base is computed

#### Scenario: Start over the region
- **WHEN** the viewer opens a commit, without `--site` and with it
- **THEN** the camera starts over the centre of the commit's tiles or the site, the title says "inside the region", and over the default site it would say "outside the region: coarse base"
