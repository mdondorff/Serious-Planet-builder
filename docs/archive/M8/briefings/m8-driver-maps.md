# M8 driver maps: a proposal for the owner (opus-planner, 2026-10-21)

Companion to ADR 0076 (proposed). This file explains the idea and lists the choices; the ADR holds the decision.

## The idea in one example
A pine does not need a "forest" map. It needs:

| Driver | Pine's curve (zero / full / full / zero) |
|---|---|
| soil depth | 0.1 / 0.4 / 3 / – m |
| moisture index | 0.2 / 0.35 / 0.7 / 0.85 |
| mean temperature | -12 / -4 / 12 / 18 °C |
| slope | – / 0 / 30 / 45 degrees |
| altitude above the tree line | – / – / -200 / 0 m |

Suitability = product of the five curves (0 to 1). Where it is high, pines grow densely; at the edges they thin out by themselves. Grass, scrub and rock compete in the same way. Nothing is painted, and changing one number (the tree line) moves every forest edge on the planet consistently.

## The layers
```
pre-bake processes        drivers (fields)              rules (data)           consumers
------------------        -----------------             -------------          ----------
stream power routing  ->  flow accumulation, incision
weathering + transport -> soil depth, sediment       ->  suitability per kind -> ground materials (shader)
climate (exists)      ->  temperature, precipitation     (response curves,      instances (CPU placement)
terrain (exists)      ->  slope, curvature, aspect        product, normalised   later: scree, terraces,
sun geometry          ->  insolation                      per class)            river beds (geometry)
regions (authored)    ->  driver offsets, kind scales
```

## Candidate drivers
| Driver | Stage | Derived from | Cost class | Used by |
|---|---|---|---|---|
| elevation, continentalness | global (exists) | generator | exists | everything |
| temperature, precipitation | global (exists) | latitude, elevation, continent | exists | materials, plants |
| slope, curvature | regional | height lattice | cheap | rock vs soil, scree, snow |
| aspect, insolation | regional | normal, latitude, sun | cheap | snow lines, sunny vs shaded slopes |
| flow accumulation | global pre-bake | stream-power routing (ADR 0030 point 2) | expensive, once | rivers, wet valleys, sediment |
| wetness index | regional | flow accumulation and slope (ln(a / tan slope)) | cheap given flow | moisture, wetland, moss |
| soil depth | global pre-bake, regional refine | slope, curvature, flow, rock hardness, climate | medium | plants, rock exposure |
| rock hardness (lithology) | global | noise bands | cheap | cliffs, scree, soil depth |
| sediment | global pre-bake | erosion and deposition | expensive, once | beaches, plains, deltas |
| distance to water | regional | water field W (exists) | medium | shore plants, beaches |

A first M8 set could be: the existing four, plus slope, aspect or insolation, flow accumulation, wetness, soil depth and rock hardness (10 channels).

## What exists already
- The four global channels and the climate per tile (GEN-021) feed today's albedo.
- G2a and G2b compute ground materials through one inputs-to-weights function with data response curves: the seam this proposal plugs into.
- ADR 0030 point 2 already decided a coarse global stream-power stage; it was never built. It is the first process.

## Choices for the owner (to think over; no answer needed now)
1. **First drivers:** the 10 above, or fewer to start (for example without sediment)?
2. **Kinds:** how many ground materials and plant kinds in the first set (for example 6 materials, 8 plants, 3 rock kinds)?
3. **Realism versus control:** strict physical processes (slower bakes, less direct control) or simplified processes with exposed parameters (faster, more art-directable)? Proposal: simplified first, each process replaceable.
4. **Authoring:** regions nudge drivers and scale kinds (proposal), or also hard overrides where needed?
5. **External data (M9):** an Earth elevation dataset would enter as a base field; the drivers above derive from it in the same way.

## What D1 measures (spikes, Sonnet)
- Bake cost and storage of flow accumulation and soil depth at the store's resolution (16384 cells per face edge), and at a coarser grid with regional refinement.
- One rule (the pine above) on the existing world as a suitability debug view.
- Per-tile storage of the driver vector against evaluation per pixel for materials.
