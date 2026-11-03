## 1. Render
- [x] 1.1 Delete `render::{horizon, horizon_wgsl}`, the bindings 4 to 7, the horizon table and the setters; drop `vis` from the lighting WGSL; renumber the views in the shaders
- [x] 1.2 `MeasureVariant`: `Product` and `NoMaterial`
- [x] 1.3 Delete `tests/gpu_horizon.rs` and the horizon synthetic goldens

## 2. Frame and app
- [x] 2.1 Delete `frame::horizonview`; `ShadeView::Horizon` out; J's cycle, key text, title
- [x] 2.2 The viewer starts no horizon jobs and feeds no horizon data; delete `HorizonRig`
- [x] 2.3 Stills: no horizon wait, no horizon columns; `shadow-cost` times the material map

## 3. Specs
- [x] 3.1 REND-038, REND-041, REND-043 removed; REND-044, VIEW-054, TEST-048 reworded
- [ ] 3.2 Archive after the merge
