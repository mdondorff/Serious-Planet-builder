# atmo-top-continuity

Why: the owner saw a tiny colour step between 95 and 100 km altitude (M6 test, P2-7). Cause: a camera inside the atmosphere is drawn from the sky table (32 quadratic steps) and the camera march (8 steps), from the top up by the orbit march (40 uniform steps); the three differ by up to 4 % near the limb, and the frame switches between them at the top.

What: across the 5 km below the top the composite mixes towards the orbit-style march from the camera (ATMO-023), so the picture at the top equals the orbit frame. CPU twin `FrameGeometry::near_top_weight` and `inside_orbit_style_cosines`; the shader mirrors them. Probe: `cargo run --release -p planet-render --example atmo_climb 98 102 0.02` prints the per-pixel step between consecutive altitudes.

Expected golden changes: none (no golden frame has the camera in the band; tier C confirms).
