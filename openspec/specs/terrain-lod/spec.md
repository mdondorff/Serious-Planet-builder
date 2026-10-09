# terrain-lod Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: LOD-001 CPU node selection
Terrain node selection (which quadtree nodes at which level, with morph factors) SHALL be a pure function of camera, parameters and the tree, implemented in GPU-free code, and its result SHALL be part of the FramePlan.

Verify: A
Status: planned
Source: CON-03, ADR 0009

#### Scenario: Determinism
- **WHEN** selection runs twice with the same camera and parameters
- **THEN** the node lists and morph factors are identical

### Requirement: LOD-002 Selection snapshots
Node selection for a fixed set of scripted views SHALL be snapshot-tested, and a snapshot change SHALL be visible as a readable diff of node IDs, levels and morph factors.

Verify: A
Status: planned
Source: CON-03, report §16

#### Scenario: Behaviour change
- **WHEN** the selection function changes its output for a scripted view
- **THEN** the snapshot test fails and prints the added and removed nodes

### Requirement: LOD-003 Complete coverage
The selected nodes SHALL cover the visible planet surface exactly once, without gaps or overlaps, and adjacent nodes SHALL differ by at most one level or be joined by morphing so that no crack is possible.

Verify: A
Status: planned
Source: report §2

#### Scenario: Partition
- **WHEN** a deterministic sample of surface directions in view is tested against the selection
- **THEN** each direction lies in exactly one selected node

### Requirement: LOD-004 Morph continuity
The morph factor of a node SHALL be a continuous, monotone function of the 3D distance metric used for selection, with value 0 inside the node's own range and 1 at the parent's range.

Verify: A
Status: planned
Source: report §2 (CDLOD)

#### Scenario: Distance sweep
- **WHEN** the camera moves away from a node in small steps
- **THEN** its morph factor never decreases and changes by at most the step bound

### Requirement: LOD-005 No cracks in debug views
Scripted views SHALL show no background-coloured pixel inside the planet disc in the tile-ID debug view.

Verify: C
Status: planned
Source: report §16

#### Scenario: Ten scripted views
- **WHEN** the ten scripted views are rendered in the tile-ID debug view
- **THEN** the hole-pixel count of each is zero
