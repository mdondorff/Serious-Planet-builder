## MODIFIED Requirements

### Requirement: REND-001 FramePlan executor
The renderer SHALL execute a FramePlan and SHALL make no decisions about which tiles, rectangles or colours to draw; the number of draw calls SHALL equal the number of draws in the plan, and a plan that cannot be executed (missing resource, rectangle outside the frame) SHALL be reported, not guessed.

Verify: C
Status: active
Source: CON-03, ADR 0009

#### Scenario: Plan in, pixels out
- **WHEN** a FramePlan listing two tiles is executed
- **THEN** exactly those two rectangles are drawn, the draw-call count is 2, and the GPU frame equals the CPU twin of the executor (exactly for flat views, within 1 grey level for the height view)

#### Scenario: Unexecutable plan
- **WHEN** the plan names a tile whose resource is missing
- **THEN** execution fails with the tile id

### Requirement: REND-006 Deterministic debug views
Debug views (tile ID, face, level, morph, normals, depth, streaming state) SHALL render flat, lighting-independent colours that compare exactly against goldens of the same adapter.

Verify: C
Status: active
Source: CON-18, report §16

#### Scenario: Re-render
- **WHEN** a debug view is rendered twice on one adapter
- **THEN** the images are identical


## ADDED Requirements

### Requirement: REND-008 GPU culling differential
GPU-side culling, when added for instances, SHALL be covered by a differential test against a CPU reference on the software adapter.

Verify: B
Status: planned
Source: CON-03, ADR 0009

#### Scenario: Differential
- **WHEN** a GPU culling pass and the CPU reference run on the same instances and frustum
- **THEN** the visible sets are equal
