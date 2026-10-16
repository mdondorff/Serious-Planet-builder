//! Exploration: does WARP crash with several devices in one process, and what is enough to trigger it? Throwaway.
//! `warp-crash --adapter warp|rtx-dx12|rtx-vulkan --threads N --iters K --level 1|2|3 [--shared-instance] [--keep]`
//! level 1: create a device and drop it; level 2: also compile a compute pipeline, dispatch and read back; level 3: also a 64 MB
//! storage buffer written and copied. Each thread builds its own Instance (as `GpuContext::new` does) unless --shared-instance.
//! --keep: threads keep their devices until every thread has finished its work (no teardown during other threads' work).

use std::ffi::c_void;
use std::sync::{Arc, Barrier};

mod probe {
    use super::*;
    #[repr(C)]
    pub struct ExceptionRecord {
        code: u32,
        flags: u32,
        record: *mut ExceptionRecord,
        address: *mut c_void,
        n: u32,
        info: [usize; 15],
    }
    #[repr(C)]
    pub struct ExceptionPointers {
        record: *mut ExceptionRecord,
        context: *mut c_void,
    }
    extern "system" {
        fn AddVectoredExceptionHandler(first: u32, h: extern "system" fn(*mut ExceptionPointers) -> i32) -> *mut c_void;
        fn GetModuleHandleExW(flags: u32, addr: *const c_void, out: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(m: *mut c_void, buf: *mut u16, n: u32) -> u32;
        fn GetCurrentThreadId() -> u32;
        fn RtlCaptureStackBackTrace(skip: u32, n: u32, frames: *mut *mut c_void, hash: *mut u32) -> u16;
    }
    fn module_of(addr: *const c_void) -> String {
        unsafe {
            let mut m: *mut c_void = std::ptr::null_mut();
            if GetModuleHandleExW(4 | 2, addr, &mut m) == 0 {
                return format!("{addr:p} (no module)");
            }
            let mut buf = [0u16; 260];
            let n = GetModuleFileNameW(m, buf.as_mut_ptr(), 260) as usize;
            let name = String::from_utf16_lossy(&buf[..n]);
            format!("{}+{:#x}", name.rsplit('\\').next().unwrap_or(&name), addr as usize - m as usize)
        }
    }
    extern "system" fn handler(p: *mut ExceptionPointers) -> i32 {
        unsafe {
            let r = &*(*p).record;
            if r.code == 0xC000_0005 {
                let mut frames = [std::ptr::null_mut::<c_void>(); 64];
                let n = RtlCaptureStackBackTrace(0, 64, frames.as_mut_ptr(), std::ptr::null_mut());
                let mut s = format!(
                    "\n=== ACCESS VIOLATION thread {} ({:?}) at {} ; {} of {:#x}\n",
                    GetCurrentThreadId(),
                    std::thread::current().name(),
                    module_of(r.address),
                    if r.info[0] == 0 { "read" } else { "write" },
                    r.info[1]
                );
                for f in &frames[..n as usize] {
                    s += &format!("   {}\n", module_of(*f));
                }
                eprintln!("{s}");
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
            }
        }
        0
    }
    pub fn install() {
        unsafe {
            AddVectoredExceptionHandler(1, handler);
        }
    }
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn flag(name: &str) -> bool {
    std::env::args().any(|x| x == name)
}

const SHADER: &str = "@group(0) @binding(0) var<storage, read_write> d: array<f32>;
@compute @workgroup_size(64) fn cs(@builtin(global_invocation_id) id: vec3<u32>) { if (id.x < arrayLength(&d)) { d[id.x] = d[id.x] * 2.0 + 1.0; } }";

fn adapter_for(instance: &wgpu::Instance, which: &str) -> wgpu::Adapter {
    match which {
        "warp" => pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            force_fallback_adapter: true,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .expect("warp"),
        other => {
            let backend = if other == "rtx-vulkan" { wgpu::Backends::VULKAN } else { wgpu::Backends::DX12 };
            pollster::block_on(instance.enumerate_adapters(backend))
                .into_iter()
                .find(|a| a.get_info().name.contains("3080"))
                .expect("rtx")
        }
    }
}

fn work(device: &wgpu::Device, queue: &wgpu::Queue, level: u32) {
    if level == 7 {
        render_tex(device, queue);
        return;
    }
    if level >= 4 {
        render_work(device, queue, level);
        return;
    }
    if level < 2 {
        return;
    }
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("cs"),
        compilation_options: Default::default(),
        cache: None,
    });
    let n: u64 = if level >= 3 { 16 << 20 } else { 4096 }; // floats
    let bytes = n * 4;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let rb = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buf, 0, &vec![0u8; bytes as usize]);
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }],
    });
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(((n as u32) / 64).min(65535), 1, 1);
    }
    enc.copy_buffer_to_buffer(&buf, 0, &rb, 0, bytes);
    queue.submit([enc.finish()]);
    rb.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    drop(rb.slice(..).get_mapped_range().expect("range"));
    rb.unmap();
}

const RENDER_SHADER: &str = "@vertex fn vs(@builtin(vertex_index) i: u32, @builtin(instance_index) inst: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[i % 3u] * 0.5 + vec2<f32>(f32(inst) * 0.01), 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }";

/// level 4: a render pass with colour and depth and one indexed draw; 5: eight records through multi_draw_indexed_indirect;
/// 6: timestamp queries around the pass, resolved and read back.
fn render_work(device: &wgpu::Device, queue: &wgpu::Queue, level: u32) {
    use wgpu::util::DeviceExt;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()) });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let size = wgpu::Extent3d { width: 128, height: 96, depth_or_array_layers: 1 };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let index: Vec<u8> = [0u32, 1, 2].iter().flat_map(|x| x.to_le_bytes()).collect();
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &index, usage: wgpu::BufferUsages::INDEX });
    let args: Vec<u8> = (0..8u32).flat_map(|k| [3u32, 1, 0, 0, k]).flat_map(|x| x.to_le_bytes()).collect();
    let abuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &args, usage: wgpu::BufferUsages::INDIRECT });
    let queries = (level >= 6).then(|| device.create_query_set(&wgpu::QuerySetDescriptor { label: None, ty: wgpu::QueryType::Timestamp, count: 2 }));
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let tsread = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let colread = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 128 * 4 * 96,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let cv = color.create_view(&Default::default());
    let dv = depth.create_view(&Default::default());
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &cv,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &dv,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: queries.as_ref().map(|q| wgpu::RenderPassTimestampWrites {
                query_set: q,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
        if level >= 5 {
            pass.multi_draw_indexed_indirect(&abuf, 0, 8);
        } else {
            pass.draw_indexed(0..3, 0, 0..1);
        }
    }
    if let Some(q) = &queries {
        enc.resolve_query_set(q, 0..2, &resolve, 0);
        enc.copy_buffer_to_buffer(&resolve, 0, &tsread, 0, 16);
    }
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &colread,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(128 * 4), rows_per_image: Some(96) },
        },
        size,
    );
    queue.submit([enc.finish()]);
    colread.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    if queries.is_some() {
        tsread.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    }
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
}

const TEX_SHADER: &str = "@group(0) @binding(0) var<storage, read> nodes: array<vec4<f32>>;
@group(0) @binding(1) var atlas: texture_2d<u32>;
@vertex fn vs(@builtin(vertex_index) i: u32, @builtin(instance_index) inst: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    let t = textureLoad(atlas, vec2<i32>(i32(i), i32(inst)), 0);
    let n = nodes[inst % 4u];
    return vec4<f32>(p[i % 3u] * 0.5 + vec2<f32>(f32(t.x & 255u) * 0.001 + n.x * 0.01), 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }";

/// level 7: like 5 but the vertex shader reads a storage buffer and `textureLoad`s a Rg32Uint atlas written by `write_texture`
/// (what the compact tile path does).
fn render_tex(device: &wgpu::Device, queue: &wgpu::Queue) {
    use wgpu::util::DeviceExt;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(TEX_SHADER.into()) });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let size = wgpu::Extent3d { width: 128, height: 96, depth_or_array_layers: 1 };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let atlas = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: 408, height: 408, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for k in 0..8u32 {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &atlas, mip_level: 0, origin: wgpu::Origin3d { x: k * 17, y: 0, z: 0 }, aspect: wgpu::TextureAspect::All },
            &vec![k as u8; 17 * 17 * 8],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(8 * 17), rows_per_image: Some(17) },
            wgpu::Extent3d { width: 17, height: 17, depth_or_array_layers: 1 },
        );
    }
    let nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &[0u8; 64], usage: wgpu::BufferUsages::STORAGE });
    let av = atlas.create_view(&Default::default());
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: nodes.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&av) },
        ],
    });
    let index: Vec<u8> = [0u32, 1, 2].iter().flat_map(|x| x.to_le_bytes()).collect();
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &index, usage: wgpu::BufferUsages::INDEX });
    let args: Vec<u8> = (0..8u32).flat_map(|k| [3u32, 1, 0, 0, k]).flat_map(|x| x.to_le_bytes()).collect();
    let abuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &args, usage: wgpu::BufferUsages::INDIRECT });
    let cv = color.create_view(&Default::default());
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &cv,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
        pass.multi_draw_indexed_indirect(&abuf, 0, 8);
    }
    queue.submit([enc.finish()]);
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
}

fn make(which: &str, shared: Option<&wgpu::Instance>) -> (wgpu::Device, wgpu::Queue) {
    let own;
    let instance = match shared {
        Some(i) => i,
        None => {
            own = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            &own
        }
    };
    let adapter = adapter_for(instance, which);
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("probe"),
        required_features: adapter.features() & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::INDIRECT_FIRST_INSTANCE),
        ..Default::default()
    }))
    .expect("device")
}

fn main() {
    probe::install();
    let which = arg("--adapter").unwrap_or_else(|| "warp".into());
    let threads: usize = arg("--threads").map(|s| s.parse().unwrap()).unwrap_or(5);
    let iters: usize = arg("--iters").map(|s| s.parse().unwrap()).unwrap_or(4);
    let level: u32 = arg("--level").map(|s| s.parse().unwrap()).unwrap_or(2);
    let shared = flag("--shared-instance");
    let keep = flag("--keep");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let instance = Arc::new(instance);
    let barrier = Arc::new(Barrier::new(threads));
    let sync_b = Arc::new(Barrier::new(threads));
    let sync = flag("--sync");
    let hs: Vec<_> = (0..threads)
        .map(|t| {
            let (which, instance, barrier, sync_b) = (which.clone(), Arc::clone(&instance), Arc::clone(&barrier), Arc::clone(&sync_b));
            std::thread::spawn(move || {
                let mut kept = Vec::new();
                for _ in 0..iters {
                    if sync {
                        sync_b.wait();
                    }
                    let (d, q) = make(&which, if shared { Some(&instance) } else { None });
                    work(&d, &q, level);
                    if keep {
                        kept.push((d, q));
                    }
                }
                barrier.wait();
                drop(kept);
                let _ = t;
            })
        })
        .collect();
    for h in hs {
        h.join().expect("thread");
    }
    println!("ok {which} threads {threads} iters {iters} level {level} shared {shared} keep {keep}");
}
