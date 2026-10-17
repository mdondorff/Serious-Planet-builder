//! Exploration: why does a 21 MB `write_buffer` into a 128 MiB storage buffer take much longer to execute on WARP than the same bytes
//! through `write_texture`? Raw wgpu, its own device. Indicative.
//! `cargo run --release -p planet-render --example probe_wbuf -- [--adapter software|hardware]`

#![allow(clippy::disallowed_methods)]

use std::time::Instant;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn main() {
    let hardware = arg("--adapter").as_deref() == Some("hardware");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: if hardware { wgpu::PowerPreference::HighPerformance } else { wgpu::PowerPreference::None },
        force_fallback_adapter: !hardware,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .expect("adapter");
    println!("{} [{:?}]", adapter.get_info().name, adapter.get_info().backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("device");
    let data = vec![7u8; 21_000_000];
    let finish = |t: Instant| -> f64 {
        queue.submit(std::iter::empty::<wgpu::CommandBuffer>());
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        t.elapsed().as_secs_f64() * 1e3
    };
    let cases: [(&str, u64, wgpu::BufferUsages); 5] = [
        ("128 MiB STORAGE|COPY_DST|COPY_SRC", 128 << 20, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC),
        ("128 MiB COPY_DST", 128 << 20, wgpu::BufferUsages::COPY_DST),
        ("128 MiB STORAGE|COPY_DST", 128 << 20, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST),
        ("32 MiB STORAGE|COPY_DST|COPY_SRC", 32 << 20, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC),
        ("24 MiB STORAGE|COPY_DST|COPY_SRC", 24 << 20, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC),
    ];
    println!("one write_buffer of 21 MB at offset 0, then an empty submit and wait; median of 5 fresh buffers (first write) and of 5 rewrites:");
    for (name, size, usage) in cases {
        let (mut first, mut again) = (Vec::new(), Vec::new());
        for _ in 0..5 {
            let b = device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false });
            let t = Instant::now();
            queue.write_buffer(&b, 0, &data);
            first.push(finish(t));
            let t = Instant::now();
            queue.write_buffer(&b, 0, &data);
            again.push(finish(t));
        }
        println!("  {name:<36} first {:>8.2} ms   rewrite {:>8.2} ms", median(first), median(again));
    }
    println!("scaling of the write size into a 128 MiB STORAGE|COPY_DST buffer (rewrite, median of 5):");
    {
        let b = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 128 << 20, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        for bytes in [8_712usize, 262_144, 1 << 20, 4 << 20, 8 << 20, 21_000_000] {
            let mut v = Vec::new();
            for _ in 0..5 {
                let t = Instant::now();
                queue.write_buffer(&b, 0, &data[..bytes]);
                v.push(finish(t));
            }
            let ms = median(v);
            println!("  {bytes:>9} bytes: {ms:8.2} ms = {:.1} MB/s", bytes as f64 / 1e6 / (ms / 1e3));
        }
    }
    // The texture path for comparison: 4096 x 4096 Rgba16Uint, a rectangle of 21 MB (4096 x 640 texels x 8 B = 21 MB).
    let (mut first, mut again) = (Vec::new(), Vec::new());
    for _ in 0..5 {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: 4096, height: 4096, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let write = || {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &data[..4096 * 8 * 640],
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4096 * 8), rows_per_image: Some(640) },
                wgpu::Extent3d { width: 4096, height: 640, depth_or_array_layers: 1 },
            );
        };
        let t = Instant::now();
        write();
        first.push(finish(t));
        let t = Instant::now();
        write();
        again.push(finish(t));
    }
    println!("  {:<36} first {:>8.2} ms   rewrite {:>8.2} ms", "4096^2 Rgba16Uint texture, 640 rows", median(first), median(again));
}
