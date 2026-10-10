# terrain-lod Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: LOD-001 CPU node selection
Terrain node selection (which quadtree nodes at which level, with morph factors) SHALL be a pure function of camera, parameters and the tree, implemented in GPU-free code, and its result SHALL be plain data (`NodeSelection`) that the plan builder consumes; terrain draws in the FramePlan arrive with the terrain render change.

Verify: A
Status: active
Source: CON-03, ADR 0009

#### Scenario: Determinism
- **WHEN** selection runs twice with the same camera and parameters
- **THEN** the node lists and morph factors are identical

### Requirement: LOD-002 Selection snapshots
Node selection for a fixed set of scripted views SHALL be snapshot-tested (node count, per-level counts and a hash over node IDs and morph bits per view), and a snapshot change SHALL be reported per view with the old and new values.

Verify: A
Status: active
Source: CON-03, report §16

#### Scenario: Behaviour change
- **WHEN** the selection function changes its output for a scripted view
- **THEN** the snapshot test fails and prints, per changed view, the node count, per-level counts and hash now and in the snapshot

### Requirement: LOD-003 Complete coverage
The selected nodes SHALL cover every surface point visible over the horizon exactly once, without gaps or overlaps (nodes entirely below the horizon, padded by the maximum terrain height, MAY be dropped), and neighbouring selected nodes SHALL differ by at most one level.

Verify: A
Status: active
Source: report §2

#### Scenario: Partition
- **WHEN** a deterministic sample of surface directions in view is tested against the selection
- **THEN** each direction lies in exactly one selected node

### Requirement: LOD-004 Morph continuity
The morph factor of a node SHALL be a continuous, monotone function of the distance metric used for selection (measured to the parent's bounding sphere), with value 0 until the last `morph_band` of the parent's split distance and 1 at that distance, so that a merge never pops.

Verify: A
Status: active
Source: report §2 (CDLOD)

#### Scenario: Distance sweep
- **WHEN** the camera moves away from a node in small steps
- **THEN** its morph factor never decreases and changes by at most the step bound

### Requirement: LOD-005 No cracks in debug views
In the tile-ID debug view of each scripted view, every pixel whose view ray hits the sphere just below the lowest terrain (reduced by the sagitta of the coarsest drawn mesh) SHALL show terrain, never the background.

Verify: C
Status: active
Source: report §16

#### Scenario: Ten scripted views
- **WHEN** the ten scripted views are rendered in the tile-ID debug view
- **THEN** the hole-pixel count of each is zero

### Requirement: LOD-006 Plan builder
The GPU-free plan builder SHALL lay tiles out in pixel rectangles, compute each camera-relative origin as the float64 difference rounded once to float32, place tile origins on the reference sphere, give distinct stable colours to distinct tile IDs, and reject unusable requests (no tiles, unknown face mapping, frame too small) with a message.

Verify: A
Status: active
Source: CON-03, CON-11, ADR 0001

#### Scenario: Two tiles
- **WHEN** two tiles are planned into a 256 by 128 frame
- **THEN** their rectangles are [0,0,128,128] and [128,0,128,128] and each camera-relative origin equals the f64 difference rounded once

### Requirement: LOD-007 Frustum selection
Selection for a camera SHALL additionally drop nodes entirely outside the side planes of its view frustum (padded by the terrain height) and SHALL still hold every in-view point of the reference sphere in exactly one leaf; the camera SHALL use a reversed-Z, infinite-far projection whose depth is 1 at the near plane and decreases with distance.

Verify: A
Status: active
Source: CON-03, CON-12, ADR 0002

#### Scenario: Smaller than the hemisphere
- **WHEN** a scripted ground or aerial view is selected with and without the frustum
- **THEN** the frustum selection has fewer nodes and still covers every ray through the image that reaches the sphere
