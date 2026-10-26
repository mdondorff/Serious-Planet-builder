# m8-gen052-active

GEN-052 (A-net chunk time) is met on the production geometry in the perf window of 2026-10-26: 8 m on the production threads 3.33 s and 3.30 s against 4 s; one thread 5.27 s and 5.35 s (recorded); 4 m 14.8 s and 14.6 s; 10 m 2.14 s and 2.11 s (log `perf/logs/anet-chunk.json`). Status planned to active.

Also: `perf anet-chunk --store DIR` is accepted by test-render's option check (it was refused: #733 passed the option, the check allowed it only for the streaming scenes), and the stills `pose_camera` no longer normalises a zero vector for a pose at its anchor (`back_m` 0).
