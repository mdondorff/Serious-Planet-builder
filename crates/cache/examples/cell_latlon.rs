//! SPIKE-134 helper (spike branch only): `cell_latlon <face> <i> <j> [cells]` prints the latitude and longitude in degrees of the centre of cell (i, j) on a
//! face of `cells` cells an edge (default 12,288, the regional grid; 98,304 for L), by the tangent warp.
use planet_core::{Face, TangentWarp};

fn main() {
    let a: Vec<f64> = std::env::args().skip(1).map(|x| x.parse().expect("numbers")).collect();
    let (face, i, j) = (a[0] as u8, a[1], a[2]);
    let n = a.get(3).copied().unwrap_or(12_288.0);
    let d = planet_core::cube::face_to_direction(&TangentWarp, Face(face), 2.0 * (i + 0.5) / n - 1.0, 2.0 * (j + 0.5) / n - 1.0);
    let (lat, lon) = d.lat_lon();
    println!("{:.6} {:.6}", lat.to_degrees(), lon.to_degrees());
}
