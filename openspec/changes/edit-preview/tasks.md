## 1. edit-preview
- [x] 1.1 `frame`: `PreviewStamp`, `preview_delta` (added, removed, changed; nearest 8), tests (REND-033 Delta)
- [x] 1.2 `frame`: the flag per node in `draw_plan_with`, `TerrainDraw.preview` (not in the recorded plan text), `DrawPlan.preview`, `draw_plan_preview`, `PlanPipeline::set_preview`; tests (REND-033 Previewed nodes)
- [x] 1.3 `render`: the globals table and the node flag, the profiles in the compact vertex stage, skipped with an empty list; the CPU evaluation as the reference (REND-033 Displacement, tier C, images to `target/review/preview/`; frame equalities on one adapter, no recorded hash)
- [ ] 1.4 (reviewer fixes: vertex up, displaced parent grid, plateau removal, whole items, even slope, the preview wired at the three submit sites) numerics-reviewer (shader and profiles), spec-guardian, verifier, `cargo xtask prepush`, PR
- [ ] 1.5 Preview latency (edit to first frame) in a perf window: only with opus-planner's go
