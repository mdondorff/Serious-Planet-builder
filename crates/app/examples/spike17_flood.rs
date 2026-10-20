//! SPIKE-17 (spike branch only): around an owner bundle's camera, a 600 km square of the bake on a tangent grid (2.5 km): the points
//! connected to an ocean vertex through bake ground at or under the band elevation, against the 100 km reach of ADR 0056.
use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_frame::TerrainBundle;
use planet_generators::water::coast_band_elevation_m;
const R: f64 = 6_371_000.0;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = TerrainBundle::from_text(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, b.request.seed), 256).unwrap();
    let c = b.request.camera.position.0.normalized();
    let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    let east = c.cross(helper).normalized();
    let north = c.cross(east).normalized();
    let n = 120i32; // +-300 km at 2.5 km
    let step = 2_500.0;
    let w = (2 * n + 1) as usize;
    let band = coast_band_elevation_m(0.0);
    let mut cls = vec![(0u8, 0.0f32); w * w]; // 1 ocean vertex, 2 band land, 3 high land
    for j in -n..=n { for i in -n..=n {
        let q = (c + east * (f64::from(i) * step / R) + north * (f64::from(j) * step / R)).normalized();
        let p = world.bake_probe(q).unwrap();
        let k = ((j + n) as usize) * w + (i + n) as usize;
        cls[k] = (if p.ocean_vertex { 1 } else if f64::from(p.elevation_m) <= band { 2 } else { 3 }, p.elevation_m);
    } }
    // flood from the ocean over band ground (4-neighbours)
    let mut dist = vec![f64::INFINITY; w * w]; // path steps, then convert
    let mut stack: Vec<usize> = Vec::new();
    for k in 0..w * w { if cls[k].0 == 1 { dist[k] = 0.0; stack.push(k); } }
    let mut head = 0;
    while head < stack.len() {
        let k = stack[head]; head += 1;
        let (x, y) = ((k % w) as i32, (k / w) as i32);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= w as i32 { continue; }
            let nk = ny as usize * w + nx as usize;
            if cls[nk].0 == 2 && dist[nk].is_infinite() { dist[nk] = dist[k] + 1.0; stack.push(nk); }
        }
    }
    let ck = (n as usize) * w + n as usize;
    println!("camera cell: class {} bake elevation {:.1} m, connected to the ocean through band ground: {} (path {:.0} steps = {:.0} km)", cls[ck].0, cls[ck].1, dist[ck].is_finite(), dist[ck], dist[ck] * step / 1000.0);
    // euclidean distance to the nearest ocean vertex vs connectivity, over the connected band cells
    let oceans: Vec<(i32, i32)> = (0..w * w).filter(|&k| cls[k].0 == 1).map(|k| ((k % w) as i32, (k / w) as i32)).collect();
    let (mut conn, mut conn_far, mut max_far) = (0, 0, 0.0f64);
    for k in 0..w * w { if cls[k].0 == 2 && dist[k].is_finite() {
        conn += 1;
        let (x, y) = ((k % w) as i32, (k / w) as i32);
        let d = oceans.iter().map(|o| f64::from((o.0 - x).pow(2) + (o.1 - y).pow(2)).sqrt() * step).fold(f64::INFINITY, f64::min);
        if d > 100_000.0 { conn_far += 1; max_far = max_far.max(d); }
    } }
    println!("band cells connected to the ocean: {conn}, of them farther than the 100 km reach from every ocean vertex: {conn_far} (up to {:.0} km)", max_far / 1000.0);
}
