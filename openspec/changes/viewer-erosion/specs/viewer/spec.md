## ADDED Requirements
### Requirement: VIEW-062 The erosion options of the viewer
The viewer SHALL take `--erosion on|off` (off by default: nothing changes, no key, id or golden) and, only with `on`, `--erosion-store DIR` (a bake's output folder or its `tiles/`; the world's store when not given), `--erosion-face-res N` and `--erosion-res N` (the bake's pass-1 and pass-2 cells, 12,288 and 98,304 by default) and `--erosion-unchecked`. The options SHALL be refused outside the viewer, and without `on`; there is no definition key.

Verify: A
Status: planned
Source: Planner 2026-11-04 (inbox #56 item 1b), owner 2026-11-04 (integrated preview), GEN-112

#### Scenario: Options
- **WHEN** the options are parsed, and given without `--erosion on` or outside the viewer
- **THEN** the defaults are off and the production bake, bad values are refused, and the options without `on` are refused

### Requirement: VIEW-063 The viewer refuses a layer that does not fit its world
`--erosion on` SHALL be refused, naming the problem, without `--store`, with a field that is not an eroded one, with a relief scale other than 1 or a sea other than at 0 m (what the layer was carved on), over a folder with no layer tile, with a layer made under other keys (the refusal names both, STRM-065), and over a store that does not say it was baked from the class source unless `--erosion-unchecked` is given.

Verify: A
Status: planned
Source: Planner 2026-11-04 (the layer must land on the surface it was carved on), STRM-065

#### Scenario: Refusals
- **WHEN** a stored world and a tiny layer made under the shape's keys are opened with `--erosion on` without `--erosion-unchecked`, without a store, with a pass-1 size the layer was not made for, over a missing folder, and over definitions with relief scale 1.5 and a sea at 100 m
- **THEN** each is refused with its reason, and the one with `--erosion-unchecked` and the right keys opens

### Requirement: VIEW-064 The viewer says the layer is on
With the layer on, the terrain's implementation id SHALL be the layered field's (`cpu-erosion-v2+band+layer`: the default field is upgraded to the C1 base the layer is read over), `describe()` and the log of the run SHALL name the layer's folder, the bake shape, the seed, the coast fade and the sea level, and the title SHALL carry "erosion layer". A repro bundle records that id, so a replay without the layer meets another id.

Verify: A
Status: planned
Source: Planner 2026-11-04 (log what is on), CON-05 to CON-08

#### Scenario: What is on
- **WHEN** the layer is opened over a stored world, and over a world whose field is not an eroded one
- **THEN** the id is the C1 layered field's and differs from the plain world's, `describe()` names the layer, the field moves land heights, and the other world is refused (the log line and the title are set from the same state and are not asserted in tier A)
