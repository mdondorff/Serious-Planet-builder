## 1. live-definition
- [x] 1.1 `WorldTerrain` definition as `RwLock<Arc<WorldDefinition>>`, `set_definition` (items only), snapshot per `field()`; STRM-028
- [x] 1.2 Stamp stages per layer in `field()` (`StampStage<'a>`, `stamp_layers`, `LAYER_STACK` in world-def); STRM-028
- [x] 1.3 `PlanField::item_revision` and the world planners; the residency set follows it (test in `stamp_cache.rs`)
- [x] 1.4 Tests: swap, field in flight, another base (`crates/app/tests/live_definition.rs`)
- [ ] 1.5 Reviewers, verifier, `cargo xtask prepush`, PR; tell sonnet-crusader when it is in main
