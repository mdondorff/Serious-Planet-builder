//! SPIKE-11 "Mouse look plumbing": a standalone winit + wgpu window with a free camera on a small checkered planet, to find which
//! mouse path works locally and over Chrome Remote Desktop. Throwaway.
//!
//! `mouse-look`                 open the window (see CHECKLIST.md for the keys)
//! `mouse-look replay FILE`     replay a recorded trace headlessly and compare the final pose with the recorded one

mod pose;
mod sim;
mod trace;

use std::sync::Arc;
use std::time::Instant;

use sim::{Command, Config, Ev, Sim, Source};
use trace::{to_line, Line};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

#[derive(Clone, Copy, PartialEq, Debug)]
enum GrabMode {
    /// Confined (falls back to nothing if refused), cursor hidden, recentred.
    Confined,
    /// Try Locked first (unsupported on Windows), then Confined.
    LockedThenConfined,
    /// No grab at all: hidden cursor, recentring only.
    None,
}

impl GrabMode {
    fn name(self) -> &'static str {
        match self {
            GrabMode::Confined => "confined",
            GrabMode::LockedThenConfined => "locked>confined",
            GrabMode::None => "none",
        }
    }
    fn next(self) -> GrabMode {
        match self {
            GrabMode::Confined => GrabMode::LockedThenConfined,
            GrabMode::LockedThenConfined => GrabMode::None,
            GrabMode::None => GrabMode::Confined,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    pos: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    fwd: [f32; 4],
    params: [f32; 4],
    size: [f32; 4],
}

struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    ubuf: wgpu::Buffer,
    bind: wgpu::BindGroup,
    adapter_line: String,
}

struct App {
    gfx: Option<Gfx>,
    sim: Sim,
    lines: Vec<Line>,
    t0: Instant,
    last_frame: Instant,
    grab: GrabMode,
    stutter: bool,
    vsync: bool,
    title_at: Instant,
    frames_since_title: u32,
    raw_at_title: u64,
    cursor_at_title: u64,
    saved: u32,
    notes: Vec<String>,
    focus_log: String,
    last_note: String,
}

fn os_name() -> &'static str {
    std::env::consts::OS
}

impl App {
    fn new() -> App {
        let mut a = App {
            gfx: None,
            sim: Sim::new(Config::default()),
            lines: Vec::new(),
            t0: Instant::now(),
            last_frame: Instant::now(),
            grab: GrabMode::Confined,
            stutter: false,
            vsync: true,
            title_at: Instant::now(),
            frames_since_title: 0,
            raw_at_title: 0,
            cursor_at_title: 0,
            saved: 0,
            notes: Vec::new(),
            focus_log: String::new(),
            last_note: String::new(),
        };
        a.start_trace("");
        a
    }

    fn t(&self) -> u64 {
        self.t0.elapsed().as_micros() as u64
    }

    fn start_trace(&mut self, note: &str) {
        self.lines.clear();
        let (scale, w, h) = match &self.gfx {
            Some(g) => {
                let s = g.window.inner_size();
                (g.window.scale_factor(), f64::from(s.width), f64::from(s.height))
            }
            None => (1.0, 960.0, 540.0),
        };
        let mut cfg = self.sim.cfg.clone();
        // A fresh trace starts from the start pose.
        self.sim.reset_pose();
        cfg.start_lat = self.sim.cfg.start_lat;
        self.lines.push(Line::Header { cfg, scale, w, h, os: os_name().into(), note: note.into() });
        self.t0 = Instant::now();
        // The replay needs the size first.
        self.lines.push(Line::Event { t: 0, ev: Ev::Resize { w, h } });
    }

    fn note(&mut self, text: String) {
        eprintln!("[{:>9.3}s] {text}", self.t() as f64 / 1e6);
        self.last_note = text.clone();
        let t = self.t();
        self.lines.push(Line::Note { t, text });
    }

    fn feed(&mut self, ev: Ev) {
        let t = self.t();
        self.lines.push(Line::Event { t, ev: ev.clone() });
        let cmds = self.sim.apply(t, &ev);
        for c in cmds {
            self.exec(c);
        }
    }

    fn exec(&mut self, c: Command) {
        let Some(g) = &self.gfx else { return };
        match c {
            Command::Grab(true) => {
                g.window.set_cursor_visible(false);
                let mut msg = String::new();
                if self.grab == GrabMode::LockedThenConfined {
                    match g.window.set_cursor_grab(CursorGrabMode::Locked) {
                        Ok(()) => msg = "grab Locked: ok".into(),
                        Err(e) => {
                            msg = format!("grab Locked refused ({e}); ");
                            match g.window.set_cursor_grab(CursorGrabMode::Confined) {
                                Ok(()) => msg += "Confined: ok",
                                Err(e) => msg += &format!("Confined refused ({e})"),
                            }
                        }
                    }
                } else if self.grab == GrabMode::Confined {
                    match g.window.set_cursor_grab(CursorGrabMode::Confined) {
                        Ok(()) => msg = "grab Confined: ok".into(),
                        Err(e) => msg = format!("grab Confined refused ({e})"),
                    }
                } else {
                    msg = "grab none (hidden cursor, recentring only)".into();
                }
                self.note(msg);
            }
            Command::Grab(false) => {
                let _ = g.window.set_cursor_grab(CursorGrabMode::None);
                g.window.set_cursor_visible(true);
                self.note("released".into());
            }
            Command::Recenter(x, y) => {
                if let Err(e) = g.window.set_cursor_position(PhysicalPosition::new(x, y)) {
                    self.sim.stats.recenter_failures += 1;
                    self.note(format!("set_cursor_position refused: {e}"));
                }
            }
        }
    }

    fn set_cfg(&mut self) {
        let c = self.sim.cfg.clone();
        let msg = format!("config: source {} recenter box {} sens {:.5} grab {}", c.source.name(), c.recenter_box, c.sens, self.grab.name());
        self.feed(Ev::Set { source: c.source, recenter_box: c.recenter_box, sens: c.sens });
        self.note(msg);
    }

    fn stats_note(&mut self) {
        let st = self.sim.stats.clone();
        let text = format!(
            "stats: raw {} cursor {} recenters {} recenter_failures {} dropped_while_free {} source {} active {} captured {}",
            st.raw_events, st.cursor_events, st.recenters, st.recenter_failures, st.dropped_while_free, self.sim.cfg.source.name(), self.sim.active_source().name(), self.sim.captured
        );
        self.note(text);
    }

    fn save(&mut self, why: &str) {
        self.stats_note();
        let t = self.t();
        self.lines.push(Line::Mark { t, text: format!("saved: {why}") });
        self.lines.push(Line::Final { pose: self.sim.pose, total_yaw: self.sim.total_yaw });
        let dir = std::path::Path::new("target/traces");
        let _ = std::fs::create_dir_all(dir);
        self.saved += 1;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir.join(format!("trace-{stamp}-{}.jsonl", self.saved));
        let text: String = self.lines.iter().map(|l| to_line(l) + "\n").collect();
        match std::fs::write(&path, text) {
            Ok(()) => eprintln!("trace written: {} ({} lines)", path.display(), self.lines.len()),
            Err(e) => eprintln!("trace write failed: {e}"),
        }
    }

    fn init_gfx(&mut self, el: &ActiveEventLoop) {
        let attrs = Window::default_attributes().with_title("SPIKE-11 mouse look").with_inner_size(LogicalSize::new(960.0, 540.0));
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .expect("adapter");
        let info = adapter.get_info();
        let adapter_line = format!("{} [{:?}]", info.name, info.backend);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("view.wgsl").into()) });
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
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }],
        });
        window.focus_window();
        self.gfx = Some(Gfx { window, surface, device, queue, config, pipeline, ubuf, bind, adapter_line });
        self.start_trace("start");
        let g = self.gfx.as_ref().unwrap();
        let line = format!("adapter {} ; scale factor {} ; inner {:?}", g.adapter_line, g.window.scale_factor(), g.window.inner_size());
        let size = g.window.inner_size();
        self.note(line);
        self.feed(Ev::Resize { w: f64::from(size.width), h: f64::from(size.height) });
    }

    fn render(&mut self) {
        let dt_us = self.last_frame.elapsed().as_micros() as u64;
        self.last_frame = Instant::now();
        self.feed(Ev::Frame { dt_us });
        if self.stutter {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let Some(g) = &mut self.gfx else { return };
        let b = self.sim.pose.basis();
        let size = g.window.inner_size();
        let f = |v: [f64; 3]| [v[0] as f32, v[1] as f32, v[2] as f32, 0.0];
        let u = Uniforms {
            pos: f(self.sim.pose.pos),
            right: f(b.right),
            up: f(b.up),
            fwd: f(b.forward),
            params: [(70.0f32.to_radians() / 2.0).tan(), size.width as f32 / size.height.max(1) as f32, self.sim.cfg.radius as f32, 0.0],
            size: [size.width as f32, size.height as f32, 0.0, 0.0],
        };
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts((&u as *const Uniforms).cast::<u8>(), std::mem::size_of::<Uniforms>()) };
        g.queue.write_buffer(&g.ubuf, 0, bytes);
        let frame = match g.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                g.surface.configure(&g.device, &g.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = g.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&g.pipeline);
            pass.set_bind_group(0, &g.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        g.queue.submit([enc.finish()]);
        g.queue.present(frame);
        self.frames_since_title += 1;
        if self.title_at.elapsed().as_millis() >= 500 {
            let secs = self.title_at.elapsed().as_secs_f64();
            let st = &self.sim.stats;
            let (raw_rate, cur_rate) = ((st.raw_events - self.raw_at_title) as f64 / secs, (st.cursor_events - self.cursor_at_title) as f64 / secs);
            let title = format!(
                "src {}({}) grab {} cap {} box {} | raw {:.0}/s cur {:.0}/s rec {} fail {} | yaw {:.1}deg pitch {:.1}deg hdg {:.0} | {:.0} fps{}{} | x{} | {}",
                self.sim.cfg.source.name(),
                self.sim.active_source().name(),
                self.grab.name(),
                if self.sim.captured { "YES" } else { "no" },
                self.sim.cfg.recenter_box,
                raw_rate,
                cur_rate,
                st.recenters,
                st.recenter_failures,
                self.sim.total_yaw.to_degrees(),
                self.sim.pose.pitch.to_degrees(),
                self.sim.pose.heading_from_north().to_degrees(),
                f64::from(self.frames_since_title) / secs,
                if self.stutter { " STUTTER" } else { "" },
                if self.vsync { "" } else { " NOVSYNC" },
                g.window.scale_factor(),
                self.last_note.chars().take(70).collect::<String>()
            );
            g.window.set_title(&title);
            self.title_at = Instant::now();
            self.frames_since_title = 0;
            self.raw_at_title = st.raw_events;
            self.cursor_at_title = st.cursor_events;
        }
    }

    fn key(&mut self, code: KeyCode, down: bool) {
        if down {
            match code {
                KeyCode::F1 => {
                    self.sim.cfg.source = self.sim.cfg.source.next();
                    return self.set_cfg();
                }
                KeyCode::F2 => {
                    self.grab = self.grab.next();
                    let msg = format!("grab mode {} (applies at the next capture)", self.grab.name());
                    return self.note(msg);
                }
                KeyCode::F3 => {
                    self.sim.cfg.recenter_box = match self.sim.cfg.recenter_box {
                        x if x < 0.1 => 0.25,
                        x if x < 0.5 => 1.0,
                        _ => 0.0,
                    };
                    return self.set_cfg();
                }
                KeyCode::F4 => {
                    self.save("F4");
                    return self.start_trace("after F4");
                }
                KeyCode::F5 => return self.feed(Ev::Reset),
                KeyCode::F6 => {
                    self.stutter = !self.stutter;
                    let msg = format!("stutter {}", self.stutter);
                    return self.note(msg);
                }
                KeyCode::F7 => {
                    self.vsync = !self.vsync;
                    if let Some(g) = &mut self.gfx {
                        g.config.present_mode = if self.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
                        g.surface.configure(&g.device, &g.config);
                    }
                    let msg = format!("vsync {}", self.vsync);
                    return self.note(msg);
                }
                KeyCode::F8 => {
                    let t = self.t();
                    let n = self.lines.iter().filter(|l| matches!(l, Line::Mark { .. })).count() + 1;
                    eprintln!("mark {n}");
                    self.lines.push(Line::Mark { t, text: format!("mark {n}") });
                    return self.stats_note();
                }
                KeyCode::Minus | KeyCode::Equal => {
                    self.sim.cfg.sens *= if code == KeyCode::Equal { 1.25 } else { 0.8 };
                    return self.set_cfg();
                }
                _ => {}
            }
        }
        // Everything else (Escape, WASD, QE, arrows) goes to the simulation, which also records it.
        self.feed(Ev::Key { code: format!("{code:?}"), down });
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_none() {
            el.listen_device_events(DeviceEvents::WhenFocused);
            self.init_gfx(el);
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, ev: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = ev {
            self.feed(Ev::Raw { dx, dy });
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                self.save("exit");
                el.exit();
            }
            WindowEvent::Resized(s) => {
                if let Some(g) = &mut self.gfx {
                    g.config.width = s.width.max(1);
                    g.config.height = s.height.max(1);
                    g.surface.configure(&g.device, &g.config);
                }
                self.feed(Ev::Resize { w: f64::from(s.width), h: f64::from(s.height) });
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.note(format!("scale factor changed to {scale_factor}")),
            WindowEvent::Focused(on) => {
                let before = self.sim.pose;
                self.feed(Ev::Focus { on });
                self.focus_log = format!("focus {on}: pose before/after identical: {}", before == self.sim.pose);
                let msg = self.focus_log.clone();
                self.note(msg);
            }
            WindowEvent::CursorMoved { position, .. } => self.feed(Ev::Cursor { x: position.x, y: position.y }),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => self.feed(Ev::Button { down: state == ElementState::Pressed }),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.repeat {
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }
}

fn replay_cmd(path: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let (pose, yaw, recorded) = trace::replay(&text)?;
    println!("replayed {path}: final pos {:?} heading {:?} pitch {:?} total yaw {:.6} rad ({:.3} deg)", pose.pos, pose.heading, pose.pitch, yaw, yaw.to_degrees());
    match recorded {
        Some((p, y)) => {
            let same = p == pose && y == yaw;
            println!("recorded final pose {}: {}", if same { "MATCHES exactly" } else { "DIFFERS" }, if same { String::new() } else { format!("recorded pos {:?} pitch {:?} yaw {y:?}", p.pos, p.pitch) });
            if same {
                Ok(())
            } else {
                Err("final pose differs".into())
            }
        }
        None => {
            println!("(the trace has no recorded final pose)");
            Ok(())
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("replay") {
        if let Err(e) = replay_cmd(args.get(2).map(String::as_str).unwrap_or("")) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }
    let el = EventLoop::new().expect("event loop");
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    el.run_app(&mut app).expect("run");
    let _ = Source::Raw;
}
