## Where the time went
Oracle test, per view at the largest (ground-1m, 10,612 candidate tiles): projection 7.5 s on 20 cores (about 150 s of CPU) because `tile_mesh_field` samples the field 5 x 1,089 + 1 times per tile; depth pass 3.2 s and ownership pass 3.1 s, both on one thread. Selection and plan builds cost under 0.1 s per view. Over ten views: 27 s locally; on a 4-core runner the projection alone is about five times longer.

`gpu_window`: 1.5 s to the first frame (device, rig, lit warm-up), then 48 frames of about 0.58 s each of WARP shading (terrain draw 0.1-0.3 s, lit and atmosphere passes the rest); the CPU side is under 50 ms per frame. The time is the software adapter's own, per frame; frames, resolution and passes are what the smoke judges, so they stay. The two runs do not share work (different looks, separate processes by the one-WARP-device-per-process rule) but they are independent and count-judged, so they overlap.

## Decisions
- The positions function lives in the test support, not in `generators`, so no production code changes; the bit-for-bit check keeps it from drifting from the mesh.
- Depth buffer merge by maximum: the depth test is `d > z[i]` per pixel, so the result is the per-pixel maximum of all depths whatever the order; the ownership pass reads the final buffer, so it is order-free too.
- No nextest timeout override: after the changes both tests sit far below 180 s (see the log line).
