## Why
SPIKE-88 (`spikes/first-read-stalls/results/`): the K view stall at orbit (report `20261030-110527Z`, 1,063 ms in the camera phase) is the free camera's ground read decoding 4 bake blocks on the main thread while K's refresh of every resident tile keeps the cores busy. A main-thread decode costs about 20 ms idle and 70-320 ms under that load, and the 512 MB shared cache (about 30 blocks) lets the loaders push the camera's blocks out.

## What Changes
- **App:** `cameraground::CameraGround`, a thread of its own for the free camera's ground (the pattern of the hover pick, VIEW-059): the frame polls it with the last frame's nadir and uses the last finished answer; until the first answer of a run the field is read on the main thread. `WorldTerrain::ground_pinned` reads the height and pins the blocks it touched.
- **Cache:** `StoreSource::pin_touched` keeps at most 8 blocks (fewer than the budget) out of LRU eviction; `record_touched_blocks` lists the blocks a read touched on the calling thread. The budget still holds.
- **Stall report:** per phase, the time the main thread spent getting blocks next to the decode count.
- **Spec:** VIEW-061 gains the camera's ground read, the pins and the decode time.
- Not in this change: the plan phase's origin heights (one decode under load in the same report).

## Capabilities
### Modified Capabilities
- `viewer`: VIEW-061
