## ADDED Requirements
### Requirement: GEN-172 A window cut gives the breach no floor
When the caller marks the land outlets on the border of the grid (of a masked grid: of its rectangle) as window cuts (`CarveParams::window_cut`), the breach SHALL NOT take their height as a base: a receiver chain that ends in such an outlet SHALL have no base (the breach works as without GEN-162), while a chain that ends in an interior land outlet (a basin's rim) keeps the base of GEN-162. The kind of an outlet SHALL be set by the caller (the flag), not inferred from heights. A region bounded by the sea SHALL have no window-cut outlet, and a build without the flag SHALL be bit-identical to the build before this requirement; the catchment key SHALL hold the outlet kind of the catchment (only when it is a window cut), and the key of a layer built with the flag SHALL differ from one built without it.

Verify: A
Status: planned
Source: Architect, 2026-11-12 (SPIKE-107)

#### Scenario: Valley behind a window cut
- **WHEN** a bowl whose border cells are land outlets runs a round with the border as a window cut
- **THEN** the breach cuts the cells beside the border far below the border cells' height, and with the border as a basin rim it does not
