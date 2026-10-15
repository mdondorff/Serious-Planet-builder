# 0038 HDR target, exposure and tone mapping
Status: proposed (sketch by opus-planner, 2026-10-15, for M5, revised after the spec-guardian and numerics reviews; decided under the owner's delegation of 2026-10-12 once the M5 queue starts; owner's M5 decisions of 2026-10-15 apply: physical defaults as data, look reviewed at the gate, ADR 0037)
Date: 2026-10-15
Source: architecture report M5 row (HDR and exposure); `docs/status.md` M5 section; ADR 0013, ADR 0035, ADR 0037, ADR 0042; constitution CON-02, CON-03, CON-04, CON-12, CON-18, CON-19, CON-20

## Context
Every pass today writes `Rgba8Unorm` debug colours. M5 lights the planet with the sun and a physically based sky; radiances span about ten orders of magnitude, so the scene needs a linear HDR target, an exposure and a tone curve. Goldens must stay deterministic, so the exposure that feeds them must not depend on frame history.

## Decision (proposed)
1. **Scene target:** linear `Rgba16Float`, radiance in units of top-of-atmosphere solar illuminance (= 1, as Hillaire) times a **pre-exposure** factor applied when writing; every write is clamped to 60,000 so the sun disk can never produce `inf` or `NaN` in `f16` (which would poison the histogram). Depth stays reversed-Z `Depth32Float` (CON-12), asserted by a test.
2. **Exposure:** automatic, from a luminance histogram of the scene target (compute pass; 64 bins of log2 luminance over EV -10 to +20; the mean of the 40 % to 90 % percentile band), clamped to an EV100 range (data, default -2 to 16) and adapted with separate speeds up and down (data, defaults 3 and 1 EV per second). Adaptation runs on the GPU as presentation data with `dt`, speeds and clamps taken from the `FramePlan` (CON-03, CON-04); no wall clock in `render`, no readback. For goldens, perf scenes and repro bundles a **fixed exposure** per view replaces the adaptive one (CON-19).
3. **Tone curve:** the ACES filmic fit (Narkowicz 2015) on exposed linear colour, then sRGB encoding **in the shader** into an `Rgba8Unorm` target (offscreen and goldens) or the surface (window, a non-sRGB view of it), so the window and the goldens encode identically. The curve is a parameter (look data, ADR 0042 point 6).
4. **Class and bounds** (ADR 0035): presentation data. Declared bound: the tone-map pass matches an f64 evaluation of the same formula within 1/255 per channel on a ramp image (tier B); the exposure maths (histogram percentile, clamp, adaptation step) are tier A on synthetic histograms in a GPU-free module (`frame::look`, CON-02).
5. **Debug views** (CON-18): false-colour luminance (EV bands) and the histogram overlay; existing debug views bypass tone mapping and keep their goldens (ADR 0042 point 1).
6. **Time box and fallback** (owner's M5 rule): one day; fallback: fixed exposure everywhere (adaptive exposure deferred), reported.

## Consequences
- One tone-mapping pass for every lit image; debug views stay byte-identical.
- The HDR target (29.5 MB at 1440p) and histogram buffers are counted in the frame-target budget of ADR 0042 point 5, outside terrain's share (ADR 0033).
- Tests: tier A exposure maths; tier B tone curve; tier C goldens per M5 view with fixed exposure.

## Licences of adopted code or techniques
ACES filmic fit: Narkowicz, "ACES Filmic Tone Mapping Curve" (2015), a published formula; no code adopted.
