# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives in `spikes/<name>/` and is never merged into the main crates as-is.

Planned spikes from the background report (open them when their milestone starts):
- GPU-driven indirect terrain/instance path in wgpu. Milestone: M2.
- CDLOD morph quality on steep terrain. Milestone: M2.
- Global hydrology on the sphere (stream-power on a 1–4 km grid + halo-based local carving). Milestone: M3. Fallback: local erosion filter + rivers on a coarse flow grid.
- City HLOD with 10^5 buildings. Milestone: M7.
