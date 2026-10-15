## MODIFIED Requirements

### Requirement: REND-013 Persistent terrain renderer
The terrain renderer SHALL create its pipeline, uniform buffers and render targets once, keep tiles in pooled fixed-size slots of a few large vertex buffers (blocks allocated on first use, a small pool for levels 0-2, no buffer per tile), upload each tile's vertices once per mesh key into a slot (a mesh with a different key takes a new slot and the old one is released), draw a frame by writing only the per-frame uniforms and recording the draws, report CPU and wall time per frame and GPU time from timestamp queries when the device supports them, and produce exactly the image of the one-shot executor. Where a tile sits (the renderer's own free list or a placement chosen by the residency manager) SHALL NOT change the image. The bytes it allocates SHALL equal the pure model of the pool configuration (STRM-006) and a full pool SHALL refuse an upload with the tile id.

Verify: C
Status: active
Source: CON-20, CON-21, ADR 0013, ADR 0033 point 3

#### Scenario: Reuse
- **WHEN** the same tiles are drawn in several frames and views
- **THEN** no vertex data is uploaded after the first upload and every frame equals the one-shot result

#### Scenario: Missing tile
- **WHEN** a plan names a tile that was not uploaded
- **THEN** rendering fails with the tile id

#### Scenario: Changed mesh
- **WHEN** a mesh with the same tile id but a different key (other seed) is uploaded
- **THEN** the resident buffer is replaced and the next frame equals the one-shot render of the new mesh

#### Scenario: Stale mesh
- **WHEN** a plan expects a mesh key that differs from the resident buffer's key (the seed changed and the mesh was not re-uploaded)
- **THEN** rendering fails with the tile id and both keys instead of drawing the stale buffer

#### Scenario: Placement
- **WHEN** the same tiles are placed by the renderer's free list, scattered over blocks by the caller, or by the residency manager's slots across evictions
- **THEN** every frame equals the one-shot result

#### Scenario: Full pool
- **WHEN** every slot is in use or still in flight and another tile is uploaded
- **THEN** the upload fails with the tile id, and succeeds into the freed slot once a frame has retired it
