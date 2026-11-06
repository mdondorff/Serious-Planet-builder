## MODIFIED Requirements
### Requirement: GEN-118 Shelf profile
The default class source SHALL give the ocean floor a continental shelf: from the coast of the field the floor falls linearly to the shelf break over a width that blends smoothly between a passive and an active value by the margin activity at that coast, then down the continental slope to the abyssal depth. The distance to the coast and the margin activity SHALL be read from the coast lattice (GEN-136); the floor SHALL fade in from the coast and keep a minimum of water. Land SHALL be unchanged. Every depth, width and ramp SHALL be a validated field of `ShelfProfile` with the Earth preset below as default (ADR 0108); both versions change.

Verify: A
Status: active
Source: owner 2026-11-05 (planner inbox #83), SPIKE-102, ADR 0107, ADR 0108, planner inbox #84

The Earth preset of `ShelfProfile`:

| Parameter | Earth default | Kind |
|---|---|---|
| break depth, abyssal depth | 130 m, 4,500 m | Earth state |
| active width, slope width | 30 km, 90 km | Earth state |
| passive width | 80 km | calibration (Earth's mean shelf width, about 80 km; tuned so that the share of the ocean under 200 m is of Earth's order on a coast longer than Earth's) |
| active within (distance), active convergence | 150 to 450 km, 0.05 to 0.35 | calibration |
| coast lattice cells, coast ramp (land depth) | 512, 0.01 | calibration |
| minimum water, its ramp | 5 m, 0.01 | calibration |

#### Scenario: Profile
- **WHEN** the profile is read at 0 km, at the shelf width, at the foot of the slope and beyond, and along a walk of quarter-km steps
- **THEN** the floor is 0 at the coast, minus the break depth at the shelf width, minus the abyssal depth at the foot of the slope and beyond, never rises seaward, and has no step over 20 m in a quarter km

#### Scenario: Margin type
- **WHEN** the active weight is 0, 1 and between, far from any margin, and at a non-converging margin
- **THEN** the width is the passive value at 0 and the active value at 1 with a smooth blend and no switch, and a coast far from a margin or at a non-converging one is passive

#### Scenario: The field
- **WHEN** the field is sampled over the sphere
- **THEN** land is bit-identical to the field before the shelf (pinned points), the sea is between the abyssal depth and 0, the share of the ocean shallower than 200 m is of Earth's order, the abyss is reached, and at every coast of the field the floor meets the land within 5 m

#### Scenario: Parameters
- **WHEN** a parameter set has a non-positive or non-finite depth or width, a break not above the abyssal plain, a ramp with equal or reversed edges, or cells that are not a power of two
- **THEN** construction refuses it

#### Scenario: The preview
- **WHEN** `generate --shelf-preview DIR [--seed N | --world F] [--km N]` is run, and combined with another generate command or with `--world` and `--seed`
- **THEN** it writes the bathymetry map, a passive and an active coast region at 2 km and 10 km and `shelf.txt` (the shelf share of the ocean, and by margin type the distance from the coast of the field to the shelf break p10, p50 and p90 and to the foot of the slope p50, against Earth's), and refuses the combinations, naming them

## ADDED Requirements
### Requirement: GEN-136 Coast distance lattice
The class source SHALL read the distance to the coast of its own field from a lattice over the cube faces: per point the signed distance to the nearest sea-level crossing of the continent value (sea positive, land negative, at most a cap of the widest shelf plus the slope) and the margin activity at that crossing. The crossings SHALL be found on the lattice's cell edges, the nearest by a bounded search; the read SHALL be bilinear and continuous across face edges. The lattice SHALL be built on the first read and its bytes SHALL not depend on the thread count. The distance SHALL not read the field's gradient.

Verify: A
Status: active
Source: planner inbox #84, #89, GEN-118, ADR 0107, ADR 0108

#### Scenario: Exactness
- **WHEN** the field is a circular island and the lattice is read at points within the cap
- **THEN** the distance equals the arc to the island's edge within a quarter of a cell

#### Scenario: Sign, cap and no coast
- **WHEN** a point lies on land, in the sea, beyond the cap, or the world has no coast
- **THEN** the distance is negative, positive, the cap, or the cap with activity 0

#### Scenario: Continuity and threads
- **WHEN** the lattice is read at two directions 6 m apart across the sphere, and built on 1 and 5 threads
- **THEN** the read changes by under 0.01 and the bits are the same

#### Scenario: Parameters
- **WHEN** the cell count is not a power of two
- **THEN** construction refuses it
