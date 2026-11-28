## ADDED Requirements

### Requirement: TEST-165 A region of a regions file bakes and opens as a commit
A tier-A test SHALL write a regions file with one 2x2 land box, read it back, bake the box with `bake_entry` over the production base of a single-planet definition on a small layout of the production shape, and open the commit with `--commit`'s in-memory path (VIEW-069).
It SHALL check the derived states before and after the bake, and after a moved box, another layout and an item put into the world from an `--items` folder.
Tier-A tests SHALL also check the format's refusals, the shared box rule against `RegionSpec::check` in the production layout, and `bake-region`'s options and plan.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E1 gate), WDEF-019, BUILD-078

#### Scenario: End to end
- **WHEN** the box is baked and opened
- **THEN** its state goes `unbaked` to `ready`, the commit opens in memory with the bake's `base_id`, and the moved box, the other layout and the edited world each read `stale`

#### Scenario: The rule and the refusals
- **WHEN** every size of box is checked at the corners and the middle of each face, and one tile further out
- **THEN** each accepted box passes `RegionSpec::check` and each box outside the band is refused naming the solve margin
