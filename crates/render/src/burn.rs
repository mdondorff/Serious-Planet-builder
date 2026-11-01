//! The GPU burn pass of the reserve test (BUDG-011, ADR 0083 point 3): a compute pass of fixed arithmetic whose time is calibrated with timestamp queries to the GPU
//! milliseconds of a reserved share (sky and weather 3.0 ms, physics 0.5 ms). It stands for the GPU work a subsystem will claim later. Only the plans of test scenes carry
//! it (CON-03: `planet_frame::reserve::ReservePlan` is a plan node of a synthetic scene; a production plan never has one). Without timestamp queries (the software adapter)
//! it cannot be calibrated and says so.

use crate::GpuContext;

/// Invocations per workgroup of the burn shader.
const WORKGROUP: u32 = 64;
/// Workgroups of every dispatch: enough to fill a laptop GPU; the time is tuned with the iteration count, not the size.
const GROUPS: u32 = 2048;

const SHADER: &str = "
struct Params { iterations: u32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> sink: array<f32>;

@compute @workgroup_size(64)
fn burn(@builtin(global_invocation_id) id: vec3<u32>) {
    var x = f32(id.x) * 0.001 + 1.0;
    for (var i = 0u; i < params.iterations; i = i + 1u) {
        x = fma(x, 1.0000001, 0.0000001);
        x = sin(x) * 0.5 + x;
    }
    sink[id.x] = x;
}
";

/// A calibrated burn: the iteration count that takes `measured_ms` on this GPU for a target of `target_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurnCalibration {
    pub iterations: u32,
    pub target_ms: f64,
    pub measured_ms: f64,
}

impl BurnCalibration {
    /// How far the measured time is from the target as a share of the target.
    pub fn error(&self) -> f64 {
        (self.measured_ms - self.target_ms).abs() / self.target_ms
    }
}

/// One burn: its pipeline and buffers, run with an iteration count.
pub struct BurnPass {
    pipeline: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    params: wgpu::Buffer,
    _sink: wgpu::Buffer,
    queries: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
}

impl BurnPass {
    pub fn new(ctx: &GpuContext) -> BurnPass {
        let device = &ctx.device;
        let module = device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("burn"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("burn-params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sink = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("burn-sink"),
            size: u64::from(GROUPS * WORKGROUP) * 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("burn"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("burn"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("burn"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("burn"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("burn"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: sink.as_entire_binding() },
            ],
        });
        let queries = ctx.timestamps_supported().then(|| {
            (
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("burn-timestamps"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("burn-resolve"),
                    size: 16,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("burn-readback"),
                    size: 16,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
            )
        });
        BurnPass { pipeline, bind, params, _sink: sink, queries }
    }

    /// Whether this pass can be calibrated (the adapter has timestamp queries).
    pub fn can_calibrate(&self) -> bool {
        self.queries.is_some()
    }

    fn record(&self, ctx: &GpuContext, encoder: &mut wgpu::CommandEncoder, iterations: u32, timed: bool) {
        ctx.queue.write_buffer(&self.params, 0, &[iterations.to_le_bytes(), [0; 4], [0; 4], [0; 4]].concat());
        let timestamp_writes = self.queries.as_ref().filter(|_| timed).map(|(q, _, _)| wgpu::ComputePassTimestampWrites {
            query_set: q,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("burn"), timestamp_writes });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(GROUPS, 1, 1);
    }

    /// Submit one burn of `iterations` on the queue and return at once (the GPU time adds to what the frame's own passes take).
    pub fn submit(&self, ctx: &GpuContext, iterations: u32) {
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("burn") });
        self.record(ctx, &mut encoder, iterations, false);
        ctx.queue.submit([encoder.finish()]);
    }

    /// The GPU time of one burn of `iterations` in milliseconds, from timestamp queries; waits for the GPU. `None` without timestamp queries.
    pub fn measure_ms(&self, ctx: &GpuContext, iterations: u32) -> Option<f64> {
        let (queries, resolve, readback) = self.queries.as_ref()?;
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("burn-timed") });
        self.record(ctx, &mut encoder, iterations, true);
        encoder.resolve_query_set(queries, 0..2, resolve, 0);
        encoder.copy_buffer_to_buffer(resolve, 0, readback, 0, 16);
        ctx.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("burn timestamp map failed"));
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        let data = slice.get_mapped_range().expect("burn timestamp range");
        let tick = |k: usize| u64::from_le_bytes(data[k * 8..k * 8 + 8].try_into().expect("8 bytes"));
        let (a, b) = (tick(0), tick(1));
        drop(data);
        readback.unmap();
        (b > a).then(|| (b - a) as f64 * f64::from(ctx.queue.get_timestamp_period()) / 1e6)
    }

    /// Tune the iteration count until one burn takes `target_ms` within `tolerance` (a share of the target), measuring the median of three runs each time; at most
    /// 12 rounds. `None` without timestamp queries or when no count reaches the target.
    pub fn calibrate(&self, ctx: &GpuContext, target_ms: f64, tolerance: f64) -> Option<BurnCalibration> {
        self.queries.as_ref()?;
        let median = |iterations: u32| -> Option<f64> {
            let mut v: Vec<f64> = (0..3).filter_map(|_| self.measure_ms(ctx, iterations)).collect();
            v.sort_by(f64::total_cmp);
            (v.len() == 3).then(|| v[1])
        };
        let mut iterations = 256u32;
        let mut best: Option<BurnCalibration> = None;
        for _ in 0..12 {
            let measured_ms = median(iterations)?;
            let cal = BurnCalibration { iterations, target_ms, measured_ms };
            if best.is_none_or(|b| cal.error() < b.error()) {
                best = Some(cal);
            }
            if cal.error() <= tolerance {
                return Some(cal);
            }
            // The time grows linearly with the iterations beyond a small fixed cost: scale towards the target.
            let scaled = f64::from(iterations) * (target_ms / measured_ms).clamp(0.25, 4.0);
            iterations = (scaled.round() as u32).clamp(1, 1 << 24);
        }
        best.filter(|b| b.error() <= tolerance * 2.0)
    }
}
