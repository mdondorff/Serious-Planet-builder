## ADDED Requirements
### Requirement: GEN-118 Shelf profile
The default class source SHALL give the ocean floor a continental shelf: from the coast the floor falls linearly to the shelf break over a width that blends smoothly between a passive and an active value by margin activity, then down the continental slope to the abyssal depth. The distance to the coast SHALL be the smooth land depth over its gradient (the continent stage without its detail); the profile SHALL fade in from the field's coast, keep a minimum of water and blend into the abyss. Land SHALL be unchanged. Every depth, width and ramp SHALL be a validated field of `ShelfProfile` with the Earth preset below as default (ADR 0108); both versions change.
Verify: A
Status: active
Source: owner 2026-11-05 (planner inbox #83), SPIKE-102, ADR 0107, ADR 0108

The Earth preset of `ShelfProfile`:

| Parameter | Earth default | Kind |
|---|---|---|
| break depth, abyssal depth | 130 m, 4,500 m | Earth state |
| passive width, active width, slope width | 150 km, 30 km, 90 km | Earth state |
| active within (distance), active convergence | 150 to 450 km, 0.05 to 0.35 | calibration |
| abyss blend, coast ramp (land depth) | 0.5 to 0.9, 0.06 | calibration |
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
- **WHEN** a parameter set has a non-positive or non-finite depth or width, a break not above the abyssal plain, or a ramp with equal or reversed edges
- **THEN** construction refuses it

#### Scenario: The preview
- **WHEN** `generate --shelf-preview DIR [--seed N | --world F] [--km N]` is run, and combined with another generate command or with `--world` and `--seed`
- **THEN** it writes the bathymetry map, a passive and an active coast region at 2 km and 10 km and `shelf.txt` (the shelf share of the ocean, the shelf width p10, p50 and p90 and the slope width p50 by margin type, against Earth's), and refuses the combinations, naming them
