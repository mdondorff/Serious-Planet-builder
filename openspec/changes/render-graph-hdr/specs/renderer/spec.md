## ADDED Requirements

### Requirement: REND-019 Lit frame graph
The lit path SHALL be declared as a render graph in `render::graph` (`lit_frame_graph`, a function of the frame size and the exposure mode) with the passes `terrain` (writes the scene target `hdr` and `depth`), `exposure-histogram` (reads `hdr`), `exposure-adapt` (reads the histogram only under adaptive exposure; writes the persistent exposure state) and `tonemap` (reads `hdr` and the exposure state; writes the presentable target; a side effect). Compiling it SHALL order the passes, cull the histogram pass under fixed exposure, compute lifetimes and alias groups, and reject a read without a producer (REND-012). The executor SHALL record passes in the compiled order and SHALL compile again only when the frame shape (size or exposure mode) changes, not every frame.

Verify: A
Status: active
Source: ADR 0042 points 1 and 2, ADR 0013, REND-012, CON-03

#### Scenario: Adaptive frame
- **WHEN** the adaptive lit frame is compiled
- **THEN** the order is terrain, exposure-histogram, exposure-adapt, tonemap and nothing is culled

#### Scenario: Fixed exposure
- **WHEN** the fixed-exposure lit frame is compiled
- **THEN** the histogram pass is culled by the compile and the order is terrain, exposure-adapt, tonemap

#### Scenario: Recompile on a shape change only
- **WHEN** three frames of one shape, a frame of another exposure mode and three more of it are executed
- **THEN** the graph is compiled twice

### Requirement: REND-020 Frame-target share
The bytes of the frame targets that the compiled frame graph allocates (transient resources unaliased, plus persistent ones such as the exposure history), besides terrain's pools and the presentable target, SHALL NOT exceed `40 B x pixels + 32 MiB` of the internal resolution (`frame_target_share_bytes`; ADR 0048, an amendment to ADR 0033's remainder). The count SHALL come from the compiled graph (`CompiledGraph::frame_target_bytes`), and the bytes the renderer allocates for the lit path SHALL equal it. A frame that grows beyond the share SHALL fail the test with the byte counts.

Verify: A
Status: active
Source: ADR 0042 point 5, ADR 0033, ADR 0048, CON-20

#### Scenario: 1440p numbers
- **WHEN** the adaptive lit frame at 2560x1440 is compiled
- **THEN** the scene target is 29,491,200 B, the depth target 14,745,600 B, the histogram 256 B, the exposure history 16 B, the total 44,237,072 B and it is within the share of 181,010,432 B

#### Scenario: A frame that outgrows the share
- **WHEN** a frame declares eight further HDR-sized targets
- **THEN** its count exceeds the share

#### Scenario: Allocation equals the graph
- **WHEN** the lit path is enabled on a renderer
- **THEN** the bytes it holds for the scene target, depth, histogram and exposure history equal the compiled graph's frame-target bytes

### Requirement: REND-021 Exposure and tone-mapping maths
`frame::look` SHALL hold, in f64 with `libm`, the formulas the lit path's shaders implement: the exposure scale `1 / (1.2 x 2^EV100)`, the pixel EV `log2(8 L)`, the pre-exposure factor from scene units (one unit is 126,600 cd/m2), the 64-bin histogram over EV -10 to +20 (pixels without light not counted, out-of-range pixels in the end bins), the mean EV of the band between 40 % and 90 % of the counted pixels, the target clamped to the EV100 range (held when nothing is counted), the adaptation step (linear in EV, at most `speed_up x dt` up and `speed_down x dt` down, snapping on a reset), the ACES filmic fit (Narkowicz 2015), the sRGB transfer function and the 8-bit tone-mapped pixel. Class: presentation data (ADR 0035); the pre-exposure factor is decided in f64 and rounded once to f32 for the shader.

Verify: A
Status: active
Source: ADR 0038 points 1-4, ADR 0048 points 2-4, ADR 0035, CON-03, CON-14

#### Scenario: Known answers
- **WHEN** the units, bins, band means, clamps, adaptation steps, ACES and sRGB are evaluated on hand-worked inputs
- **THEN** each equals its worked answer (EV 0 exposure 1/1.2, L = 1 is EV 3, ACES(1) = 2.54/3.16, sRGB(0.5) = 0.735357)

#### Scenario: Adaptation
- **WHEN** the EV is 10, the target 16 and dt 0.5 s with the default speeds, and the target 2 for the other direction
- **THEN** the EV becomes 11.5 and 9.5, a close target is reached without overshoot, no time means no change and a reset snaps

### Requirement: REND-022 Exposure histogram and adaptation on the GPU
The exposure histogram pass SHALL count the scene target's pixels into the 64 bins of REND-021 and the adaptation pass SHALL compute the band mean, the clamped target and the adapted EV from the histogram, with `dt`, the speeds, the clamps, the mode and the pre-exposure taken from the plan (no wall clock, no readback). Class: presentation data. Bound: the GPU histogram equals `look::histogram_of` bin for bin on pixels away from bin edges (on a terrain frame at most 0.5 % of the counts may sit in a neighbouring bin, the total is exact); the adapted and target EV equal the CPU step within 1e-4 EV; the same plan sequence gives the same exposure bit for bit; fixed exposure ignores the scene and the history. The adaptation sums bin counts in f32, exact below 2^24 counted pixels (4K is 8.3 million); larger internal resolutions need an integer running sum first. A plan with non-finite or negative time, a reversed range, negative speeds or a non-unit sun is refused with the value named (REND-024).

Verify: B
Status: active
Source: ADR 0038 points 2 and 4, ADR 0048 point 4, CON-04

#### Scenario: Histogram against the reference
- **WHEN** an image of pixels at bin centres, black pixels and out-of-range pixels is written to the scene target and the adaptive frame runs
- **THEN** every bin equals the CPU reference and a second frame gives the same counts

#### Scenario: Adaptation sequence
- **WHEN** scenes of EV 8, 14, 14, 3, 3, 25, 25 and black follow each other with different dt
- **THEN** the GPU EV and target match the CPU chain frame by frame and never pass the range maximum

#### Scenario: Fixed exposure
- **WHEN** a fixed-exposure frame runs on a dark and on a bright scene
- **THEN** the EV and the target are the fixed EV and the factor is 1

### Requirement: REND-023 Tone map
The tone-map pass SHALL read the scene target and the exposure state, expose the value (`written x 2^(pre - ev)`), apply the ACES filmic fit and the sRGB transfer function in the shader, and write an `Rgba8Unorm` target (not an sRGB format), so the window and the goldens encode identically. A false-colour mode SHALL show the exposed luminance in whole-stop bands (`look::EV_BAND_COLORS`) as the tone map's debug view (CON-18). Class: presentation data. Bound: within 1/255 per channel of `look::tonemap_pixel` on a ramp image at several exposures and through the adaptive exposure the GPU reports; band colours exact.

Verify: B
Status: active
Source: ADR 0038 points 3-5, CON-18

#### Scenario: Ramp
- **WHEN** a log ramp from 1e-3 to 50 is tone mapped at EV 10 and EV 13.5
- **THEN** every channel is within 1/255 of the f64 reference

#### Scenario: Exposure difference
- **WHEN** the adaptive exposure lands on an EV other than the pre-exposure
- **THEN** the image equals the reference at `2^(pre - ev)` times the written values

#### Scenario: EV bands
- **WHEN** greys at the middle of each band, a clipped value and black are shown in the false-colour mode
- **THEN** each shows its band colour, the clipped value band 0 and black stays black

### Requirement: REND-024 Lit plan
The `FramePlan` (`TerrainPlan`) SHALL carry the lit path's decisions in an optional `LitPlan`: the exposure mode (fixed with its EV100, or adaptive with its range, speeds and the CPU-chosen pre-exposure EV100), `dt` in seconds from the injected clock, the sun direction, a reset flag and the tone-map mode. Its text form SHALL be exact (floats as bit patterns) and round-trip, and a plan without it SHALL keep its text and hash. A plan with it SHALL be refused by a renderer that has not enabled the lit path, with an error naming `enable_lit`.

Verify: A
Status: active
Source: ADR 0042 point 3, ADR 0048 point 3, CON-03, CON-04, CON-19

#### Scenario: Round trip
- **WHEN** a fixed and an adaptive lit plan are written to text and read back
- **THEN** they are equal

#### Scenario: Debug path unchanged
- **WHEN** a plan has no lit part
- **THEN** its text has no `lit` line and equals the text it had before, and a lit plan has exactly one more line

#### Scenario: Not enabled
- **WHEN** a lit plan is rendered by a renderer without the lit path
- **THEN** the render fails with an error that names `enable_lit`

### Requirement: REND-025 Lit terrain radiance
The terrain fragment stage of the lit path SHALL compute radiance in units of the sun's illuminance as Lambert of the (normalised) vertex normal under the plan's sun direction on the relief view's height tint scaled by 0.4, with no ambient, multiply by the plan's pre-exposure factor and clamp each channel to [0, 60000] (ADR 0038 point 1). It SHALL use only unit normals, heights and the plan's numbers, never a planet-sized value (CON-11; the shader scan of REND-005 covers it). Class: presentation data (placeholder until ADR 0039's albedo). Bound: per channel `|GPU - f64 reference| <= max(1e-4 |v|, 1e-5 x factor)` on probe inputs that include unnormalised normals, heights beyond both ends of the range, grazing and back-facing suns and pre-exposures that reach the clamp.

Verify: B
Status: active
Source: ADR 0038 point 1, ADR 0039 (placeholder), ADR 0043, ADR 0048 point 5, CON-11

#### Scenario: Probe set
- **WHEN** 404 probe inputs are evaluated by the shader's own function and by `look::lit_terrain_radiance` with `look::written_value`
- **THEN** every channel is within the bound and the set covers clamped, dark and lit results

#### Scenario: Known answers
- **WHEN** a surface faces the sun, faces away, is tilted by 60 degrees or lies beyond the tint range
- **THEN** the reference gives albedo/pi, zero, half and the end tint

### Requirement: REND-026 Lit terrain frame
A plan with a `LitPlan` SHALL be drawn into the `Rgba16Float` scene target with the same geometry and the same reversed-Z `Depth32Float` test as the debug path (the same pixels covered), every written value finite and within [0, 60000], and tone mapped into the renderer's colour target; the debug path of the same renderer SHALL stay byte-identical, before and after lit frames and compared with a renderer that never enabled the lit path. A lit frame at a fixed exposure SHALL be a pure function of its plan; the sun and the exposure SHALL come from the plan. Lit frames of the orbit, aerial and ground views and the EV-band view SHALL match the adapter's goldens (class lit, first goldens waiting for the owner's `/bless`). After `enable_lit` no frame SHALL create a pipeline, target or ring.

Verify: C
Status: active
Source: ADR 0038, ADR 0042 points 1-3, ADR 0037, ADR 0048, CON-12, CON-18, REND-017

#### Scenario: Coverage and range
- **WHEN** the scripted views are drawn in the vertex and compact formats on the lit path and on the debug path
- **THEN** the covered pixels are the same, every scene value is finite and at most 60,000, and the planet is lit

#### Scenario: Debug path untouched
- **WHEN** a renderer with the lit path draws a debug plan before and after lit frames
- **THEN** the image equals the image of a renderer that never enabled the lit path

#### Scenario: Pure function of the plan
- **WHEN** the same fixed-exposure lit plan is drawn twice, then with another sun, then two stops darker
- **THEN** the first two images are equal, the sun changes the image and the darker exposure is darker

#### Scenario: Formats
- **WHEN** the lit path's targets are inspected
- **THEN** the scene target is `Rgba16Float`, the tone-map output `Rgba8Unorm` and the depth `Depth32Float` with the Greater test
