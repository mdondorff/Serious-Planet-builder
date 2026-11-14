## MODIFIED Requirements

### Requirement: VIEW-070 The viewer keeps at most a capped number of tiles resident
The viewer's streamer SHALL count its capacity as the smaller of the renderer's pool slots (STRM-006) and the resident-tile cap, 24,000 tiles by default (`--resident-cap N` overrides it). At the cap the streamer evicts by STRM-007; the draw set with its ancestors and the roots are never evicted. The cap is above R of the standard views (20,517 slots, 10 % of the mid tier), so with every tile of R resident eviction changes no pixel. The perf scenes keep the pool as their capacity. A window prints on closing `planet editor: resident tiles at most N, evicted M (cap C)` and the device allocator's memory at the start and the end (allocated, reserved, the largest allocations).

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E2: the owner's flight stalled at the cap, and the video memory grew from 1.5 to 5.2 GB in a replay at a steady resident count), STRM-007
