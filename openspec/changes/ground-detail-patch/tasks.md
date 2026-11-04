## 1. Tasks
- [x] 1.1 `ground.patch` in the look file (LOOK-012) and `LitPlan::ground_patch` (REND-055): parse, validate, write when on, plan text and key; tier A
- [x] 1.2 The shader: the patch octaves and the steepness growth behind `g.detail.z`; the globals; tier C (off is inert, on moves the boundary, strength 0 returns first)
- [x] 1.3 Stills poses `[look] ground_patch`
- [x] 1.4 Stills before/after (steep and flat sites, 1.8, 10 and 50 m; `docs/dev_screenshots/`); the GPU time follows with slice 2 (off by default, two octaves behind a uniform branch; the planner: the cost harness takes a look file and a poses file first)
- [x] 1.4b Reviews (`numerics-reviewer`, `spec-guardian`) applied
- [ ] 1.5 `verifier`, prepush, PR; was: reviews (`numerics-reviewer` on the shader, `spec-guardian`), `verifier`, prepush, PR
- [ ] 2.1 (slice 2 or its own small PR, planner 2026-11-04) `perf stream shadow-cost` or the stills cost path takes a look file and a poses file, so the D1 + D2 cost can be measured on the sanctioned harness against REND-034's 1.0 ms
