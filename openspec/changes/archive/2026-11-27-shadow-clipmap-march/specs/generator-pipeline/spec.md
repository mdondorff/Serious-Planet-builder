## REMOVED Requirements

### Requirement: GEN-044 The chunk horizon map
**Reason**: ADR 0085 point 7: the horizon maps are retired; the shadow march reads the clipmap (REND-049).
**Migration**: PR 8 of `shadow-clipmap-march`, after PR 7 removed its last consumer (the horizon jobs).

### Requirement: GEN-045 The coarse horizon from the global run
**Reason**: ADR 0085 points 4 and 7: the coarse cube of max heights (REND-048) replaces it.
**Migration**: PR 8. The coupled run (GEN-040) stays; the stored coarse horizon channel stays on disk unread (STRM-045's removal).

### Requirement: GEN-046 The horizon of a point
**Reason**: ADR 0085 point 7; shadows are presentation data with no CPU twin (CON-14 v1.3). Its helpers (`horizon_lookup::{interpolate, point_horizon, lit_factor, azimuth_in_frame, sun_elevation, face_frame}`) have no caller outside the horizon maps (checked at main 10db3b63; the only other user, `examples/coarse_probe.rs`, goes in the same PR).
**Migration**: PR 8, with its last consumer; a helper found shared at that PR moves to a surviving requirement first.

### Requirement: GEN-085 The horizon march walks a step table
**Reason**: ADR 0085 point 7; the bake it speeds up is removed.
**Migration**: PR 8.

### Requirement: GEN-086 The horizon march has an AVX2 kernel with the bytes of the scalar one
**Reason**: ADR 0085 point 7; the march it vectorises is removed (synced from `m8-horizon-march-simd` at its archive, 2026-11-01).
**Migration**: PR 8.
