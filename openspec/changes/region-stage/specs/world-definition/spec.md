## ADDED Requirements

### Requirement: WDEF-012 Regions and surface types
The world definition SHALL hold a surface-type table (ADR 0069), a data table and not a code enum, with the entries `rock`, `sand`, `grass`, `snow`, `forest`, `ice`, `desert`, `wetland` and `urban` in this order, each with its name and a debug tint (an sRGB colour). The item kind `region` SHALL be a closed polygon of 3 to 64 points (`[[points]]`, AUTH-005; the polygon closes from the last point to the first, which is the anchor) with the parameters `surface_type` (a whole number, the index of an entry of the table) and `mask_mode` (0 none, 1 clip outside, 2 clip inside), and the item's `mask_layer` (the name of the layer whose edits the region clips; required for a mode other than 0, refused for mode 0). A region changes no height itself. The item's `reach_m` SHALL cover every point and stay within one nominal cell edge (WDEF-007), so a polygon larger than one reach is refused naming the limit. A put of a region SHALL be refused, naming the item and the reason, when an index is outside the table, the mode is not 0, 1 or 2, `mask_layer` is missing or given against the rules, the layer does not exist, the polygon has fewer than 3 or more than 64 points, two consecutive points coincide, the polygon crosses itself, or a point lies beyond `reach_m` of the anchor; the definition is then unchanged. A point is inside a region when it lies inside or on the polygon by the even-odd rule on the gnomonic projection of the points onto the tangent plane at the anchor (f64, `libm`). The footprint of a tile SHALL return a region with its points when its cap meets the polygon, and, for a region of mode 1, every tile of the clipped layer. The region's content hash SHALL be derived from the kind, the parameters and the points under `planet-builder stamp v1` (WDEF-010).

Verify: A
Status: planned
Source: ADR 0069, M6 plan row E5, owner (2026-10-19), WDEF-010

#### Scenario: The table
- **WHEN** the table is read
- **THEN** it has the nine entries in the order above, each with a tint, and an index of 9 or more is refused

#### Scenario: Refusal
- **WHEN** a region is put with an index outside the table, a mode of 3, a mode 1 without `mask_layer`, an unknown layer, two points, a self-crossing polygon or a point beyond its reach
- **THEN** the call is refused naming the item and the reason and the definition is unchanged

#### Scenario: Inside
- **WHEN** points inside, outside, on an edge and at a vertex of a concave polygon are tested, near the pole and across the antimeridian included
- **THEN** inside and on-edge points are inside, the others outside, and the answer does not depend on the order or starting point of the polygon's points

#### Scenario: Footprint
- **WHEN** the footprint of a tile that meets a region, of one that does not, and of a tile under a mode-1 region's layer is asked
- **THEN** the first returns the region with its points, the second does not, and the third does

#### Scenario: Re-key
- **WHEN** a point, the surface type, the mode or the mask layer of a region changes
- **THEN** the footprint hash of a tile it clips changes and that of an unaffected tile does not
