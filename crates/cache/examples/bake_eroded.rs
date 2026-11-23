//! SPIKE-115 (spike branch, never merged): a spike bake of R3a whose K7 starts from one round of the regional kernel at L and burns rivers routed at L, for the picture pair.
//! SPIKE-114's metrics, the relief kept, and the cost of re-routing the dirty catchments after a ridge edit against a full L route.
//!
//! Usage: `bake_eroded --out <commit dir> [--window W] [--k-scale S] [--diffusion KAPPA] [--start <file.f64>] [--rounds 0|1] [--threads T]` (margin 200). W is
//! `face,i0,i1,j0,j1,seed` or r1h (the default), r3a, r3b, m1, f1. SPIKE-121 (E3): S scales K dt (variant B), KAPPA adds that many explicit diffusion sweeps as the round has
//! steps (variant C), the start file (8160 x 8160 LE f64 over the L window, margin included) replaces B + regional and `--rounds 0` skips the L round (spikes' D to F). A variant
//! other than A mixes its file and parameters into the inputs identity; A keeps its bits.
use std::path::{Path, PathBuf};

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};

use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

struct Lattice {
    face: Face,
    i0: usize,
    j0: usize,
    w: usize,
    h: usize,
    z: Vec<f32>,
}

impl LatticeSource for Lattice {
    fn res(&self) -> u32 {
        LATTICE as u32
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let (i, j) = (i as usize, j as usize);
        if face != self.face || i < self.i0 || j < self.j0 || i >= self.i0 + self.w || j >= self.j0 + self.h {
            return Err(ReadError::Source(format!("node ({}, {i}, {j}) is outside the sampled rectangle", face.0)));
        }
        Ok(GlobalSample {
            continent: 0.0,
            elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)],
            temperature_c: 0.0,
            precipitation_mm: 0.0,
        })
    }
}

/// The canonical windows of the river gate (BUILD-055): `face,i0,i1,j0,j1,seed` in regional cells.
const WINDOWS: [(&str, [usize; 6]); 5] = [
    ("r1h", [1, 8056, 8675, 760, 1379, 14]),
    ("r3a", [4, 368, 987, 4720, 5339, 15]),
    ("r3b", [2, 9968, 10587, 8304, 8923, 15]),
    ("m1", [4, 6600, 7219, 10200, 10819, 15]),
    ("f1", [4, 4800, 5419, 3300, 3919, 15]),
];

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).map(|k| args.get(k + 1).map(String::as_str).expect("an option takes a value"))
}

/// FNV-1a over bytes: the identity of a start file and the variant's parameters.
fn fnv(bytes: impl IntoIterator<Item = u8>, mut h: u64) -> u64 {
    for b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = Path::new(&args[0]);
    let threads: usize = args.iter().position(|a| a == "--threads").map_or(8, |k| args[k + 1].parse().expect("--threads T"));
    // lead #173 (reading 30): B on the turned base for g1f8; default axis, the same bits as before
    let noise = opt(&args, "--noise").unwrap_or("axis");
    if noise != "axis" && noise != "turned" {
        return Err(format!("--noise axis|turned, not '{noise}'"));
    }
    planet_generators::height::set_turned_noise(noise == "turned");
    eprintln!("bake_eroded: base noise {noise}");
    // lead #196 (SPIKE-143): the read of the regional change K7 and the start surface take; clamped (default) is main's
    let region_read = opt(&args, "--region-read").unwrap_or("clamped");
    planet_cache::region_bake::K5_REGION_READ.store(
        match region_read {
            "clamped" => 0,
            "cr" => 1,
            "bspline" => 2,
            "bsplinei" => 3,
            o => return Err(format!("--region-read clamped | cr | bspline | bsplinei, not '{o}'")),
        },
        std::sync::atomic::Ordering::Relaxed,
    );
    eprintln!("bake_eroded: region read {region_read}");
    // SPIKE-121 (E3): the window, B's K dt scale, C's diffusion, a start surface (spikes' D to F) and the L rounds. The defaults are A on R1h, the same bits as before.
    let win: [usize; 6] = match opt(&args, "--window") {
        None => WINDOWS[0].1,
        Some(v) => match WINDOWS.iter().find(|(n, _)| n.eq_ignore_ascii_case(v)) {
            Some((_, w)) => *w,
            None => {
                let p: Vec<usize> = v
                    .split(',')
                    .map(|x| x.parse().map_err(|_| format!("--window face,i0,i1,j0,j1,seed or a name, not '{v}'")))
                    .collect::<Result<_, _>>()?;
                p.try_into().map_err(|_| format!("--window takes six numbers, not '{v}'"))?
            }
        },
    };
    let k_scale: f64 = opt(&args, "--k-scale").map_or(Ok(1.0), str::parse).map_err(|_| "--k-scale takes a number")?;
    let kappa: f64 = opt(&args, "--diffusion").map_or(Ok(0.0), str::parse).map_err(|_| "--diffusion takes a number")?;
    let l_rounds: usize = opt(&args, "--rounds").map_or(Ok(1), str::parse).map_err(|_| "--rounds takes 0 or 1")?;
    if !(k_scale > 0.0 && k_scale.is_finite()) || !(0.0..=0.25).contains(&kappa) || l_rounds > 1 {
        return Err(format!(
            "--k-scale > 0 ({k_scale}), --diffusion in 0..=0.25 for a stable explicit step ({kappa}), --rounds 0 or 1 ({l_rounds})"
        ));
    }
    let start_file = opt(&args, "--start").map(PathBuf::from);
    let (seed, face, margin) = (win[5] as u64, Face(win[0] as u8), 200usize);
    let (i0, j0, size) = (win[1], win[3], win[2] - win[1] + 1);
    if win[4] - win[3] + 1 != size {
        return Err(format!("the window {win:?} is not square: the bake lays a square region"));
    }
    let t0 = std::time::Instant::now();
    let def = WorldDefinition::single_planet(R, seed);

    let inputs = class_inputs(&def, seed, threads);
    let region = (i0, i0 + size - 1, j0, j0 + size - 1);
    let (ri0, ri1, rj0, rj1) = (region.0 - margin, region.1 + margin, region.2 - margin, region.3 + margin);
    let to_node = |reg: usize| reg * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let mut nodes = vec![0f32; nw * nh];
    let rows = nh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in nodes.chunks_mut(rows * nw).enumerate() {
            let inputs = &inputs;
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * nw + c;
                    *v = inputs.base(lattice_direction(face, LATTICE, ni0 + p % nw, nj0 + p / nw)) as f32;
                }
            });
        }
    });
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes };
    let base = |d: Vec3| -> f64 { f64::from(read_elevation_c1(&lattice, &TangentWarp, d).expect("a node of the sampled rectangle")) };
    let orography = inputs.orography();
    let climate = |d: Vec3| {
        let (pm, tc) = (orography.precipitation_mm(d), orography.temperature_c(d));
        let ro = runoff_mm(pm, tc);
        (ro / 1000.0 / SECONDS_PER_YEAR, (pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR, discharge_weight(ro))
    };
    let spec = RegionSpec {
        seed,
        face,
        radius_m: R,
        regional: REGIONAL,
        region,
        margin,
        tile: 768,
        halo: 64,
        rounds: 4,
        threads,
        res: LATTICE as u32,
        base_id: 0x6c61_7474_6963_6531 ^ (LATTICE as u64),
        inputs_id: class_inputs_key(&def, seed),
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: false,
    };
    let front = region_front(&spec, &base, &climate)?;
    let out = PathBuf::from(
        args.iter().position(|a| a == "--out").map(|k| args[k + 1].clone()).unwrap_or_else(|| "target/stores/r3a-p2e".into()),
    );
    if out.exists() {
        return Err(format!("{} exists: a spike bake never writes into an existing folder", out.display()));
    }
    let t_front = t0.elapsed().as_secs_f64();
    let l = planet_cache::region_bake::LOCAL_PER_REGIONAL;
    let (ri0, rj0, rw, rh) = (front.solve.ri0, front.solve.rj0, front.solve.rw, front.solve.rh);
    let (x0, y0, w, h) = ((ri0 * l) as i64, (rj0 * l) as i64, rw * l, rh * l);
    let n = spec.local() as f64;
    let cell_m = 101.8f64;
    let dir_of = |x: i64, y: i64| {
        planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0)
    };
    let mut z0 = vec![0f64; w * h];
    {
        let rows = h.div_ceil(threads);
        let regional = &front.regional_field;
        let base = &base;
        std::thread::scope(|s| {
            for (k, chunk) in z0.chunks_mut(rows * w).enumerate() {
                s.spawn(move || {
                    for (c, v) in chunk.iter_mut().enumerate() {
                        let p = k * rows * w + c;
                        let d = dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64);
                        *v = base(d) + regional.at(d);
                    }
                });
            }
        });
    }
    // B + regional: K7's change is taken against it, so a start file's surface (spikes' D to F) is what K7 starts from.
    let z_ref = z0.clone();
    let mut identity = 0u64;
    if let Some(path) = &start_file {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() != w * h * 8 {
            return Err(format!("{}: {} bytes, the window's L grid is {w} x {h} f64 ({} bytes)", path.display(), bytes.len(), w * h * 8));
        }
        for (c, v) in bytes.as_chunks::<8>().0.iter().enumerate() {
            z0[c] = f64::from_le_bytes(*v);
        }
        identity = fnv(bytes.iter().copied(), 0xcbf2_9ce4_8422_2325);
    }
    if k_scale != 1.0 || kappa != 0.0 || l_rounds != 1 {
        identity = fnv(
            k_scale.to_le_bytes().into_iter().chain(kappa.to_le_bytes()).chain((l_rounds as u64).to_le_bytes()),
            identity ^ 0x5350_494b_4531_3231,
        );
    }
    // One round of the regional kernel at L over the region (SPIKE-115's N = 1).
    let g = planet_generators::bake::Grid::new(w, h, cell_m);
    let outlet = g.outlets(&z0, 0.0);
    let rules = planet_generators::bake::route::RoundRules::default();
    let mut carve = planet_generators::bake::carve::CarveParams {
        steps: 5,
        reroute_every: 5,
        round_rules: rules,
        cell_metric: false,
        window_cut: true,
        ..planet_generators::bake::carve::CarveParams::default()
    };
    front.solve.precision.apply(&mut carve);
    carve.round_f32 = false;
    // B: K dt scaled (the step's uplift `u_over_k * k_dt` with it: the same dt).
    carve.k_dt *= k_scale;
    let mut z1 = z0.clone();
    if l_rounds == 1 {
        let _ = planet_generators::bake::round::solve_rounds(&g, &mut z1, &outlet, &carve, 1, &threads, front.solve.stream);
    }
    // C: after the round's carve steps, as many explicit sweeps of `z += kappa * lap5(z)` over the cells that are no outlet and stand above the sea (an operator split; the sea and the
    // outlets keep their heights; the grid's border cells are outlets).
    if kappa > 0.0 {
        for _ in 0..carve.steps {
            let prev = z1.clone();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let c = y * w + x;
                    if outlet[c] || prev[c] <= 0.0 {
                        continue;
                    }
                    let lap = prev[c - 1] + prev[c + 1] + prev[c - w] + prev[c + w] - 4.0 * prev[c];
                    z1[c] = (prev[c] + kappa * lap).max(carve.epsilon);
                }
            }
        }
    }
    let t_round = t0.elapsed().as_secs_f64();
    // Rivers routed at L on the eroded surface: channel cells by area (75 km2), the discharge from H1's runoff, the network levels (all breach), cut into segments between confluences.
    let routing = planet_generators::bake::route::route_rules(&g, &z1, &outlet, 1.0e-3, front.solve.stream, 0.0, None, rules);
    let none = planet_generators::bake::route::NONE;
    let h1_runoff: Vec<f32> = (0..rw * rh)
        .map(|c| {
            let d = front.solve.face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
            if front.solve.zs[c] > 0.0 {
                climate(d).0 as f32
            } else {
                0.0
            }
        })
        .collect();
    let (mut area, mut q) = (vec![cell_m * cell_m; w * h], vec![0f64; w * h]);
    for c in 0..w * h {
        q[c] = f64::from(h1_runoff[(c / w / l) * rw + c % w / l]) * cell_m * cell_m;
    }
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        let r = routing.receiver[c];
        if r != none {
            area[r as usize] += area[c];
            q[r as usize] += q[c];
        }
    }
    let threshold = planet_generators::bake::hydro::HydroParams::default().initiation_m2;
    let channel: Vec<bool> = area.iter().zip(&z1).map(|(a, z)| *a >= threshold && *z > 0.0).collect();
    let mut level = vec![f64::INFINITY; w * h];
    let mut donors = vec![0u8; w * h];
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        if !channel[c] {
            continue;
        }
        let v = level[c].min(z1[c]);
        level[c] = v;
        let r = routing.receiver[c];
        if r != none && channel[r as usize] {
            level[r as usize] = level[r as usize].min(v);
            donors[r as usize] = donors[r as usize].saturating_add(1);
        }
    }
    let mut channels: Vec<planet_generators::bake::k7::Channel> = Vec::new();
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        // a segment starts at a head (no channel donor) or just below a confluence (its receiver has two or more donors and it is the confluence cell itself)
        if !channel[c] || donors[c] == 1 {
            continue;
        }
        let mut cells = vec![c];
        let mut at = c;
        loop {
            let r = routing.receiver[at];
            if r == none || !channel[r as usize] {
                break;
            }
            let r = r as usize;
            cells.push(r);
            if donors[r] >= 2 {
                break;
            }
            at = r;
        }
        if cells.len() < 2 {
            continue;
        }
        channels.push(planet_generators::bake::k7::Channel {
            points: cells.iter().map(|&k| ((x0 + (k % w) as i64) as f64, (y0 + (k / w) as i64) as f64)).collect(),
            levels: cells.iter().map(|&k| level[k]).collect(),
            q: cells.iter().map(|&k| q[k] / (cell_m * cell_m) * cell_m * cell_m).collect(),
            q_unit: planet_generators::bake::hydro::QUnit::Discharge,
        });
    }
    let t_lines = t0.elapsed().as_secs_f64();
    println!(
        "BAKE-E front {t_front:.0} s, one round at L {:.0} s, L channels {:.0} s: {} segments, {} channel cells",
        t_round - t_front,
        t_lines - t_round,
        channels.len(),
        channel.iter().filter(|&&b| b).count()
    );
    // K7's start surface is the eroded one: the regional term plus the round's change at the L cell under the direction.
    let change: Vec<f32> = z1.iter().zip(&z_ref).map(|(a, b)| (a - b) as f32).collect();
    let regional_eroded = |d: Vec3| -> f64 {
        let (f, s, t) = planet_core::cube::direction_to_face(&TangentWarp, d);
        let r = front.regional_field.at(d);
        if f != face {
            return r;
        }
        let (x, y) = (((s + 1.0) * 0.5 * n).floor() as i64 - x0, ((t + 1.0) * 0.5 * n).floor() as i64 - y0);
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            return r;
        }
        r + f64::from(change[y as usize * w + x as usize])
    };
    let feed = planet_cache::l_writer::MemoryFeed { face, channels, discharge: true };
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?;
    // A variant other than A names itself in the inputs identity (its start file and parameters), so the gate's key check tells the stores apart; A keeps the identity it had.
    let inputs_id = spec.effective_inputs_id() ^ identity;
    let basin = planet_cache::basin_table::BasinSpec {
        hydro: planet_generators::bake::hydro::HydroParams::default(),
        tile_cells: spec.tile as u32,
        tiles_per_edge: spec.tiles_per_edge(),
        base_id: spec.base_id,
        inputs_id,
    };
    let l_spec = planet_cache::layer_l::LTileSpec {
        grid: planet_generators::bake::tile::TileGrid { cells: spec.local(), tile: spec.tile, halo: spec.halo, radius_m: spec.radius_m },
        // the plain flood in K7: the canonical flood refuses flats of the eroded surface (SPIKE-115 finding)
        k7: planet_generators::bake::k7::K7Params::standard(spec.seed, 0.0),
        base_id: spec.base_id,
        inputs_id,
    };
    let h1_cell = |f: u8, i: i64, j: i64| {
        let (i, j) = (i - ri0 as i64, j - rj0 as i64);
        (f == face.0 && i >= 0 && j >= 0 && (i as usize) < rw && (j as usize) < rh).then(|| j as usize * rw + i as usize)
    };
    let tile_grid = l_spec.grid;
    let surface = |d: Vec3| base(d) + regional_eroded(d);
    let tiles: Vec<planet_generators::bake::tile::TileId> =
        spec.tiles().into_iter().filter(|&id| planet_generators::bake::tile::tile_has_land(&tile_grid, id, &surface, 0.0, 8)).collect();
    let b = planet_cache::l_writer::LBake {
        world: &world,
        res: spec.res,
        spec: l_spec,
        basin,
        base: &base,
        regional: &regional_eroded,
        feed: &feed,
        lakes: &front.lakes,
        h1_cells: spec.regional,
        h1_cell: &h1_cell,
        face_of: &|_| face,
        threads,
        graph: None,
    };
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let commit = planet_cache::l_writer::bake_l(&out, &b, &tiles)?;
    std::fs::write(
        out.join("README.txt"),
        format!(
            "SPIKE BAKE, not the world bake (SPIKE-115, SPIKE-120, SPIKE-121): window {win:?} (face, i0, i1, j0, j1, seed), K7 started from {} of the regional kernel at L (K dt x {k_scale}, diffusion {kappa}), start surface {}, fed with rivers routed at L (no channel graph stored: the format holds H1 lines only, so no river water is drawn). Commit {:016x}, {} tiles, inputs identity {inputs_id:016x}. Branch spike/e3-variants-bc (spike/b-turned; copied to spike/rg-prototype for lead #196), base noise {noise}, region read {region_read}.\n",
            if l_rounds == 1 { "one round" } else { "no round" },
            start_file.as_ref().map_or_else(|| "B + regional".to_string(), |p| p.display().to_string()),
            commit.id,
            tiles.len()
        ),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "BAKE-E commit {:016x}, {} tiles, into {} ({:.0} s in all)",
        commit.id,
        tiles.len(),
        out.display(),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
