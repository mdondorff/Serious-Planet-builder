## Decisions
- Schema version 6 (LOOK-002: a new key raises the version and adds an identity step, so an older build says "newer schema", not "unknown key"): `v5_to_v6`, a version 6 fixture, and the canonical text writes `patch` always (LOOK-011: every key), so no exception to LOOK-011 is needed. The plan's text and key carry the token only when it is not 0 (as `detail`), so plans without it keep their key.
- The shift is in slope-cosine units (the spike tried 0.12 and 0.25); the maximum accepted is 0.5.
- The patch octaves use the same planes and lattice turns as the detail octaves and fade at twice their footprint (`foot * 0.5`: the numerics review found the spike's `foot * 0.25` left 74 % amplitude at the Nyquist limit, which a sharp material table turns into flicker); they hash with salts 4 and 5. The "32 m" octave's lattice is turned, so its cells have sides of 22.6 m; the same for the 16 m one. `rough` and `tone` follow the shifted weights, and so would a gloss modifier on the slope input.
- The steepness term is `1 - smoothstep(0.5, 0.86, cs)`; it multiplies the rock's tilt at 2 m by `1 + 2 steep` and at 25 cm by `1 + steep`, only with the patch on. It is not a requirement: no tier A or C scene is steep enough to test it, and what it looks like is the owner's call on the stills (tier D). The rock tilt can reach about 55 to 60 degrees on a slope already past 60: a dark speck or a specular artefact at a grazing view is the thing to look for in the stills.
- Not in this slice: five materials (scree, wet ground), the channels, six octaves, geometry below 100 m.

## Evidence the off picture is today's
- The existing exact goldens of the lit ground at natural strength (`gpu_ground_look`: `lit_ground_eye1p8m/0p5m_natural`, the march shadow) are the bit-for-bit evidence for the patch 0 path; the new test only shows that the patch 0 plan equals the plain one. `cargo xtask test gpu` must make no candidate.

## Risks
- The patches are 8 and 32 m features: a 1 m view of a few metres sees a part of one patch, so the picture changes in places, by a lot where the boundary moves. The stills at 10 and 50 m show them.
