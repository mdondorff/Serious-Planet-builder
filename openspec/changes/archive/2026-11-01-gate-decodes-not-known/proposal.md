# gate-decodes-not-known

## Why
- The gate listed main-thread bake-block decodes as KNOWN (ceiling 6): frame-102263 showed 2 in 2 frames and frame-197238 showed 1 in 1 frame on 2026-11-01 (before #1047).
- On main with #1047 (camera-ground-off-main), 3 gate runs of frame-102263, frame-116591 and frame-197238 on the real store (2026-11-01) show 0 decodes in 0 frames each. No decode phase remains to move off the main thread; the plan's origin-height read did not decode in any run.

## What
- Removed the KNOWN entry "bake blocks decoded" and its ceiling parser. A main-thread decode (VIEW-061) now fails the gate at every site.
- Tier-A test: a decode failure is new at every site (TEST-063).
- No spec delta.
