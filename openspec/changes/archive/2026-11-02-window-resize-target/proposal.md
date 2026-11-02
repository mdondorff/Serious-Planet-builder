## Why
Tracked owner bug (2026-10-30, P2): the 3D target keeps its size after a window resize (`--size WxH` at the start worked around it). VIEW-006 said the renderer is not rebuilt on a resize; the blit scaled the old target with black bars.

## What Changes
- `TerrainRenderer::resize`: colour and depth, the lit path's targets, the atmosphere's products and the water copy are made again; tiles, pipelines and pools are kept.
- The window applies a resize once the size has held for 150 ms, sets the camera's aspect from the target size and snaps the exposure.
- VIEW-006 modified (the "without rebuilding the renderer" clause replaced by the follow rule); scenarios Resize and Settle added.
- Tests: tier C `gpu_resize` (a resized renderer draws a fresh one's frame, lit and with the atmosphere, there and back; a plan of the old size is refused), tier A `resize_due` and `aspect_of`.

## Capabilities
### Modified Capabilities
- `viewer`: VIEW-006
