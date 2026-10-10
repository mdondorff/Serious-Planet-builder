## ADDED Requirements

### Requirement: REND-013 Persistent terrain renderer
The terrain renderer SHALL create its pipeline, uniform buffers and render targets once, upload each tile's vertices once, draw a frame by writing only the per-frame uniforms and recording the draws, report CPU and wall time per frame and GPU time from timestamp queries when the device supports them, and produce exactly the image of the one-shot executor.

Verify: C
Status: active
Source: CON-20, CON-21, ADR 0013

#### Scenario: Reuse
- **WHEN** the same tiles are drawn in several frames and views
- **THEN** no vertex data is uploaded after the first upload and every frame equals the one-shot result

#### Scenario: Missing tile
- **WHEN** a plan names a tile that was not uploaded
- **THEN** rendering fails with the tile id
