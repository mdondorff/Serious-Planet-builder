## Why

ADR 0004 review (relayed by the SMB exploration session on behalf of the owner): `render` re-exported `wgpu` and exposed `device`, `queue` and `info` publicly, so any crate could reach the graphics API. This change closes the outer boundary of ADR 0004's seam (no wgpu type crosses render's API); it does not build the internal device/buffer/pipeline interface the ADR also describes (see ADR 0004).

## What Changes

- `render`: `pub use wgpu` removed; `GpuContext` fields private; accessors `adapter_name`, `backend_name`, `device_type_name`, `adapter_log_line`; `validate_wgsl` (the shader-validation oracle no longer touches the device itself); `read_rgba8` becomes crate-private.
- `app`: uses the accessors (the adapter log line is byte-identical to before).
- `xtask layering`: no crate other than `render` may list `wgpu`, `wgpu-core`, `wgpu-hal`, `naga` or `ash` in any dependency section (dev and build included; table form, quoted keys and renamed packages are caught), for every crate and xtask; and no public item in render's sources may name a wgpu type.
- Modified: BUILD-001.
- No pixel changes expected; goldens stay byte-identical.
