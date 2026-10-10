## MODIFIED Requirements

### Requirement: LOD-005 No cracks in debug views
In the tile-ID debug view of each scripted view, every pixel whose view ray hits the sphere just below the lowest terrain (reduced by the sagitta of the coarsest drawn mesh) SHALL show terrain, never the background.

Verify: C
Status: active
Source: report §16

#### Scenario: Ten scripted views
- **WHEN** the ten scripted views are rendered in the tile-ID debug view
- **THEN** the hole-pixel count of each is zero


## ADDED Requirements

### Requirement: LOD-007 Frustum selection
Selection for a camera SHALL additionally drop nodes entirely outside the side planes of its view frustum (padded by the terrain height) and SHALL still hold every in-view point of the reference sphere in exactly one leaf; the camera SHALL use a reversed-Z, infinite-far projection whose depth is 1 at the near plane and decreases with distance.

Verify: A
Status: active
Source: CON-03, CON-12, ADR 0002

#### Scenario: Smaller than the hemisphere
- **WHEN** a scripted ground or aerial view is selected with and without the frustum
- **THEN** the frustum selection has fewer nodes and still covers every ray through the image that reaches the sphere
