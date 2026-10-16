//! M4 fix F5 micro-benchmark (docs/learnings.md): what a frame's tile writes cost on the queue, one call per slot against one call per
//! run of adjacent slots, for compact tiles (33 x 33 texels of 8 bytes into an atlas, plus a 128-byte record each) and for
//! vertex-format tiles (53,724 bytes each into a pool buffer), and the CPU conversion of a vertex tile to bytes. Raw wgpu with its own
//! device, so it measures the queue and the staging only. Indicative: the GPU clock state is not controlled, never a measurement
//! for a budget. Not production code.
//!
//! `cargo run --release -p planet-render --example upload_bench -- [--adapter software|hardware] [--iters 300]`

#![allow(clippy::disallowed_methods)]

use planet_generators::mesh::MeshVertex;
use std::time::Instant;

const W: u32 = 33;
const TEXEL: usize = 8;
const ROW_BYTES: usize = W as usize * TEXEL;
const RECORD: usize = 128;
const COLS: u32 = 16;
const VERTEX_TILE_BYTES: usize = 1_221 * 44;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn stats(mut v: Vec<f64>) -> String {
    v.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    format!("median {:.3} ms, p99 {:.3} ms", at(0.5), at(0.99))
}

fn main() {
    let hardware = arg("--adapter").is_some_and(|a| a == "hardware");
    let iters: usize = arg("--iters").and_then(|v| v.parse().ok()).unwrap_or(300);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: if hardware { wgpu::PowerPreference::HighPerformance } else { wgpu::PowerPreference::None },
        force_fallback_adapter: !hardware,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .expect("adapter");
    let info = adapter.get_info();
    println!(
        "adapter: {:?} {} ({:?}); {iters} iterations per row; CPU time of the write calls plus the submit",
        info.backend, info.name, info.device_type
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");

    let side = COLS * W;
    let atlas = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench-atlas"),
        size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let records = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bench-records"),
        size: (COLS * COLS * 8 * RECORD as u32) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pool = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bench-pool"),
        size: (256 * VERTEX_TILE_BYTES) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let tile_texels = vec![7u8; ROW_BYTES * W as usize];
    let record = [3u8; RECORD];
    let vertex_tile = vec![5u8; VERTEX_TILE_BYTES];

    let texel_dst = |slot: u32| wgpu::TexelCopyTextureInfo {
        texture: &atlas,
        mip_level: 0,
        origin: wgpu::Origin3d { x: (slot % COLS) * W, y: (slot / COLS) * W, z: 0 },
        aspect: wgpu::TextureAspect::All,
    };

    println!("-- compact tiles (texels 33 x 33 x 8 B + record 128 B each; adjacent slots)");
    for n in [1u32, 10, 118] {
        // Per slot: two calls per tile.
        let mut per_slot = Vec::new();
        for _ in 0..iters {
            let t = Instant::now();
            for s in 0..n {
                queue.write_texture(
                    texel_dst(s),
                    &tile_texels,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW_BYTES as u32), rows_per_image: Some(W) },
                    wgpu::Extent3d { width: W, height: W, depth_or_array_layers: 1 },
                );
                queue.write_buffer(&records, u64::from(s) * RECORD as u64, &record);
            }
            queue.submit([]);
            per_slot.push(t.elapsed().as_secs_f64() * 1000.0);
            device.poll(wgpu::PollType::wait_indefinitely()).ok();
        }
        // Batched: a run per atlas row (16 slots per row) for texels, one record run.
        let mut batched = Vec::new();
        let mut calls = 0;
        for _ in 0..iters {
            calls = 0;
            let t = Instant::now();
            let mut s = 0;
            while s < n {
                let k = (COLS - s % COLS).min(n - s);
                let mut data = vec![0u8; k as usize * ROW_BYTES * W as usize];
                for tile in 0..k as usize {
                    for row in 0..W as usize {
                        let dst = (row * k as usize + tile) * ROW_BYTES;
                        data[dst..dst + ROW_BYTES].copy_from_slice(&tile_texels[row * ROW_BYTES..(row + 1) * ROW_BYTES]);
                    }
                }
                queue.write_texture(
                    texel_dst(s),
                    &data,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(k * ROW_BYTES as u32), rows_per_image: Some(W) },
                    wgpu::Extent3d { width: k * W, height: W, depth_or_array_layers: 1 },
                );
                calls += 1;
                s += k;
            }
            let mut recs = Vec::with_capacity(n as usize * RECORD);
            for _ in 0..n {
                recs.extend_from_slice(&record);
            }
            queue.write_buffer(&records, 0, &recs);
            calls += 1;
            queue.submit([]);
            batched.push(t.elapsed().as_secs_f64() * 1000.0);
            device.poll(wgpu::PollType::wait_indefinitely()).ok();
        }
        println!("N={n:>3}: per slot ({} calls) {}; batched ({calls} calls) {}", 2 * n, stats(per_slot), stats(batched));
    }

    println!("-- vertex-format tiles (53,724 B each, adjacent slots)");
    for n in [1u32, 10, 19] {
        let mut per_slot = Vec::new();
        for _ in 0..iters {
            let t = Instant::now();
            for s in 0..n {
                queue.write_buffer(&pool, (s as usize * VERTEX_TILE_BYTES) as u64, &vertex_tile);
            }
            queue.submit([]);
            per_slot.push(t.elapsed().as_secs_f64() * 1000.0);
            device.poll(wgpu::PollType::wait_indefinitely()).ok();
        }
        let mut batched = Vec::new();
        for _ in 0..iters {
            let t = Instant::now();
            let mut data = Vec::with_capacity(n as usize * VERTEX_TILE_BYTES);
            for _ in 0..n {
                data.extend_from_slice(&vertex_tile);
            }
            queue.write_buffer(&pool, 0, &data);
            queue.submit([]);
            batched.push(t.elapsed().as_secs_f64() * 1000.0);
            device.poll(wgpu::PollType::wait_indefinitely()).ok();
        }
        println!("N={n:>3}: per slot ({n} calls) {}; batched (1 call) {}", stats(per_slot), stats(batched));
    }

    println!("-- CPU conversion of one vertex tile (1,221 vertices) to bytes");
    let vertices = vec![MeshVertex::default(); 1_221];
    let mut a = Vec::new();
    for _ in 0..iters {
        let t = Instant::now();
        let bytes: Vec<u8> = vertices.iter().flat_map(|v| v.to_floats().into_iter().flat_map(|x| x.to_le_bytes())).collect();
        a.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(bytes);
    }
    let mut b = Vec::new();
    for _ in 0..iters {
        let t = Instant::now();
        let mut bytes = Vec::with_capacity(vertices.len() * MeshVertex::STRIDE as usize);
        for v in &vertices {
            for x in v.to_floats() {
                bytes.extend_from_slice(&x.to_le_bytes());
            }
        }
        b.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(bytes);
    }
    println!("flat_map {}; extend_from_slice {}", stats(a), stats(b));
}
