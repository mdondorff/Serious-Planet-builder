# SPIKE-100 Ground detail and materials for M9 (I8a, I8b)

Opened: 2026-11-04
Question: what geometry detail below the erosion layer's 101.8 m grid and what material mix from the baked channels give the ground the variety of GLO-30 and of the realism reference (s7 sunset), at what GPU cost?
Why it matters: M9 items I8a (ground detail) and I8b (materials from the baked channels, GEN-110 drainage area and GEN-111 erosion/deposition depth); the owner found SPIKE-67's detail uniform and low-frequency (owner, 2026-11-04; planner inbox #82).
Time box: about 1 day of agent work.
Success criteria: per relief class, plains included, the runtime detail's std per band (30-200 m) and 3 km relief against GLO-30; a multi-scale design for 1.8-50 m that varies with slope, the two channels and the material; a material mix (rock, scree, soil, grass, wet ground) from slope, drainage area and deposition depth; the GPU cost per frame against the REND budgets; what stays runtime and what is baked; stills from the viewer path, anything not in the product labelled; a ranked design.
Fallback if it fails: keep SPIKE-67's shader detail and the current materials; record the measured gap.
Code location: branch `spike/spike-100-ground-detail` (throwaway; never merged into the main crates). `spikes/ground-detail/` and `spike/ground-detail` are SPIKE-27's (M7 G1), read for reference only.
