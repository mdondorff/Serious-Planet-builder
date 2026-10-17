## MODIFIED Requirements

### Requirement: VIEW-001 Adapter gate
The application SHALL choose its GPU before any device is created, on plain descriptions of the adapters (name, backend, device class, supported features, limits by name, and for a window whether the adapter can present to it), in a GPU-free function (CON-03). The interactive application SHALL accept a discrete GPU only and SHALL refuse an integrated or virtual GPU, a software renderer, an adapter missing a required feature or below a required limit (the limits are listed by name: among them `max_storage_buffer_binding_size` and `max_buffer_size` at the 128 MiB of a compact tile block, ADR 0054, and `max_storage_buffers_per_shader_stage` for the stage that reads the block), and an adapter that cannot present to the window, with an error that names the reason for every adapter turned down (the feature or limit by name and, for a limit, its value and the minimum) and, for an integrated GPU, the way out; it SHALL never crash on a refusal. The software adapters (WARP, lavapipe) SHALL run only in the test and CI mode (`PLANET_ADAPTER=software`, `editor --smoke`); the interactive editor SHALL refuse `--adapter software`. Among several acceptable adapters the choice SHALL be deterministic whatever order the platform lists them in (Vulkan before Dx12 before other backends, then by name). `render` SHALL enumerate and describe the adapters and request the device with the limits the gate checked; `GpuContext::new(HighPerformance)` SHALL use the same gate.

Verify: A
Status: active
Source: owner 2026-10-16, ADR 0047, ADR 0054, ADR 0015, ADR 0004, CON-03

#### Scenario: Discrete accepted
- **WHEN** the adapters are one discrete GPU with every required feature and limit
- **THEN** it is chosen

#### Scenario: Integrated refused
- **WHEN** the only adapter is an integrated GPU
- **THEN** selection fails with a message that names the adapter, says it is an integrated GPU and not a discrete one, and points to the High performance setting

#### Scenario: Missing feature or limit
- **WHEN** a discrete GPU lacks a required feature or reports a limit below the minimum
- **THEN** it is refused, naming the feature, or the limit with its value and the minimum

#### Scenario: Software only in test mode
- **WHEN** the adapters are a software renderer and a discrete GPU
- **THEN** interactive selection takes the discrete GPU and refuses the software renderer by name; test-mode selection takes the software renderer

#### Scenario: Several adapters
- **WHEN** an integrated GPU and the same discrete GPU on Dx12 and Vulkan are listed in any order
- **THEN** the same discrete adapter is chosen every time, the Vulkan one
