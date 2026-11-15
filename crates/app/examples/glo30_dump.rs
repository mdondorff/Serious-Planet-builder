//! River review (#138, spike branch, never merged): a GLO-30 tile resampled to square 101.8 m cells (box mean of the 1" pixels in each cell), written as raw little-endian f64.
//! `glo30_dump <tile.tif> <out.f64>`; prints `DUMP <out> <w> <h>`.
const CELL_M: f64 = 101.8;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h, z) = match "glo30" {
        "glo30" => {
            let t = planet_testkit::glo30::read(std::path::Path::new(&a[0])).unwrap_or_else(|e| panic!("{}: {e:?}", a[0]));
            let lat0 = t.geo.origin_lat - 0.5 * t.height as f64 * t.geo.scale_lat;
            let m_per_deg = 111_320.0;
            let (py, px) = (t.geo.scale_lat * m_per_deg, t.geo.scale_lon * m_per_deg * lat0.to_radians().cos());
            let (h, w) = ((t.height as f64 * py / CELL_M) as usize, (t.width as f64 * px / CELL_M) as usize);
            let mut z = vec![f64::NAN; w * h];
            for (c, v) in z.iter_mut().enumerate() {
                let (r0, r1) = ((((c / w) as f64) * CELL_M / py) as usize, ((((c / w) + 1) as f64) * CELL_M / py) as usize);
                let (c0, c1) = ((((c % w) as f64) * CELL_M / px) as usize, ((((c % w) + 1) as f64) * CELL_M / px) as usize);
                let (mut s, mut k) = (0.0, 0usize);
                for r in r0..r1.min(t.height) {
                    for col in c0..c1.min(t.width) {
                        let x = t.sample(r, col);
                        if x.is_finite() {
                            s += f64::from(x);
                            k += 1;
                        }
                    }
                }
                *v = if k > 0 { s / k as f64 } else { -100.0 };
            }
            (w, h, z)
        }
        _ => unreachable!(),
    };
    let bytes: Vec<u8> = z.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(&a[1], bytes).expect("written");
    println!("DUMP {} {w} {h}", a[1]);
}
