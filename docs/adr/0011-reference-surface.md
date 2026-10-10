# 0011 Reference surface
Status: proposed
Date: 2026-10-10
Source: background report §17 item 8; constitution CON-15

## Context
Geo-coordinates (lat, lon, height) are the serialization format for authored anchors. Whether height is above a sphere or an ellipsoid must not leak into stored data.

## Options (with trade-offs)
1. Sphere only, hard-coded. Simplest, but adding an ellipsoid later changes the meaning of stored heights.
2. **Sphere first behind a `ReferenceSurface` interface** (geo ↔ PlanetFixed, surface normal, height conversion), so an ellipsoid is an additional implementation.
3. Ellipsoid from the start. Extra complexity with no MVP benefit.

## Decision
Option 2. `core` defines the `ReferenceSurface` trait with a `Sphere { radius }` implementation (default Earth mean radius 6,371,000 m, configurable in the WorldDefinition for other planets). Stored anchors are `(lat, lon, height above the surface, heading)` in degrees/metres; the surface description (kind and parameters) is part of the WorldDefinition and its hash. A contract test suite runs against every implementation.

## Consequences
- Round trip geo → PlanetFixed → geo ≤ 1 mm everywhere including poles and the antimeridian (CON-13).
- Adding `Ellipsoid` later is a new implementation plus a schema version bump (CON-10), not a rewrite.

## Licences of adopted code or techniques
None.
