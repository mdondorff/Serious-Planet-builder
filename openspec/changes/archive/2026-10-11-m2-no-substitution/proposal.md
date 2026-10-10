## Why

ADR 0009 review follow-up 5 (docs/status.md on change/adr-0009-review, owner asked): outside the height view the executor bound a dummy 1x1 texture for a draw whose tile resource was missing, so a plan and its resources could disagree without any error. Low item from the same list: `gpu_renderer` asserted a strictly positive GPU timestamp delta, which a fast frame can quantise to zero.

## What Changes

- `render::exec::check` requires a resource for every drawn tile in every view (`MissingTile`); the flat-colour views still bind a 1x1 layout placeholder that the shader never reads, documented as such, but nothing is substituted for missing data.
- The one existing test that drew a Face view without resources now supplies the resource (the plan and the resources agree).
- `gpu_renderer` accepts a zero GPU time (`0 <= g < 10 s`).
- Modified: REND-001 (scenario: missing resource in a flat-colour view).
