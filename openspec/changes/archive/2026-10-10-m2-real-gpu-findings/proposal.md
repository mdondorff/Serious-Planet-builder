## Why

The owner's first real-GPU runs (RTX 3080 Ti Laptop, Vulkan) found two things: `cargo xtask test gpu --real` failed one test (hello triangle: the shader wrote 0.5, which is 127.5 in 8 bits, and NVIDIA rounds it to 127 where WARP gives 128), and the performance skeleton log was taken under the Balanced power scheme with the GPU idle in P8 at 210 MHz, so it says nothing about budgets.

## What Changes

- The hello-triangle shader writes `128/255` so the colour is exact on every adapter.
- The performance harness marks a session `valid_for_budgets: false` with warnings when the power scheme is not a performance scheme (English and German names) or a run starts with the GPU in P5 or lower, and prints the warnings.
- TEST-006 states this; two tests.

## Capabilities

### Modified Capabilities
- `testing`: TEST-006.

## Impact

`crates/render` (one shader constant), `crates/perf`, `crates/app`.
