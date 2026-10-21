# 0076 Driver maps: physical fields decide materials, plants and shapes through suitability rules
Status: proposed (opus-planner, 2026-10-21; for the owner's review before M8; D1 measures the open costs)
Date: 2026-10-21
Source: owner, 2026-10-21 (generation principles; materials and plants from physical maps such as soil depth, not from painted type maps); `docs/briefings/m8-driver-maps.md`; ADR 0022, 0026, 0030, 0035, 0039, 0066; constitution CON-06, CON-07, CON-08, CON-14

## Context
- The generator has four global channels (continentalness, elevation, temperature, precipitation; ADR 0026) and a local erosion filter (ADR 0030 point 1); the global stream-power stage (ADR 0030 point 2) is not built.
- Ground colour is chosen in the shader from temperature, precipitation, slope and height (ADR 0039); regions carry an authored surface type (WDEF-012).
- The owner wants materials, plants and later shapes to follow from physical maps (soil depth, moisture, insolation) through rules, instead of maps that name a type.
- Anything a CPU system reads (placement, collision, picking) is a field: one canonical CPU implementation, bit-exact (CON-07, CON-14).
- Instancing (I1, then a build) needs a placement density per plant or rock kind.

## Decision
1. **Three layers.** Drivers (physical fields) → suitability (rules) → consumers (ground materials, instances, later geometry). No consumer reads a map that names a type; the region's surface type becomes one driver among others.
2. **Drivers are fields** under CON-07, CON-08 and CON-14, each with its own version and implementation id, in the stage that can compute it: global-coarse for anything that needs the whole planet (flow accumulation, water table, soil depth), regional for neighbourhood terms (slope, curvature, aspect, wetness index, insolation), local only for cheap per-point derivations.
3. **A driver vector per sample** is the interface between drivers and rules: a fixed, versioned list of at most 12 named channels with units and ranges. A new driver adds a channel and bumps the vector's version; rules and consumers address channels by name.
4. **Suitability is a pure function** of the driver vector and a rule table: per kind, one response curve (trapezoid: zero, rise, plateau, fall) per driver it cares about, combined by product; kinds of one class (ground cover, canopy, rock) are normalised against each other in a fixed order. The rule table is data in the world definition (it decides placement, so it is keyed); the look file holds only appearance.
5. **Two evaluations of one rule table:** the CPU evaluates it exactly for placement (a field, CON-14); the shader evaluates it for ground materials (presentation, ADR 0035) from drivers stored per tile, within a tested bound of the CPU result.
6. **Processes are pre-bake stages**, one field each: first the stream-power routing of ADR 0030 point 2 (flow accumulation, incision), then soil depth and moisture from it. A new process adds a stage and re-keys only the fields that read it.
7. **Authoring acts on drivers or on suitability**, softly and feathered (owner Q8): a region adds to a driver (wetter, deeper soil) or scales a kind's suitability; it never paints a type.
8. **Every driver and every kind's suitability has a debug view** (CON-18); the overview maps of M9 are these views in a rectangular projection.

## Consequences
- Materials (G2a, G2b) already read an inputs-to-weights function, so they move to the driver vector without a shader interface change.
- Bake time and storage grow per driver; D1 measures soil depth and flow accumulation at the store's resolution before M8 commits to a list.
- Placement becomes reproducible and editable by rules: changing a curve re-places plants everywhere it matters, with a diff of the suitability views.
- Open: the first driver list and resolutions (D1), the number of kinds per class, whether suitability for materials is stored per tile or evaluated per pixel (D1 measures both).

## Licences of adopted code or techniques
None yet. Response-curve niche models and stream-power routing are published methods; any adopted implementation is recorded here.
