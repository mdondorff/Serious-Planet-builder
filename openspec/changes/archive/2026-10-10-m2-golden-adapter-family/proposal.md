## Why

ADR 0009 review follow-up 1 (docs/status.md on change/adr-0009-review, owner asked): `adapter_key()` included the LLVM version on Linux (`vulkan-llvmpipe-llvm-20-1-2-256-bits`), so a Mesa update on the runner image would orphan every Linux golden, and `assert_golden` would pass silently as PENDING.

## What Changes

- Golden keys name the adapter family: parenthesised driver and compiler details are dropped (`vulkan-llvmpipe`). `tests/goldens/vulkan-llvmpipe-llvm-20-1-2-256-bits` moves to `tests/goldens/vulkan-llvmpipe` with `git mv` (images byte-identical, no re-bless).
- The full driver string (name, backend, type, driver, driver info) is written to `driver.txt` beside the goldens (and to the candidate folder on every run) and printed on every mismatch or missing golden.
- `PLANET_REQUIRE_ADAPTER_GOLDENS=1` (set in CI) fails a run whose adapter has no golden directory; pending-listed names do not excuse a missing directory.
- CI pins `ubuntu-24.04` for all Linux jobs and prints the installed Mesa package version; pinning the Mesa package itself is not done (the runner image's apt index drops old versions; the sidecar and the failing check make a Mesa change visible instead).
- Modified: TEST-002.
