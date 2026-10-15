## Decisions

- **Velocity** = displacement over the samples of the last 400 ms divided by their time span; only positions enter, so a rotation (same position, bit for bit) gives exactly zero.
- **Look-ahead** 2 s, distance cap 50 km, speed floor 0.5 m/s. A dive is not extrapolated below half its altitude (a geometric descent covers 54 % of its altitude per second, so a linear 2 s would end under the ground). Tunable after the first dive and teleport measurements (ADR 0031 owner question 2).
- **Teleport** = a step between samples above 500 m plus 10 % of the altitude (a dive moves under 1 % of its altitude per frame at 30 fps). The history is cleared, so there is no prediction until two samples exist at the new place.
- **Where it runs**: the worker, after R in the same job, on the plan's cache; a changed prediction resubmits (as a changed request does); delivered in the same outcome as R so the set always matches the R it was made against.
- **Free share**: the loop counts missing tiles against `main_free` of the previous frame's stats; resident prefetch tiles are kept in the wants (free). The streamer already starts prefetch only while nothing else waits and never evicts R or D.
- **Not changed**: the streamer, the manager, `predicted_position` (reused for the extrapolation), `--stream` (stationary).
