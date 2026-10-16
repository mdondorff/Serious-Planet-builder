## MODIFIED Requirements

### Requirement: REND-024 Lit plan
The `FramePlan` (`TerrainPlan`) SHALL carry the lit path's decisions in an optional `LitPlan`: the exposure mode (fixed with its EV100, or adaptive with its range, speeds and the CPU-chosen pre-exposure EV100), `dt` in seconds from the injected clock, the sun direction, a reset flag, the tone-map mode, the look's scene-unit scale and histogram axis and band (LOOK-003), and the base shading's albedo table, water level and debug view (ADR 0039). The sun direction comes from the look's sun (LOOK-001) or from a time of day (`sun_direction`, decided in f64 with `libm` and rounded once to f32). Its text form SHALL be exact (floats as bit patterns) and round-trip, a plan without it SHALL keep its text and hash, and a lit plan made with the Earth-default look SHALL keep the text it had before the look file existed (the look's tokens, the albedo table, the water level and the debug view are written, in that order, only when they differ from the default). A plan with it SHALL be refused by a renderer that has not enabled the lit path, with an error naming `enable_lit`.

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
The terrain fragment stage of the lit path SHALL compute radiance in units of the sun's illuminance as Lambert diffuse `albedo x max(n . sun, 0) / pi` of the normalised shading normal under the plan's sun direction (ADR 0039 point 4: no ambient, no terrain shadows, no specular; the sky's light comes with ADR 0040), where the albedo is `AlbedoParams::albedo` of the climate at the fragment, the cosine of the slope (the angle between the normal and up) and the height above the plan's water level W (ADR 0039 point 3, LOOK-001's `albedo` table): ground from the climate (desert, grass, forest, tundra), bare rock on steep ground, snow by temperature and slope, a beach band above W and the seabed below it. It SHALL multiply by the plan's pre-exposure factor and clamp each channel to [0, 60000] (ADR 0038 point 1). Debug views (CON-18) of the plan SHALL show the albedo alone, the lighting alone (a white surface), the temperature and the precipitation (false colours, written as if lit by a sun facing the surface). Class: presentation data (ADR 0035). Bound: per channel `|GPU - f64 reference| <= max(2e-4 |v|, 2e-5 x factor)` on probe inputs that include unnormalised normals, climates over the whole range, heights above and far below the water, grazing and back-facing suns, two albedo tables and pre-exposures that reach the clamp (the reference sees the f32 inputs and the f32 table the shader receives).

Verify: B
Status: active
Source: ADR 0038 point 1, ADR 0039 points 1, 3 and 4, ADR 0035, ADR 0043, CON-11, CON-18

#### Scenario: Probe set
- **WHEN** 970 probe inputs in all five views are evaluated by the shader's own function (`shade_lit`) and by `look::shading::terrain_radiance` with `look::written_value`
- **THEN** every channel is within the bound, and the set covers clamped, dark and lit results and every view

#### Scenario: Known answers
- **WHEN** a flat warm dry surface, a humid one, a wet one, a steep one, a cold flat one, a cold steep one, a shore and a seabed are evaluated, and a surface faces the sun, faces away or is tilted by 60 degrees
- **THEN** the albedo is exactly the desert, grass, forest, rock, snow, rock, beach and seabed colour, halfway through a transition it is halfway between the two colours, and the radiance is albedo/pi, zero and half


### Requirement: REND-026 Lit terrain frame
A plan with a `LitPlan` SHALL be drawn into the `Rgba16Float` scene target with the same geometry and the same reversed-Z `Depth32Float` test as the debug path (the same pixels covered), every written value finite and within [0, 60000], and tone mapped into the renderer's colour target; the debug path of the same renderer SHALL stay byte-identical, before and after lit frames and compared with a renderer that never enabled the lit path. A lit frame at a fixed exposure SHALL be a pure function of its plan; the sun and the exposure SHALL come from the plan. Lit frames of the orbit, aerial and ground views, the EV-band view and the base shading's debug views (temperature, precipitation, albedo and lighting) SHALL match the adapter's goldens (class lit, first goldens of M5: blessed by `golden-review` under ADR 0037's policy, escalated to the owner's `/bless` otherwise). After `enable_lit` no frame SHALL create a pipeline, target or ring.

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

## ADDED Requirements

### Requirement: REND-027 Climate and slope reach the fragment stage
Both tile formats SHALL deliver the tile's climate field (GEN-021) to the fragment stage: the compact tile's fragment stage reads the 9 x 9 texture from the fourth channel of the atlas texels of the tile's slot (the tile reference and the atlas are visible to the fragment stage) and interpolates it bilinearly at the fragment's tile coordinates; the vertex format's GPU vertex is 48 bytes (the 44-byte vertex and the vertex's climate as two f16, `MeshVertex::GPU_STRIDE`; 58,608 B per tile at cells 32), the value at each vertex being the stored texture sampled at the vertex (`TileMesh::gpu_climate`) and interpolated across the triangles. The node record (208 bytes) SHALL carry the unit up vector of the tile origin and the inverse of its distance from the planet centre, decided in f64 and rounded once; the fragment stage SHALL form up as `normalize(up0 + local x inv_r)` from the tile-local position, so no planet-radius value reaches the stage (CON-11 v1.4). Class and bound of each attribute (ADR 0035): the stored climate texture is a **field** (GEN-021: the GPU decode is exact); the climate at a fragment is presentation data, the bilinear interpolation of the stored texels (the climate views agree with the world's climate colour with a mean error of at most 0.02 on the orbit view and 0.005 on the 20 km view, and the two formats shade the same picture: at most 0.5 % of covered pixels differ by more than 5 % of a lit unit); the up vector is presentation data within 5e-7 rad of the exact radial direction at every tile-local offset up to 1 km from the origin of a tile at 8,848 m or at -11,000 m, anywhere on the planet; temporal stability at 1 m (CON-11): the slope input changes by at most 3e-7 between fragments 1 mm apart at the worst case (a finest-level tile far from the origin), the albedo selected from it by at most 1/255, two tiles that cover one point agree about up within 5e-7 rad, and the shading depends only on tile-local quantities, never on the camera; the albedo, the radiance, the sun direction (f32, 6e-8) and the water level (f32) are presentation data with the bounds of REND-025 and LOOK-003.

Verify: B, C
Status: active
Source: ADR 0039 points 2 and 3, ADR 0035, ADR 0043, ADR 0033 point 4, CON-11, CON-14, REND-016

#### Scenario: Up vector
- **WHEN** 1,584 tile origins and offsets, anywhere on the planet and at 8,848 m, -11,000 m and sea level, are evaluated by the shader's `up_at` and compared with the exact radial direction
- **THEN** every angle is at most 5e-7 rad (measured 1.4e-7) and every vector is a unit vector

#### Scenario: Stability at 1 m
- **WHEN** 1,001 fragments 1 mm apart along three axes of a tile at 8,848 m are evaluated against a surface at the middle of the soil-to-rock transition
- **THEN** the slope input changes by at most 3e-7 and the albedo by at most 1/255 between neighbours, and two neighbouring tiles agree about up at one point within 5e-7 rad

#### Scenario: Climate views
- **WHEN** the temperature and precipitation views of the orbit and the 20 km view are drawn in both formats
- **THEN** their colours are the colours of the world's climate at the point under each pixel within the bounds above

#### Scenario: Formats
- **WHEN** every view is drawn from compact and from vertex tiles
- **THEN** the pictures agree within the bound above

