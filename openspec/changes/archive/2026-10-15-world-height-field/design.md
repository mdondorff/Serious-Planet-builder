## Decisions

- The id lives in the definition beside the nodes, not as a node: `NodeValue` is `Copy` over `u64`/`f64`, and a string node would touch every read-set record. It is hashed into `base_hash` under the path `world.height_field` only when set, so existing definitions keep their hashes (WDEF-003 known answer unchanged). The path is reserved against nodes (`set` panics, TOML typing is a string).
- Validity of an id is decided where the implementations are known (generators), not at load time: a file naming a future field still loads and round-trips, and the refusal comes from `resolve` with the list of known ids.
- The default is the eroded field. The seed-only legacy invocations (`--seed N`) do not describe a world file; they name `cpu-ref-height-v2` explicitly because every existing golden, plan snapshot and bundle was made with it (not with `cpu-terrain-v1`, as the M4 queue text assumed: only tier-A/B tests in generators and frame use `cpu-terrain-v1`, and they construct it directly).
- A bake for the eroded and terrain fields comes from an in-memory `GlobalGrid` at `--global-res` (default 256) in `test-render`; the stored bake of `generate --plan-check` is unchanged.
